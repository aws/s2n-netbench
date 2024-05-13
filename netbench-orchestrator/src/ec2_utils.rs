// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    ec2_utils::types::{InstanceDetail, PubIp},
    orchestrator::{OrchError, OrchResult},
};
use aws_sdk_ec2::{error::SdkError, types::PlacementGroup};
use std::{collections::HashMap, time::Duration};
use tracing::{debug, error, info};

mod instance;
mod launch_plan;
mod networking;
mod types;

pub use types::{Az, PrivIp};

const MAX_RETRY_COUNT: usize = 25;
const RETRY_BACKOFF: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub struct InfraDetail {
    pub security_group_id: String,
    pub clients: Vec<InstanceDetail>,
    pub servers: Vec<InstanceDetail>,
    placement_map: HashMap<Az, PlacementGroup>,
}

impl InfraDetail {
    pub async fn cleanup(&self, ec2_client: &aws_sdk_ec2::Client) -> OrchResult<()> {
        // instances must be deleted before other infra
        self.delete_instances(ec2_client).await?;

        // attempt only after deleting instances
        self.delete_placement_group(ec2_client).await?;
        self.delete_security_group(ec2_client).await?;
        Ok(())
    }

    pub fn public_server_ips(&self) -> Vec<&PubIp> {
        self.servers
            .iter()
            .map(|instance| instance.host_ips().public_ip())
            .collect()
    }

    pub fn private_server_ips(&self) -> Vec<&PrivIp> {
        self.servers
            .iter()
            .map(|instance| instance.host_ips().private_ip())
            .collect()
    }

    pub fn public_client_ips(&self) -> Vec<&PubIp> {
        self.clients
            .iter()
            .map(|instance| instance.host_ips().public_ip())
            .collect()
    }
}

impl InfraDetail {
    async fn delete_instances(&self, ec2_client: &aws_sdk_ec2::Client) -> OrchResult<()> {
        info!("Start: deleting instances");
        let ids: Vec<String> = self
            .servers
            .iter()
            .chain(self.clients.iter())
            .map(|instance| instance.instance_id().to_string())
            .collect();

        ec2_client
            .terminate_instances()
            .set_instance_ids(Some(ids))
            .send()
            .await
            .map_err(|err| OrchError::Ec2 {
                dbg: err.to_string(),
            })?;

        Ok(())
    }

    // Attempt to delete the Security Group.
    //
    // Retry the operation if the resource is still 'in-use' (`DependencyViolation`). Since an
    // EC2 instance takes time to fully terminate, a Security Group could be 'in-use' until the
    // EC2 host is fully cleaned up.
    async fn delete_security_group(&self, ec2_client: &aws_sdk_ec2::Client) -> OrchResult<()> {
        info!("Start: deleting security groups");

        let mut retries = 0;
        while retries < MAX_RETRY_COUNT {
            let delete_security_group = ec2_client
                .delete_security_group()
                .group_id(self.security_group_id.to_string())
                .send()
                .await;
            debug!(
                "deleting security group. retry {retries}. result: {:?}",
                delete_security_group
            );

            match delete_security_group {
                Ok(_) => return Ok(()),
                Err(SdkError::ServiceError(service_err))
                    if service_err.err().meta().code() == Some("DependencyViolation") =>
                {
                    // retryable error
                    retries += 1;
                    tokio::time::sleep(RETRY_BACKOFF).await;
                    continue;
                }
                Err(err) => {
                    // non-retryable error
                    error!("abort deleting security group {}", self.security_group_id);
                    return Err(OrchError::Ec2 {
                        dbg: err.to_string(),
                    });
                }
            }
        }

        error!("abort deleting security group {}", self.security_group_id);
        Err(OrchError::Ec2 {
            dbg: "Failed to delete security group because it's still in use".to_string(),
        })
    }

    // Attempt to delete the Placement Group.
    //
    // Retry the operation if the resource is still 'in-use' (`InvalidPlacementGroup.InUse`). Since
    // an EC2 instance takes time to fully terminate, a Placement Group could be 'in-use' until the
    // EC2 host is fully cleaned up.
    async fn delete_placement_group(&self, ec2_client: &aws_sdk_ec2::Client) -> OrchResult<()> {
        info!("Start: deleting placement groups");

        for (_az, placement_group) in self.placement_map.iter() {
            let placement_group_name = placement_group.group_name().ok_or(OrchError::Ec2 {
                dbg: "Failed to get placement_group name".to_string(),
            })?;

            let mut retries = 0;
            while retries < MAX_RETRY_COUNT {
                let delete_placement_group = ec2_client
                    .delete_placement_group()
                    .group_name(placement_group_name)
                    .send()
                    .await;
                debug!(
                    "deleting placement group. retry {retries}. \nresult: {:?}",
                    delete_placement_group
                );

                match delete_placement_group {
                    Ok(_) => return Ok(()),
                    Err(SdkError::ServiceError(service_err))
                        if service_err.err().meta().code()
                            == Some("InvalidPlacementGroup.InUse") =>
                    {
                        // retryable error
                        retries += 1;
                        tokio::time::sleep(RETRY_BACKOFF).await;
                        continue;
                    }
                    Err(err) => {
                        // non-retryable error
                        error!("abort deleting placement group {:?}", placement_group);
                        return Err(OrchError::Ec2 {
                            dbg: err.to_string(),
                        });
                    }
                }
            }
        }

        error!("abort deleting placement groups");
        Err(OrchError::Ec2 {
            dbg: "Failed to delete placement group because it's still in-use".to_string(),
        })
    }
}
