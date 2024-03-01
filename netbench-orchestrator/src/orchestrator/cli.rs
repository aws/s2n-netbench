// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::orchestrator::{
    cli::types::{CdkConfig, CliInfraScenario, IntermediateCli, NetbenchScenario},
    OrchResult,
};
use clap::Parser;
use std::path::PathBuf;

mod types;

pub use types::HostConfig;

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
