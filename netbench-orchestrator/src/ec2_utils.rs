// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    ec2_utils::instance::delete_instance,
    orchestrator::{OrchError, OrchResult},
};
use aws_sdk_ec2::types::PlacementGroup;
use std::{collections::HashMap, time::Duration};
use tracing::{debug, error, info};

mod instance;
mod launch_plan;
mod networking;

pub use instance::{EndpointType, InstanceDetail, PrivIp, PubIp};
pub use launch_plan::LaunchPlan;
pub use networking::Az;

#[derive(Debug)]
pub struct InfraDetail {
    pub security_group_id: String,
    pub clients: Vec<InstanceDetail>,
    pub servers: Vec<InstanceDetail>,
    placement_map: HashMap<Az, PlacementGroup>,
}

impl InfraDetail {
    pub async fn cleanup(&self, ec2_client: &aws_sdk_ec2::Client) -> OrchResult<()> {
        self.delete_instances(ec2_client).await?;
        self.delete_placement_group(ec2_client).await?;
        // generally takes a long time and has retries built-in so attempt this last
        self.delete_security_group(ec2_client).await?;
        Ok(())
    }

    pub fn public_server_ips(&self) -> Vec<&PubIp> {
        self.servers
            .iter()
            .map(|instance| instance.host_ips.public_ip())
            .collect()
    }

    pub fn private_server_ips(&self) -> Vec<&PrivIp> {
        self.servers
            .iter()
            .map(|instance| instance.host_ips.private_ip())
            .collect()
    }

    pub fn public_client_ips(&self) -> Vec<&PubIp> {
        self.clients
            .iter()
            .map(|instance| instance.host_ips.public_ip())
            .collect()
    }
}

impl InfraDetail {
    async fn delete_instances(&self, ec2_client: &aws_sdk_ec2::Client) -> OrchResult<()> {
        info!("Start: deleting instances");
        println!("Start: deleting instances");
        let ids: Vec<String> = self
            .servers
            .iter()
            .chain(self.clients.iter())
            .map(|instance| instance.instance_id().unwrap().to_string())
            .collect();

        delete_instance(ec2_client, ids).await?;
        Ok(())
    }

    async fn delete_security_group(&self, ec2_client: &aws_sdk_ec2::Client) -> OrchResult<()> {
        info!("Start: deleting security groups");
        println!("Start: deleting security groups");

        let retry_backoff = Duration::from_secs(5);
        let mut deleted_sec_group = ec2_client
            .delete_security_group()
            .group_id(self.security_group_id.to_string())
            .send()
            .await;
        tokio::time::sleep(retry_backoff).await;

        let mut retries = 25;
        while deleted_sec_group.is_err() && retries > 0 {
            debug!("deleting security group. retry {retries}");
            tokio::time::sleep(retry_backoff).await;
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
        println!("Start: deleting placement groups");

        let retry_backoff = Duration::from_secs(5);
        for (_k, v) in self.placement_map.iter() {
            let mut retries = 25;

            let placement_group_name = v.group_name().unwrap();
            debug!("Start: deleting placement group: {placement_group_name}");

            let mut delete_placement_group = ec2_client
                .delete_placement_group()
                .group_name(placement_group_name)
                .send()
                .await;

            while delete_placement_group.is_err() && retries > 0 {
                debug!("deleting placement group. retry {retries}");
                tokio::time::sleep(retry_backoff).await;
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
