// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{ec2_utils::InfraDetail, orchestrator::OrchestratorConfig, s3_utils, OrchResult};
use aws_sdk_s3::primitives::{ByteStream, SdkBody};
use std::{path::Path, process::Command};
use tracing::{debug, info, trace};

pub async fn generate_report(
    s3_client: &aws_sdk_s3::Client,
    unique_id: &str,
    infra: &InfraDetail,
    config: &OrchestratorConfig,
) -> OrchResult<()> {
    let tmp_dir = tempfile::Builder::new()
        .prefix(unique_id)
        .tempdir()
        .expect("failed to create temp dir")
        .into_path();
    let tmp_dir = tmp_dir.to_str().expect("failed to create temp dir");

    download_results(unique_id, config, tmp_dir).await?;
    generate_report_from_results(tmp_dir).await?;
    upload_report_to_s3(unique_id, config, tmp_dir).await?;
    update_report_url(s3_client, unique_id, config).await?;

    println!("Report Finished!: Successful: true");
    println!("URL: {}/report/index.html", config.cf_url(unique_id));
    info!("Report Finished!: Successful: true");
    info!("URL: {}/report/index.html", config.cf_url(unique_id));

    download_remote_logs(unique_id, infra);

    Ok(())
}

async fn upload_report_to_s3(
    unique_id: &str,
    config: &OrchestratorConfig,
    tmp_dir: &str,
) -> OrchResult<()> {
    let mut cmd = Command::new("aws");
    let output = cmd
        .args([
            "s3",
            "sync",
            tmp_dir,
            &format!(
                "s3://{}/{}",
                config.cdk_config.netbench_runner_public_s3_bucket(),
                unique_id
            ),
        ])
        .output()
        .unwrap();

    debug!("{:?}", cmd);
    trace!("{:?}", output);
    assert!(cmd.status().expect("aws sync").success(), "aws sync");
    Ok(())
}

async fn generate_report_from_results(tmp_dir: &str) -> OrchResult<()> {
    let results_path = format!("{}/results", tmp_dir);
    let report_path = format!("{}/report", tmp_dir);
    let mut cmd = Command::new("s2n-netbench");
    cmd.args(["report-tree", &results_path, &report_path]);
    debug!("{:?}", cmd);
    let status = cmd.status().expect("s2n-netbench command failed");
    assert!(status.success(), " s2n-netbench command failed");

    Ok(())
}

async fn download_results(
    unique_id: &str,
    config: &OrchestratorConfig,
    tmp_dir: &str,
) -> OrchResult<()> {
    let mut cmd = Command::new("aws");
    let output = cmd
        .args([
            "s3",
            "sync",
            &format!(
                "s3://{}/{}",
                config.cdk_config.netbench_runner_public_s3_bucket(),
                unique_id
            ),
            tmp_dir,
        ])
        .output()
        .unwrap();
    debug!("{:?}", cmd);
    trace!("{:?}", output);
    assert!(cmd.status().expect("aws sync").success(), "aws sync");

    Ok(())
}

async fn update_report_url(
    s3_client: &aws_sdk_s3::Client,
    unique_id: &str,
    config: &OrchestratorConfig,
) -> OrchResult<()> {
    let body = ByteStream::new(SdkBody::from(format!(
        "<a href=\"{}/report/index.html\">Final Report</a>",
        config.cf_url(unique_id)
    )));
    let key = format!("{}/finished-step-0", unique_id);
    s3_utils::upload_object(
        s3_client,
        config.cdk_config.netbench_runner_public_s3_bucket(),
        body,
        &key,
    )
    .await?;
    Ok(())
}

// This function is best effort and will not return an error.
//
// Requires ssh access to the host. See STATE.ssh_key_name for more info
fn download_remote_logs(unique_id: &str, infra: &InfraDetail) {
    // get logs
    let get_logs = true;
    if get_logs {
        infra.public_client_ips().iter().for_each(|ip| {
            let log_folder = format!("./target/logs/{unique_id}/client_{ip}");
            std::fs::create_dir_all(Path::new(&log_folder)).expect("create log dir");
            let res = Command::new("scp")
                .args([
                    "-oStrictHostKeyChecking=no",
                    &format!("ec2-user@{ip}:netbench_orchestrator/target/russula*"),
                    &log_folder,
                ])
                .output();
            debug!("client log download succeeded: {:?}", res.ok());
        });

        infra.public_server_ips().iter().for_each(|ip| {
            let log_folder = format!("./target/logs/{unique_id}/server_{ip}");
            std::fs::create_dir_all(Path::new(&log_folder)).expect("create log dir");
            let res = Command::new("scp")
                .args([
                    "-oStrictHostKeyChecking=no",
                    &format!("ec2-user@{ip}:netbench_orchestrator/target/russula*"),
                    &log_folder,
                ])
                .output();

            debug!("server log download succeeded: {:?}", res.ok());
        });
    }
}
