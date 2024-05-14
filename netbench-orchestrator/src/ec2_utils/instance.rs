// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    ec2_utils::{
        launch_plan::LaunchPlan,
        types::{Az, EndpointType, HostIps, PrivIp, PubIp},
    },
    orchestrator::{HostConfig, OrchError, OrchResult, OrchestratorConfig, STATE},
};
use aws_sdk_ec2::types::{
    BlockDeviceMapping, EbsBlockDevice, IamInstanceProfileSpecification, Instance,
    InstanceNetworkInterfaceSpecification, InstanceStateName, InstanceType, PlacementGroup,
    ResourceType, ShutdownBehavior, Tag, TagSpecification,
};
use std::{collections::HashMap, net::IpAddr, str::FromStr, time::Duration};
use tracing::{debug, info};

pub async fn launch_instances(
    ec2_client: &aws_sdk_ec2::Client,
    launch_plan: &LaunchPlan<'_>,
    security_group_id: &str,
    unique_id: &str,
    host_config: &HostConfig,
    placement_map: &HashMap<Az, PlacementGroup>,
    endpoint_type: EndpointType,
) -> OrchResult<Instance> {
    let instance_type = InstanceType::from(host_config.instance_type().as_str());

    let subnet_id = launch_plan
        .networking_detail
        .get(&host_config.az.clone().into())
        .ok_or(OrchError::Ec2 {
            dbg: "Subnet not found".to_string(),
        })?;

    let placement = host_config.to_ec2_placement(placement_map)?;
    let launch_request = ec2_client
        .run_instances()
        .placement(placement)
        .set_key_name(STATE.ssh_key_name.map(|s| s.to_string()))
        .iam_instance_profile(
            IamInstanceProfileSpecification::builder()
                .arn(&launch_plan.instance_profile_arn)
                .build(),
        )
        .instance_type(instance_type)
        .image_id(&launch_plan.ami_id)
        .instance_initiated_shutdown_behavior(ShutdownBehavior::Terminate)
        // give the instances human readable names. name is set via tags
        .tag_specifications(
            TagSpecification::builder()
                .resource_type(ResourceType::Instance)
                .tags(
                    Tag::builder()
                        .key("Name")
                        .value(instance_name(unique_id, endpoint_type))
                        .build(),
                )
                .build(),
        )
        .block_device_mappings(
            BlockDeviceMapping::builder()
                .device_name("/dev/xvda")
                .ebs(
                    EbsBlockDevice::builder()
                        .delete_on_termination(true)
                        .volume_size(50)
                        .build(),
                )
                .build(),
        )
        .network_interfaces(
            InstanceNetworkInterfaceSpecification::builder()
                .associate_public_ip_address(true)
                .delete_on_termination(true)
                .device_index(0)
                .subnet_id(subnet_id.as_string())
                .groups(security_group_id)
                .build(),
        )
        .min_count(1_i32)
        .max_count(1_i32)
        .send()
        .await
        .map_err(|err| OrchError::Ec2 {
            dbg: format!("{:#?}", err),
        })?;
    let instance = launch_request.instances();

    // Get the launched instance
    instance
        .first()
        .ok_or(OrchError::Ec2 {
            dbg: "Failed to launch instance".to_string(),
        })
        .cloned()
}

fn instance_name(unique_id: &str, endpoint_type: EndpointType) -> String {
    format!("{}_{}", endpoint_type.as_str().to_lowercase(), unique_id)
}

// Wait for running state
pub async fn poll_running(
    ec2_client: &aws_sdk_ec2::Client,
    instance: &Instance,
    launch_cnt: usize,
    endpoint_type: &EndpointType,
) -> OrchResult<HostIps> {
    let mut actual_instance_state = InstanceStateName::Pending;
    let mut host_ip = None;
    let mut attempt = 1;
    while actual_instance_state != InstanceStateName::Running {
        let instance_id = instance.instance_id().expect("describe_instances failed");
        let result = ec2_client
            .describe_instances()
            .instance_ids(instance_id)
            .send()
            .await
            .map_err(|err| OrchError::Ec2 {
                dbg: err.to_string(),
            })?;

        let instance = result
            .reservations()
            .first()
            .and_then(|reservation| reservation.instances().first())
            .expect("failed to get instance");

        // Get public and private ips
        host_ip = instance
            .private_ip_address()
            .and_then(|ip| IpAddr::from_str(ip).ok())
            .and_then(|private_ip| {
                instance
                    .public_ip_address()
                    .and_then(|ip| IpAddr::from_str(ip).ok())
                    .map(|public_ip| HostIps::new(PrivIp(private_ip), PubIp(public_ip)))
            });

        // Get the current instance state
        actual_instance_state = instance
            .state()
            .and_then(|state| state.name())
            .expect("Failed to get instance state")
            .clone();

        debug!("poll attempt: {:?}", attempt);
        attempt += 1;
        info!(
            "{:?} {} state: {:?}",
            endpoint_type, launch_cnt, actual_instance_state
        );
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    host_ip.ok_or(OrchError::Ec2 {
        dbg: "Failed to launch EC2 host".to_string(),
    })
}

pub async fn get_instance_profile(
    iam_client: &aws_sdk_iam::Client,
    config: &OrchestratorConfig,
) -> OrchResult<String> {
    let instance_profile_arn = iam_client
        .get_instance_profile()
        .instance_profile_name(config.cdk_config.netbench_runner_instance_profile())
        .send()
        .await
        .map_err(|err| OrchError::Iam {
            dbg: err.to_string(),
        })?;

    Ok(instance_profile_arn
        .instance_profile()
        .expect("instance_profile failed")
        .arn()
        .to_string())
}

pub async fn get_latest_ami(ssm_client: &aws_sdk_ssm::Client) -> OrchResult<String> {
    let ami_id = ssm_client
        .get_parameter()
        .name(STATE.ami_name)
        .with_decryption(true)
        .send()
        .await
        .map_err(|err| OrchError::Ssm {
            dbg: err.to_string(),
        })?
        .parameter()
        .expect("expected ami value")
        .value()
        .expect("expected ami value")
        .into();
    Ok(ami_id)
}
