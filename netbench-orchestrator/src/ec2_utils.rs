// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    ec2_utils::types::{InstanceDetail, PubIp},
    orchestrator::{OrchError, OrchResult},
};
use aws_sdk_ec2::types::PlacementGroup;
use std::{collections::HashMap, time::Duration};
use tracing::{debug, error, info};

mod instance;
mod launch_plan;
mod networking;
mod types;

pub use types::{Az, PrivIp};

const RETRY_COUNT: usize = 25;
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

        self.delete_placement_group(ec2_client).await?;
        // generally takes a long time so attempt this last
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

    async fn delete_security_group(&self, ec2_client: &aws_sdk_ec2::Client) -> OrchResult<()> {
        info!("Start: deleting security groups");

        let mut deleted_sec_group = ec2_client
            .delete_security_group()
            .group_id(self.security_group_id.to_string())
            .send()
            .await;

        let mut retries = RETRY_COUNT;
        while deleted_sec_group.is_err() && retries > 0 {
            debug!("deleting security group. retry {retries}");
            tokio::time::sleep(RETRY_BACKOFF).await;
            deleted_sec_group = ec2_client
                .delete_security_group()
                .group_id(self.security_group_id.to_string())
                .send()
                .await;

            retries -= 1;
        }

        deleted_sec_group.map_err(|err| {
            error!("abort deleting security group {}", self.security_group_id);
            OrchError::Ec2 {
                dbg: err.to_string(),
            }
        })?;

        Ok(())
    }

    async fn delete_placement_group(&self, ec2_client: &aws_sdk_ec2::Client) -> OrchResult<()> {
        info!("Start: deleting placement groups");

        for (_az, placement_group) in self.placement_map.iter() {
            let mut retries = RETRY_COUNT;

            let placement_group_name = placement_group.group_name().ok_or(OrchError::Ec2 {
                dbg: "Failed to get placement_group name".to_string(),
            })?;
            debug!("Start: deleting placement group: {placement_group_name}");

            let mut delete_placement_group = ec2_client
                .delete_placement_group()
                .group_name(placement_group_name)
                .send()
                .await;

            while delete_placement_group.is_err() && retries > 0 {
                debug!("deleting placement group. retry {retries}");
                tokio::time::sleep(RETRY_BACKOFF).await;
                delete_placement_group = ec2_client
                    .delete_placement_group()
                    .group_name(placement_group_name)
                    .send()
                    .await;

                retries -= 1;
            }
        }

        Ok(())
    }
}
