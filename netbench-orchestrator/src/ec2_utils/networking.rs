// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    ec2_utils::{
        launch_plan::NetworkingInfraDetail,
        types::{Az, SubnetId, VpcId},
        InfraDetail, PlacementGroup,
    },
    orchestrator::{OrchError, OrchResult, OrchestratorConfig, STATE},
};
use aws_sdk_ec2::types::{
    Filter, IpPermission, IpRange, PlacementStrategy, ResourceType, TagSpecification,
    UserIdGroupPair,
};
use std::collections::HashMap;
use tracing::info;

pub async fn set_routing_permissions(
    ec2_client: &aws_sdk_ec2::Client,
    infra: &InfraDetail,
) -> OrchResult<()> {
    let security_group_id = &infra.security_group_id;

    let sg_group = UserIdGroupPair::builder()
        .set_group_id(Some(security_group_id.clone()))
        .build();

    // Egress
    ec2_client
        .authorize_security_group_egress()
        .group_id(security_group_id.clone())
        .ip_permissions(
            // Authorize SG (all traffic within the same SG)
            IpPermission::builder()
                .from_port(-1)
                .to_port(-1)
                .ip_protocol("-1")
                .user_id_group_pairs(sg_group.clone())
                .build(),
        )
        .send()
        .await
        .map_err(|err| OrchError::Ec2 {
            dbg: format!("Failed to set egress permissions: {err}"),
        })?;

    let ssh_ip_range = IpRange::builder().cidr_ip("0.0.0.0/0").build();
    // TODO only specify the russula ports
    let russula_ip_range = IpRange::builder().cidr_ip("0.0.0.0/0").build();
    let public_host_ip_ranges: Vec<IpRange> = infra
        .clients
        .iter()
        .chain(infra.servers.iter())
        .map(|instance_detail| {
            info!("{}", instance_detail);

            IpRange::builder()
                .cidr_ip(format!("{}/32", instance_detail.host_ips().public_ip()))
                .build()
        })
        .collect();

    // Ingress
    ec2_client
        .authorize_security_group_ingress()
        .group_id(security_group_id.clone())
        .ip_permissions(
            // Authorize SG (all traffic within the same SG)
            IpPermission::builder()
                .from_port(-1)
                .to_port(-1)
                .ip_protocol("-1")
                .user_id_group_pairs(sg_group)
                .build(),
        )
        .ip_permissions(
            // Authorize all host ips
            IpPermission::builder()
                .from_port(-1)
                .to_port(-1)
                .ip_protocol("-1")
                .set_ip_ranges(Some(public_host_ip_ranges.clone()))
                .build(),
        )
        .ip_permissions(
            // Authorize port 22 (ssh)
            IpPermission::builder()
                .from_port(22)
                .to_port(22)
                .ip_protocol("tcp")
                .ip_ranges(ssh_ip_range)
                .build(),
        )
        .ip_permissions(
            // Authorize russula ports (Coordinator <-> Workers)
            IpPermission::builder()
                .from_port(STATE.russula_port.into())
                .to_port(STATE.russula_port.into())
                .ip_protocol("tcp")
                .ip_ranges(russula_ip_range)
                .build(),
        )
        .send()
        .await
        .map_err(|err| OrchError::Ec2 {
            dbg: format!("Failed to set ingress permissions: {err}"),
        })?;

    Ok(())
}

// Create one per VPC. There is 1 VPC per region.
pub async fn create_security_group(
    ec2_client: &aws_sdk_ec2::Client,
    vpc_id: &VpcId,
    unique_id: &str,
) -> OrchResult<String> {
    let security_group_id = {
        let req = ec2_client
            .create_security_group()
            .group_name(STATE.security_group_name(unique_id))
            .description("This is a security group for a single run of netbench.")
            .vpc_id(vpc_id.as_string())
            .tag_specifications(
                TagSpecification::builder()
                    .resource_type(ResourceType::SecurityGroup)
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("Name")
                            .value(STATE.security_group_name(unique_id))
                            .build(),
                    )
                    .build(),
            )
            .send()
            .await
            .map_err(|err| OrchError::Ec2 {
                dbg: err.to_string(),
            })?;

        req.group_id()
            .ok_or(OrchError::Ec2 {
                dbg: "Failed to create security group".to_owned(),
            })?
            .into()
    };
    Ok(security_group_id)
}

pub async fn get_subnet_vpc_ids(
    ec2_client: &aws_sdk_ec2::Client,
    config: &OrchestratorConfig,
) -> OrchResult<(NetworkingInfraDetail, VpcId)> {
    let subnets = ec2_client
        .describe_subnets()
        .filters(
            Filter::builder()
                .name(config.cdk_config.netbench_runner_subnet_tag_key())
                .values(config.cdk_config.netbench_runner_subnet_tag_value())
                .build(),
        )
        .send()
        .await
        .map_err(|e| OrchError::Ec2 {
            dbg: format!("Couldn't describe subnets: {:#?}", e),
        })?;

    let subnets = subnets.subnets();
    assert!(!subnets.is_empty(), "No subnets found");
    tracing::debug!("{:?}", subnets);

    let mut az_subnet_map = HashMap::new();

    let mut vpc_id = None;
    for subnet in subnets.iter() {
        let az = Az::from(
            subnet
                .availability_zone()
                .ok_or(OrchError::Ec2 {
                    dbg: "Couldn't find AZ".into(),
                })?
                .to_owned(),
        );
        let subnet_id = SubnetId::from(
            subnet
                .subnet_id()
                .ok_or(OrchError::Ec2 {
                    dbg: "Couldn't find subnet".into(),
                })?
                .to_owned(),
        );
        let subnet_vpc_id = VpcId::from(
            subnet
                .vpc_id()
                .ok_or(OrchError::Ec2 {
                    dbg: "Couldn't find vpc".into(),
                })?
                .to_owned(),
        );
        // all subnets should have the same VPC id
        if let Some(ref vpc_id) = vpc_id {
            assert_eq!(vpc_id, &subnet_vpc_id);
        }
        vpc_id = Some(subnet_vpc_id);

        az_subnet_map.insert(az, subnet_id);
    }
    let vpc_id = vpc_id.expect("VPC id should be set at this point");

    // Validate that we have a subnet for each AZ
    for host_config in config.client_config.iter() {
        let az = Az::from(host_config.az.clone());
        if !az_subnet_map.contains_key(&az) {
            return Err(OrchError::Ec2 {
                dbg: "Subnet not found for Az: {az}".into(),
            });
        }
    }
    // Validate that we have a subnet for each AZ
    for host_config in config.server_config.iter() {
        let az = Az::from(host_config.az.clone());
        if !az_subnet_map.contains_key(&az) {
            return Err(OrchError::Ec2 {
                dbg: "Subnet not found for Az: {az}".into(),
            });
        }
    }

    Ok((az_subnet_map, vpc_id))
}

pub async fn create_placement_group(
    ec2_client: &aws_sdk_ec2::Client,
    az: &Az,
    unique_id: &str,
) -> OrchResult<PlacementGroup> {
    let placement = ec2_client
        .create_placement_group()
        .group_name(format!("cluster-{}-{}", unique_id, az))
        .strategy(PlacementStrategy::Cluster)
        .send()
        .await
        .map_err(|err| OrchError::Ec2 {
            dbg: format!("{}", err),
        })?;
    placement
        .placement_group()
        .ok_or(OrchError::Ec2 {
            dbg: "Failed to retrieve placement_group".to_string(),
        })
        .cloned()
}
