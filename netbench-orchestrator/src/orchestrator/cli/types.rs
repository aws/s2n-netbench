// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    ec2_utils::Az,
    orchestrator::{OrchError, OrchResult, OrchestratorConfig, STATE},
};
use aws_sdk_ec2::types::{Placement as AwsPlacement, PlacementGroup};
use clap::Args;
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::HashMap,
    fs::File,
    path::{Path, PathBuf},
    process::Command,
};
use tracing::debug;

// Parse the netbench and cdk config files
pub struct IntermediateCli {
    cdk_config: CdkConfig,
    netbench_scenario: NetbenchScenario,
    netbench_scenario_filename: String,
    netbench_scenario_filepath: PathBuf,
    infra: CliInfraScenario,
}

impl IntermediateCli {
    pub fn new(
        cdk_config: CdkConfig,
        netbench_scenario: NetbenchScenario,
        netbench_scenario_filename: String,
        netbench_scenario_filepath: PathBuf,
        infra: CliInfraScenario,
    ) -> Self {
        IntermediateCli {
            cdk_config,
            netbench_scenario,
            netbench_scenario_filename,
            netbench_scenario_filepath,
            infra,
        }
    }

    pub fn region(&self) -> String {
        self.cdk_config.netbench_primary_region().to_string()
    }

    // Perform sanity checks on user environment and input
    //
    // Things to check for include:
    // - user arguments are sanity
    // - s2n-netbench utility is installed
    // - aws cli is installed
    // - aws credentials are loaded
    // - local report directory has been created
    pub async fn check_requirements(
        self,
        aws_config: &aws_types::SdkConfig,
    ) -> OrchResult<OrchestratorConfig> {
        let scenario = self.netbench_scenario;
        let netbench_scenario_filename = self.netbench_scenario_filename;
        let cdk_config = self.cdk_config;

        // AZ
        assert_eq!(
            self.infra.server_az.len(),
            scenario.servers.len(),
            "AZ overlay should match the number of server hosts in the netbench scenario"
        );
        assert_eq!(
            self.infra.client_az.len(),
            scenario.clients.len(),
            "AZ overlay should match the number of client hosts in the netbench scenario"
        );
        // Placement
        assert!(
            self.infra.server_placement.is_empty()
                || self.infra.server_placement.len() == scenario.servers.len(),
            "Placement overlay should be empty or match the number of client hosts in the netbench scenario"
        );
        assert!(
            self.infra.client_placement.is_empty()
                || self.infra.client_placement.len() == scenario.clients.len(),
            "Placement overlay should be empty or match the number of client hosts in the netbench scenario"
        );

        let mut client_config = Vec::with_capacity(self.infra.client_az.len());
        for (i, az) in self.infra.client_az.into_iter().enumerate() {
            let placement = self
                .infra
                .client_placement
                .get(i)
                .unwrap_or(&PlacementGroupConfig::Unspecified);
            client_config.push(HostConfig::new(
                cdk_config.netbench_primary_region(),
                az,
                placement.clone(),
            ));
        }
        let mut server_config = Vec::with_capacity(self.infra.server_az.len());
        for (i, az) in self.infra.server_az.into_iter().enumerate() {
            let placement = self
                .infra
                .server_placement
                .get(i)
                .unwrap_or(&PlacementGroupConfig::Unspecified);
            server_config.push(HostConfig::new(
                cdk_config.netbench_primary_region(),
                az,
                placement.clone(),
            ));
        }

        let config = OrchestratorConfig {
            netbench_scenario_filename,
            netbench_scenario_filepath: self.netbench_scenario_filepath,
            client_config,
            server_config,
            cdk_config,
        };
        debug!("{:?}", config);

        Command::new("s2n-netbench")
            .output()
            .map_err(|_err| OrchError::Init {
                dbg: "Missing `s2n-netbench` cli. Please the Getting started section in the Readme"
                    .to_string(),
            })?;

        Command::new("aws")
            .output()
            .map_err(|_err| OrchError::Init {
                dbg: "Missing `aws` cli.".to_string(),
            })?;

        let iam_client = aws_sdk_iam::Client::new(aws_config);
        iam_client
            .list_roles()
            .send()
            .await
            .map_err(|_err| OrchError::Init {
                dbg: "Missing AWS credentials.".to_string(),
            })?;

        // report folder
        std::fs::create_dir_all(STATE.workspace_dir).map_err(|_err| OrchError::Init {
            dbg: "Failed to create local workspace".to_string(),
        })?;

        Ok(config)
    }
}

