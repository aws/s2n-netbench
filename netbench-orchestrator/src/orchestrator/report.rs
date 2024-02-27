// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    orchestrator::{OrchestratorConfig, STATE},
    s3_utils::*,
    InfraDetail,
};
use aws_sdk_s3::primitives::{ByteStream, SdkBody};
use std::{path::Path, process::Command};
use tempdir::TempDir;
use tracing::{debug, info, trace};

pub async fn orch_generate_report(
    s3_client: &aws_sdk_s3::Client,
    unique_id: &str,
    infra: &InfraDetail,
    config: &OrchestratorConfig,
) {
    let tmp_dir = TempDir::new(unique_id).unwrap().into_path();
    let tmp_dir = tmp_dir.to_str().unwrap();

    // download results from s3 -----------------------
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

    // CLI ---------------------------
    let results_path = format!("{}/results", tmp_dir);
    let report_path = format!("{}/report", tmp_dir);
    let mut cmd = Command::new("s2n-netbench");
    cmd.args(["report-tree", &results_path, &report_path]);
    debug!("{:?}", cmd);
    let status = cmd.status().expect("s2n-netbench command failed");
    assert!(status.success(), " s2n-netbench command failed");

    // upload report to s3 -----------------------
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

    update_report_url(s3_client, unique_id, config).await;

    println!("Report Finished!: Successful: true");
    println!("URL: {}/report/index.html", STATE.cf_url(unique_id, config));
    info!("Report Finished!: Successful: true");
    info!("URL: {}/report/index.html", STATE.cf_url(unique_id, config));

    let get_logs = true;
    if get_logs {
        infra.public_client_ips().iter().for_each(|ip| {
            let log_folder = format!("./target/logs/{unique_id}/client_{ip}");
            std::fs::create_dir_all(Path::new(&log_folder)).expect("create log dir");
            let out = Command::new("scp")
                .args([
                    "-oStrictHostKeyChecking=no",
                    &format!("ec2-user@{ip}:netbench_orchestrator/target/russula*"),
                    &log_folder,
                ])
                .output()
                .unwrap();
            debug!("{}", out.status);
            debug!("{:?}", out.stdout);
        });
        infra.public_server_ips().iter().for_each(|ip| {
            let log_folder = format!("./target/logs/{unique_id}/server_{ip}");
            std::fs::create_dir_all(Path::new(&log_folder)).expect("create log dir");
            Command::new("scp")
                .args([
                    "-oStrictHostKeyChecking=no",
                    &format!("ec2-user@{ip}:netbench_orchestrator/target/russula*"),
                    &log_folder,
                ])
                .output()
                .unwrap();
        });
    }
}

async fn update_report_url(
    s3_client: &aws_sdk_s3::Client,
    unique_id: &str,
    config: &OrchestratorConfig,
) {
    let body = ByteStream::new(SdkBody::from(format!(
        "<a href=\"{}/report/index.html\">Final Report</a>",
        STATE.cf_url(unique_id, config)
    )));
    let key = format!("{}/finished-step-0", unique_id);
    let _ = upload_object(
        s3_client,
        config.cdk_config.netbench_runner_public_s3_bucket(),
        body,
        &key,
    )
    .await
    .unwrap();
}
