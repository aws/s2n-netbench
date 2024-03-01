// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

mod cli;
mod dashboard;
mod error;
mod report;
mod state;

pub use cli::{Cli, HostConfig, OrchestratorConfig};
pub use error::{OrchError, OrchResult};
pub use state::STATE;

pub enum RunMode {
    // Skip the netbench run.
    //
    // Useful for testing infrastructure setup.
    TestInfra,

    Full,
}

pub async fn run(
    _unique_id: String,
    _config: &OrchestratorConfig,
    _aws_config: &aws_types::SdkConfig,
    _run_mode: RunMode,
) -> OrchResult<()> {
    Ok(())
}
