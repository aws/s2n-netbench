// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    orchestrator::{OrchError, OrchResult, OrchestratorConfig, STATE},
    InfraDetail,
};
use aws_sdk_ec2::types::{
    Filter, IpPermission, IpRange, ResourceType, TagSpecification, UserIdGroupPair,
};
use std::collections::HashMap;
use tracing::info;

pub async fn set_routing_permissions(
    ec2_client: &aws_sdk_ec2::Client,
    infra: &InfraDetail,
) -> OrchResult<()> {
    let sg_id = infra.security_group_id.clone();

    let sg_group = UserIdGroupPair::builder()
        .set_group_id(Some(sg_id.clone()))
        .build();

    // Egress
    ec2_client
        .authorize_security_group_egress()
        .group_id(sg_id.clone())
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
            dbg: err.to_string(),
        })?;

    let ssh_ip_range = IpRange::builder().cidr_ip("0.0.0.0/0").build();
    // TODO can we make this more restrictive?
    let russula_ip_range = IpRange::builder().cidr_ip("0.0.0.0/0").build();
    let public_host_ip_ranges: Vec<IpRange> = infra
        .clients
        .iter()
        .chain(infra.servers.iter())
        .map(|instance_detail| {
            info!("{}", instance_detail);

            IpRange::builder()
                .cidr_ip(format!("{}/32", instance_detail.host_ips.public_ip()))
                .build()
        })
        .collect();

    // Ingress
    ec2_client
        .authorize_security_group_ingress()
        .group_id(sg_id.clone())
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
            dbg: err.to_string(),
        })?;

    Ok(())
}

// Create one per VPC. There is 1 VPC per region.
pub async fn create_security_group(
    ec2_client: &aws_sdk_ec2::Client,
    vpc_id: &VpcId,
    unique_id: &str,
) -> OrchResult<String> {
    let security_group_id = ec2_client
        .create_security_group()
        .group_name(STATE.security_group_name(unique_id))
        .description("This is a security group for a single run of netbench.")
        .vpc_id(vpc_id.into_string())
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
        })?
        .group_id()
        .expect("expected security_group_id")
        .into();
    Ok(security_group_id)
}

pub type NetworkingInfraDetail = HashMap<Az, SubnetId>;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct SubnetId(String);

impl SubnetId {
    pub fn into_string(&self) -> String {
        self.clone().0
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct Az(String);

impl std::fmt::Display for Az {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)?;
        Ok(())
    }
}

impl From<String> for Az {
    fn from(value: String) -> Self {
        Az(value)
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct VpcId(String);

impl VpcId {
    pub fn into_string(&self) -> String {
        self.clone().0
    }
}

pub async fn get_subnet_vpc_ids(
    ec2_client: &aws_sdk_ec2::Client,
    config: &OrchestratorConfig,
) -> OrchResult<(NetworkingInfraDetail, VpcId)> {
    let describe_subnet_output = ec2_client
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
    assert!(
        describe_subnet_output.subnets().expect("No subnets?").len() >= 1,
        "Couldn't describe subnets"
    );

    tracing::debug!("{:?}", describe_subnet_output.subnets());

    let mut map = HashMap::new();

    let subnets = &describe_subnet_output.subnets().expect("subnets failed");
    let mut vpc_id = None;
    for subnet in subnets.iter() {
        let az = Az(subnet
            .availability_zone()
            .ok_or(OrchError::Ec2 {
                dbg: "Couldn't find subnet".into(),
            })?
            .to_owned());
        let subnet_id = SubnetId(
            subnet
                .subnet_id()
                .ok_or(OrchError::Ec2 {
                    dbg: "Couldn't find subnet".into(),
                })?
                .to_owned(),
        );
        vpc_id = Some(VpcId(
            subnet
                .vpc_id()
                .ok_or(OrchError::Ec2 {
                    dbg: "Couldn't find vpc".into(),
                })?
                .to_owned(),
        ));
        map.insert(az, subnet_id);
    }

    for host_config in config.client_config.iter() {
        let az = Az(host_config.az.clone());
        if !map.contains_key(&az) {
            return Err(OrchError::Ec2 {
                dbg: "Subnet not found for Az: {az}".into(),
            });
        }
    }
    for host_config in config.server_config.iter() {
        let az = Az(host_config.az.clone());
        if !map.contains_key(&az) {
            return Err(OrchError::Ec2 {
                dbg: "Subnet not found for Az: {az}".into(),
            });
        }
    }

    Ok((map, vpc_id.expect("VPC not found")))
}
