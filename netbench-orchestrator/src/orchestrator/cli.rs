// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::orchestrator::{
    cli::types::{CdkConfig, CliInfraScenario, HostConfig, IntermediateCli, NetbenchScenario},
    OrchResult,
};
use clap::Parser;
use std::path::PathBuf;

mod types;

#[derive(Parser, Debug)]
pub struct Cli {
    /// Path to cdk parameter file
    #[arg(long, default_value = "cdk_config.json")]
    cdk_config_file: PathBuf,

    /// Path to the scenario file
    #[arg(long)]
    netbench_scenario_file: PathBuf,

    // An infrastructure overlay for the hosts specified in the
    // netbench scenario file
    #[command(flatten)]
    infra: CliInfraScenario,
}

impl Cli {
    pub fn process_config_files(self) -> OrchResult<IntermediateCli> {
        let (netbench_scenario, netbench_scenario_filename) =
            NetbenchScenario::from_file(&self.netbench_scenario_file)?;
        let cdk_config = CdkConfig::from_file(&self.cdk_config_file)?;

        Ok(IntermediateCli::new(
            cdk_config,
            netbench_scenario,
            netbench_scenario_filename,
            self.netbench_scenario_file,
            self.infra,
        ))
    }
}

#[derive(Clone, Debug)]
pub struct OrchestratorConfig {
    // netbench
    netbench_scenario_filename: String,
    netbench_scenario_filepath: PathBuf,

    // cdk
    pub cdk_config: CdkConfig,

    // infra
    pub client_config: Vec<HostConfig>,
    pub server_config: Vec<HostConfig>,
}

// impl OrchestratorConfig {
//     pub fn netbench_scenario_filename(&self) -> &str {
//         &self.netbench_scenario_filename
//     }

//     pub fn netbench_scenario_file_stem(&self) -> &str {
//         self.netbench_scenario_filepath
//             .as_path()
//             .file_stem()
//             .expect("expect scenario file")
//             .to_str()
//             .unwrap()
//     }

//     pub fn cf_url(&self, unique_id: &str) -> String {
//         format!(
//             "{}/{}",
//             self.cdk_config.netbench_cloudfront_distribution(),
//             unique_id
//         )
//     }

//     pub fn s3_path(&self, unique_id: &str) -> String {
//         format!(
//             "s3://{}/{}",
//             self.cdk_config.netbench_runner_public_s3_bucket(),
//             unique_id
//         )
//     }

//     pub fn s3_private_path(&self, unique_id: &str) -> String {
//         format!(
//             "s3://{}/{}",
//             self.cdk_config.netbench_runner_private_s3_bucket(),
//             unique_id
//         )
//     }
// }

// impl CdkConfig {
//     pub fn netbench_runner_public_s3_bucket(&self) -> &String {
//         &self.resources.output_netbench_runner_public_logs_bucket
//     }

//     pub fn netbench_runner_private_s3_bucket(&self) -> &String {
//         &self.resources.output_netbench_runner_private_src_bucket
//     }

//     pub fn netbench_cloudfront_distribution(&self) -> &String {
//         &self.resources.output_netbench_cloudfront_distribution
//     }

//     pub fn netbench_runner_log_group(&self) -> &String {
//         &self.resources.output_netbench_runner_log_group
//     }

//     pub fn netbench_runner_instance_profile(&self) -> &String {
//         &self.resources.output_netbench_runner_instance_profile
//     }

//     pub fn netbench_runner_subnet_tag_key(&self) -> String {
//         format!("tag:{}", self.resources.output_netbench_subnet_tag_key)
//     }

//     pub fn netbench_runner_subnet_tag_value(&self) -> &String {
//         &self.resources.output_netbench_subnet_tag_value
//     }

//     pub fn netbench_primary_region(&self) -> &String {
//         &self.resources.output_netbench_infra_primary_prod_region
//     }

//     fn from_file(cdk_config_file: &PathBuf) -> OrchResult<Self> {
//         let path = Path::new(&cdk_config_file);
//         let cdk_config_file = File::open(path).map_err(|_err| OrchError::Init {
//             dbg: format!("Scenario file not found: {:?}", path),
//         })?;
//         let config: CdkConfig = serde_json::from_reader(cdk_config_file).unwrap();
//         Ok(config)
//     }
// }
