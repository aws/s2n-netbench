// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    ec2_utils::{EndpointType, InstanceDetail},
    orchestrator::{OrchResult, OrchestratorConfig},
    s3_utils::upload_object,
};
use aws_sdk_s3::primitives::ByteStream;
use bytes::Bytes;
use tracing::info;

pub enum Step<'a> {
    UploadIndex,
    HostsRunning(&'a Vec<InstanceDetail>),
}

pub async fn update_dashboard(
    step: Step<'_>,
    s3_client: &aws_sdk_s3::Client,
    unique_id: &str,
    config: &OrchestratorConfig,
    endpoint_type: EndpointType,
) -> OrchResult<()> {
    match step {
        Step::UploadIndex => upload_index_html(s3_client, unique_id, config).await,
        Step::HostsRunning(instances) => {
            update_instance_running(s3_client, instances, unique_id, config, endpoint_type).await
        }
    }
}

async fn upload_index_html(
    s3_client: &aws_sdk_s3::Client,
    unique_id: &str,
    config: &OrchestratorConfig,
) -> OrchResult<()> {
    let cf_url = config.cf_url(unique_id);
    let status = format!("{}/index.html", cf_url);
    let template_server_prefix = format!("{}/server-step-", cf_url);
    let template_client_prefix = format!("{}/client-step-", cf_url);
    let template_finished_prefix = format!("{}/finished-step-", cf_url);

    let index_file = std::fs::read_to_string("index.html")
        .expect("index.html not found")
        .replace("template_unique_id", unique_id)
        .replace("template_server_prefix", &template_server_prefix)
        .replace("template_client_prefix", &template_client_prefix)
        .replace("template_finished_prefix", &template_finished_prefix);

    // Upload to s3
    upload_object(
        s3_client,
        config.cdk_config.netbench_runner_public_s3_bucket(),
        ByteStream::from(Bytes::from(index_file)),
        &format!("{unique_id}/index.html"),
    )
    .await?;

    println!("Status: URL: {status}");
    info!("Status: URL: {status}");

    Ok(())
}

async fn update_instance_running(
    s3_client: &aws_sdk_s3::Client,
    instances: &[InstanceDetail],
    unique_id: &str,
    config: &OrchestratorConfig,
    endpoint_type: EndpointType,
) -> OrchResult<()> {
    let instance_detail = instances
        .iter()
        .map(|instance| format!("{} {}", instance.host_ips(), instance.instance_id()))
        .collect::<Vec<String>>()
        .join(" - ");

    let payload = ByteStream::from(Bytes::from(format!(
        "EC2 {:?} instances up: {}",
        endpoint_type, instance_detail
    )));
    upload_object(
        s3_client,
        config.cdk_config.netbench_runner_public_s3_bucket(),
        payload,
        &format!(
            "{unique_id}/{}-step-0",
            endpoint_type.as_str().to_lowercase()
        ),
    )
    .await?;
    Ok(())
}
