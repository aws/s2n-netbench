// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::orchestrator::{OrchError, OrchResult, RunMode, STATE};
use aws_config::BehaviorVersion;
use aws_types::region::Region;
use clap::Parser;
use tracing_subscriber::EnvFilter;

mod ec2_utils;
mod orchestrator;
mod russula;
mod s3_utils;
mod ssm_utils;

#[tokio::main(flavor = "current_thread")]
async fn main() -> OrchResult<()> {
    let unique_id = format!(
        "{}-{}",
        humantime::format_rfc3339_seconds(std::time::SystemTime::now()),
        STATE.version
    );

    let file_appender =
        tracing_appender::rolling::daily("./target", format!("russula_{}", unique_id));
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(non_blocking)
        .init();

    let cli = orchestrator::Cli::parse().process_config_files()?;
    let region = Region::new(cli.region());
    let aws_config = aws_config::defaults(BehaviorVersion::latest())
        .region(region)
        .load()
        .await;

    // perform sanity and check before proceeding
    let config = cli.check_requirements(&aws_config).await?;

    orchestrator::run(unique_id, &config, &aws_config, RunMode::Full).await
}