#[derive(Clone, Debug)]
pub struct HostConfig {
    pub az: String,
    instance_type: String,
    placement: PlacementGroupConfig,
}

impl HostConfig {
    fn new(region: &str, az: String, placement: PlacementGroupConfig) -> Self {
        assert!(
            az.starts_with(region),
            "User specified AZ: {} is not in the region: {}",
            az,
            region
        );
        HostConfig {
            az,
            instance_type: "c5.4xlarge".to_owned(),
            placement,
        }
    }

    pub fn instance_type(&self) -> &String {
        &self.instance_type
    }

    pub fn to_ec2_placement(
        &self,
        placement_map: &HashMap<Az, PlacementGroup>,
    ) -> OrchResult<AwsPlacement> {
        let mut aws_placement = AwsPlacement::builder();

        // set placement group
        match self.placement {
            PlacementGroupConfig::Unspecified => {
                debug!("unspecified placement group");
            }
            PlacementGroupConfig::Cluster => {
                debug!("cluster placement group specified");
                let az = Az::from(self.az.clone());
                let placement_group = placement_map.get(&az).expect("placement group not found");

                aws_placement = aws_placement.group_name(
                    placement_group
                        .group_name()
                        .expect("placement group_name not found"),
                );
            }
        };

        // Set AZ. This is also set when chooing the subnet for the instance
        aws_placement = aws_placement.availability_zone(&self.az);

        Ok(aws_placement.build())
    }
}

// Placement strategy for a cluster of EC2 hosts.
//
// Only cluster placement supported at the moment. Placement groups are created per run.
// https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/placement-groups.html?icmpid=docs_ec2_console
#[derive(Clone, Debug, Default, clap::ValueEnum)]
enum PlacementGroupConfig {
    #[default]
    Unspecified,

    // Packs instances close together inside an Availability Zone. This
    // strategy enables workloads to achieve the low-latency network
    // performance necessary for tightly-coupled node-to-node communication
    // that is typical of high-performance computing (HPC) applications.
    Cluster,
}

// Used for parsing the scenario file generated by the s2n-netbench project
#[derive(Clone, Debug, Default, Deserialize)]
pub struct NetbenchScenario {
    pub clients: Vec<Value>,
    pub servers: Vec<Value>,
}

impl NetbenchScenario {
    pub fn from_file(netbench_scenario_file: &PathBuf) -> OrchResult<(Self, String)> {
        let path = Path::new(&netbench_scenario_file);
        let name = path
            .file_name()
            .and_then(|f| f.to_str())
            .ok_or(OrchError::Init {
                dbg: "Scenario file not specified".to_string(),
            })?
            .to_string();
        let netbench_scenario_file = File::open(path).map_err(|_err| OrchError::Init {
            dbg: format!("Scenario file not found: {:?}", path),
        })?;
        let scenario: NetbenchScenario =
            serde_json::from_reader(netbench_scenario_file).map_err(|err| OrchError::Init {
                dbg: format!("Failed to parse netbench file. {err}"),
            })?;
        Ok((scenario, name))
    }
}

#[derive(Clone, Debug, Default, Args)]
pub struct CliInfraScenario {
    /// Placement strategy for the netbench client hosts
    #[arg(long, value_delimiter = ',')]
    client_placement: Vec<PlacementGroupConfig>,

    /// Placement strategy for the netbench server hosts
    #[arg(long, value_delimiter = ',')]
    server_placement: Vec<PlacementGroupConfig>,

    /// AZ placement for the netbench server hosts
    #[arg(long, value_delimiter = ',')]
    client_az: Vec<String>,

    /// AZ placement for the netbench server hosts
    #[arg(long, value_delimiter = ',')]
    server_az: Vec<String>,
}

