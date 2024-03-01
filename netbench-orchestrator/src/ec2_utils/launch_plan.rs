// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    ec2_utils::{
        instance, networking,
        types::{EndpointType, SubnetId, VpcId},
        Az, InfraDetail, InstanceDetail,
    },
    orchestrator::{OrchError, OrchResult, OrchestratorConfig},
};
use aws_sdk_ec2::types::Instance;
use std::{collections::HashMap, time::Duration};
use tracing::debug;

const WAIT_INSTANCE_LAUNCH: Duration = Duration::from_secs(10);

pub type NetworkingInfraDetail = HashMap<Az, SubnetId>;

// A collection of components necessary to launch all infrastructure
#[derive(Clone, Debug)]
pub struct LaunchPlan<'a> {
    pub ami_id: String,
    pub networking_detail: NetworkingInfraDetail,
    pub vpc_id: VpcId,
    pub instance_profile_arn: String,
    pub config: &'a OrchestratorConfig,
}

impl<'a> LaunchPlan<'a> {
    pub async fn create(
        ec2_client: &aws_sdk_ec2::Client,
        iam_client: &aws_sdk_iam::Client,
        ssm_client: &aws_sdk_ssm::Client,
        config: &'a OrchestratorConfig,
    ) -> OrchResult<Self> {
        let instance_profile_arn = instance::get_instance_profile(iam_client, config)
            .await
            .map_err(|err| OrchError::Ec2 {
                dbg: format!("{}", err),
            })?;
        let ami_id = instance::get_latest_ami(ssm_client)
            .await
            .map_err(|err| OrchError::Ec2 {
                dbg: format!("{}", err),
            })?;
        let (networking_detail, vpc_id) = networking::get_subnet_vpc_ids(ec2_client, config)
            .await
            .map_err(|err| OrchError::Ec2 {
                dbg: format!("{}", err),
            })?;
        Ok(LaunchPlan {
            ami_id,
            networking_detail,
            vpc_id,
            instance_profile_arn,
            config,
        })
    }

    pub async fn launch(
        &self,
        ec2_client: &aws_sdk_ec2::Client,
        unique_id: &str,
    ) -> OrchResult<InfraDetail> {
        debug!("{:?}", self);
        let security_group_id =
            networking::create_security_group(ec2_client, &self.vpc_id, unique_id)
                .await
                .map_err(|err| OrchError::Ec2 {
                    dbg: format!("{}", err),
                })?;

        // Create placement per az.
        //
        // Only cluster placement supported at the moment
        let mut placement_map = HashMap::new();
        for az in self.networking_detail.keys() {
            let placement = networking::create_placement_group(ec2_client, az, unique_id).await?;
            placement_map.insert(az.clone(), placement.clone());
        }

        let mut infra = InfraDetail {
            security_group_id,
            clients: Vec::new(),
            servers: Vec::new(),
            placement_map,
        };

        self.launch_host_group(ec2_client, EndpointType::Server, &mut infra, unique_id)
            .await?;
        self.launch_host_group(ec2_client, EndpointType::Client, &mut infra, unique_id)
            .await?;

        networking::set_routing_permissions(ec2_client, &infra).await?;

        // wait for instance to spawn
        tokio::time::sleep(WAIT_INSTANCE_LAUNCH).await;

        Ok(infra)
    }

    async fn launch_host_group(
        &self,
        ec2_client: &aws_sdk_ec2::Client,
        endpoint_type: EndpointType,
        infra: &mut InfraDetail,
        unique_id: &str,
    ) -> OrchResult<()> {
        let (instance_detail, host_config) = match endpoint_type {
            EndpointType::Server => (&mut infra.servers, &self.config.server_config),
            EndpointType::Client => (&mut infra.clients, &self.config.client_config),
        };

        let mut launch_requests = Vec::with_capacity(host_config.len());
        for host_config in host_config {
            let instance = instance::launch_instances(
                ec2_client,
                self,
                &infra.security_group_id,
                unique_id,
                host_config,
                &infra.placement_map,
                endpoint_type,
            )
            .await
            .map_err(|err| {
                debug!("{}", err);
                err
            });
            launch_requests.push(instance);
        }

        let launch_request: OrchResult<Vec<_>> = launch_requests.into_iter().collect();
        // FIXME make cleanup more resilient.
        if let Err(launch_err) = launch_request {
            let _ = infra.cleanup(ec2_client).await.map_err(|delete_err| {
                // ignore error on cleanup.. since this is best effort
                debug!("{}", delete_err);
            });

            return Err(launch_err);
        }

        let instances = launch_request.map_err(|err| OrchError::Ec2 {
            dbg: format!("{}", err),
        })?;

        self.resolve_ips(instances, ec2_client, endpoint_type, instance_detail)
            .await?;

        Ok(())
    }

    async fn resolve_ips(
        &self,
        instances: Vec<Instance>,
        ec2_client: &aws_sdk_ec2::Client,
        endpoint_type: EndpointType,
        instance_detail: &mut Vec<InstanceDetail>,
    ) -> OrchResult<()> {
        for (launch_id, server) in instances.into_iter().enumerate() {
            let server_ip =
                instance::poll_running(ec2_client, &server, launch_id, &endpoint_type).await?;
            let az = server
                .placement()
                .and_then(|placement| placement.availability_zone())
                .ok_or(OrchError::Ec2 {
                    dbg: "Failed to find placement".to_string(),
                })?;
            let server =
                InstanceDetail::new(endpoint_type, Az::from(az.to_string()), server, server_ip)?;
            instance_detail.push(server);
        }
        Ok(())
    }
}