// Used for parsing the scenario file generated by the s2n-netbench project
#[derive(Clone, Debug, Default, Deserialize)]
pub struct CdkConfig {
    #[serde(rename(deserialize = "NetbenchInfraPrimaryProd"))]
    resources: CdkResources,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
struct CdkResources {
    // CloudWatch log group name
    output_netbench_runner_log_group: String,
    // public netbench s3 bucket
    //
    // Used for storing artifacts for the orchestrator and netbench runs
    output_netbench_runner_public_logs_bucket: String,
    // private netbench s3 bucket
    //
    // Used for storing source based netbench drivers which could be private
    output_netbench_runner_private_src_bucket: String,
    // CloudFront stack which serves the netbench reports
    output_netbench_cloudfront_distribution: String,
    // EC2 instance profile for orchestrator hosts
    output_netbench_runner_instance_profile: String,
    // Subnet tag/key values
    //
    // The orchestrator supports multi-AZ runs and expects the a provisioned
    // subnet per AZ. We discover these subnets via a tag key/value pair, which
    // should be the same for all Subnets.
    output_netbench_subnet_tag_key: String,
    output_netbench_subnet_tag_value: String,
    // AWS region where the netbench orchestrator will run
    output_netbench_infra_primary_prod_region: String,
}

impl OrchestratorConfig {
    pub fn netbench_scenario_filename(&self) -> &str {
        &self.netbench_scenario_filename
    }

    pub fn netbench_scenario_file_stem(&self) -> &str {
        self.netbench_scenario_filepath
            .as_path()
            .file_stem()
            .expect("expect scenario file")
            .to_str()
            .unwrap()
    }

    pub fn cf_url(&self, unique_id: &str) -> String {
        format!(
            "{}/{}",
            self.cdk_config.netbench_cloudfront_distribution(),
            unique_id
        )
    }

    pub fn s3_path(&self, unique_id: &str) -> String {
        format!(
            "s3://{}/{}",
            self.cdk_config.netbench_runner_public_s3_bucket(),
            unique_id
        )
    }

    pub fn s3_private_path(&self, unique_id: &str) -> String {
        format!(
            "s3://{}/{}",
            self.cdk_config.netbench_runner_private_s3_bucket(),
            unique_id
        )
    }
}

impl CdkConfig {
    pub fn netbench_runner_public_s3_bucket(&self) -> &String {
        &self.resources.output_netbench_runner_public_logs_bucket
    }

    pub fn netbench_runner_private_s3_bucket(&self) -> &String {
        &self.resources.output_netbench_runner_private_src_bucket
    }

    pub fn netbench_cloudfront_distribution(&self) -> &String {
        &self.resources.output_netbench_cloudfront_distribution
    }

    pub fn netbench_runner_log_group(&self) -> &String {
        &self.resources.output_netbench_runner_log_group
    }

    pub fn netbench_runner_instance_profile(&self) -> &String {
        &self.resources.output_netbench_runner_instance_profile
    }

    pub fn netbench_runner_subnet_tag_key(&self) -> String {
        // https://docs.rs/aws-sdk-ec2/latest/aws_sdk_ec2/operation/describe_subnets/builders/struct.DescribeSubnetsFluentBuilder.html#method.filters
        // "EC2 api requires the `tag:` prefix when specifying tags"
        format!("tag:{}", self.resources.output_netbench_subnet_tag_key)
    }

    pub fn netbench_runner_subnet_tag_value(&self) -> &String {
        &self.resources.output_netbench_subnet_tag_value
    }

    pub fn netbench_primary_region(&self) -> &String {
        &self.resources.output_netbench_infra_primary_prod_region
    }

    pub fn from_file(cdk_config_file: &PathBuf) -> OrchResult<Self> {
        let path = Path::new(&cdk_config_file);
        let cdk_config_file = File::open(path).map_err(|_err| OrchError::Init {
            dbg: format!("Scenario file not found: {:?}", path),
        })?;
        let config: CdkConfig = serde_json::from_reader(cdk_config_file).unwrap();
        Ok(config)
    }
}
