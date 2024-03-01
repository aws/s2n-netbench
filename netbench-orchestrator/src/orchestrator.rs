// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

mod cli;
mod dashboard;
mod error;
mod report;
mod state;

use crate::{
    ec2_utils, ec2_utils::InfraDetail, s3_utils, ssm_utils, ssm_utils::NetbenchDriverType,
};
use aws_sdk_s3::primitives::ByteStream;
use tracing::info;

pub use cli::{Cli, HostConfig, OrchestratorConfig};
pub use error::{OrchError, OrchResult};
pub use state::STATE;

#[allow(dead_code)]
pub enum RunMode {
    // Skip the netbench run.
    //
    // Useful for testing infrastructure setup.
    TestInfra,

    Full,
}

pub async fn run(
    unique_id: String,
    config: &OrchestratorConfig,
    aws_config: &aws_types::SdkConfig,
    run_mode: RunMode,
) -> OrchResult<()> {
    let iam_client = aws_sdk_iam::Client::new(aws_config);
    let s3_client = aws_sdk_s3::Client::new(aws_config);
    let ec2_client = aws_sdk_ec2::Client::new(aws_config);
    let ssm_client = aws_sdk_ssm::Client::new(aws_config);

    upload_run_parameters_to_s3(&s3_client, config, &unique_id).await?;

    // Setup instances
    let infra = ec2_utils::LaunchPlan::create(&ec2_client, &iam_client, &ssm_client, config)
        .await?
        .launch(&ec2_client, &unique_id)
        .await?;

    update_dashboard_with_instances(&s3_client, config, &infra, &unique_id).await?;

    run_netbench(
        run_mode,
        config,
        &infra,
        &ssm_client,
        &s3_client,
        &unique_id,
    )
    .await?;

    // Cleanup
    infra
        .cleanup(&ec2_client)
        .await
        .map_err(|err| eprintln!("Failed to cleanup all resources. {err} {:?}", infra))
        .unwrap();

    Ok(())
}

async fn upload_run_parameters_to_s3(
    s3_client: &aws_sdk_s3::Client,
    config: &OrchestratorConfig,
    unique_id: &str,
) -> OrchResult<()> {
    let scenario_file = ByteStream::from_path(config.netbench_scenario_filepath())
        .await
        .map_err(|err| OrchError::Init {
            dbg: err.to_string(),
        })?;

    s3_utils::upload_object(
        s3_client,
        config.cdk_config.netbench_runner_public_s3_bucket(),
        scenario_file,
        &format!("{unique_id}/{}", config.netbench_scenario_filename()),
    )
    .await
    .unwrap();

    // upload the index.html dashboard file
    dashboard::upload_index_html(s3_client, unique_id, config).await?;

    Ok(())
}

async fn update_dashboard_with_instances(
    s3_client: &aws_sdk_s3::Client,
    config: &OrchestratorConfig,
    infra: &InfraDetail,
    unique_id: &str,
) -> OrchResult<()> {
    dashboard::update_instance_running(
        s3_client,
        infra,
        unique_id,
        config,
        ec2_utils::EndpointType::Server,
    )
    .await?;

    dashboard::update_instance_running(
        s3_client,
        infra,
        unique_id,
        config,
        ec2_utils::EndpointType::Client,
    )
    .await?;

    Ok(())
}

async fn run_netbench(
    run_mode: RunMode,
    config: &OrchestratorConfig,
    infra: &InfraDetail,
    ssm_client: &aws_sdk_ssm::Client,
    s3_client: &aws_sdk_s3::Client,
    unique_id: &str,
) -> OrchResult<()> {
    if matches!(run_mode, RunMode::Full) {
        let server_drivers = vec![
            ssm_utils::s2n_quic_dc_driver::dc_quic_server_driver(unique_id, config),
            ssm_utils::tcp_driver_crates::tcp_server_driver(),
            ssm_utils::s2n_quic_driver_crates::s2n_quic_server_driver(),
            ssm_utils::s2n_tls_driver::s2n_tls_server_driver(),
            // ssm_utils::native_tls_driver::native_tls_server_driver(),
        ];
        let client_drivers = vec![
            ssm_utils::s2n_quic_dc_driver::dc_quic_client_driver(unique_id, config),
            ssm_utils::tcp_driver_crates::tcp_client_driver(),
            ssm_utils::s2n_quic_driver_crates::s2n_quic_client_driver(),
            ssm_utils::s2n_tls_driver::s2n_tls_client_driver(),
            // ssm_utils::native_tls_driver::native_tls_client_driver(),
        ];

        assert_eq!(server_drivers.len(), client_drivers.len());

        configure_remote_hosts(
            config,
            infra,
            ssm_client,
            unique_id,
            &server_drivers,
            &client_drivers,
        )
        .await?;

        let driver_pairs = client_drivers.into_iter().zip(server_drivers);
        for (client_driver, server_driver) in driver_pairs {
            let msg = format!(
                "Running server: {} and client: {}",
                server_driver.driver_name(),
                client_driver.driver_name()
            );
            info!(msg);

            // run russula
            {
                let mut server_russula = ssm_utils::ServerNetbenchRussula::new(
                    ssm_client,
                    infra,
                    config,
                    &server_driver,
                )
                .await?;

                let mut client_russula = ssm_utils::ClientNetbenchRussula::new(
                    ssm_client,
                    infra,
                    config,
                    &client_driver,
                )
                .await?;

                // run client/server
                server_russula.wait_netbench_running(ssm_client).await?;
                client_russula.wait_done(ssm_client).await?;
                server_russula.wait_done(ssm_client).await?;
            }

            copy_netbench_results_to_s3(
                config,
                infra,
                ssm_client,
                unique_id,
                &server_driver,
                &client_driver,
            )
            .await?;
        }

        report::generate_report(s3_client, unique_id, infra, config).await?;
    }

    Ok(())
}

async fn configure_remote_hosts(
    config: &OrchestratorConfig,
    infra: &InfraDetail,
    ssm_client: &aws_sdk_ssm::Client,
    unique_id: &str,
    server_drivers: &Vec<NetbenchDriverType>,
    client_drivers: &Vec<NetbenchDriverType>,
) -> OrchResult<()> {
    let client_ids = infra.client_ids();
    let server_ids = infra.server_ids();

    let mut build_cmds = ssm_utils::common::collect_config_cmds(
        "server",
        ssm_client,
        server_ids.clone(),
        config,
        server_drivers,
        unique_id,
        config,
    )
    .await;
    let client_build_cmds = ssm_utils::common::collect_config_cmds(
        "client",
        ssm_client,
        client_ids.clone(),
        config,
        client_drivers,
        unique_id,
        config,
    )
    .await;
    build_cmds.extend(client_build_cmds);
    ssm_utils::common::wait_complete(
        "Setup hosts: update and install dependencies",
        ssm_client,
        build_cmds,
    )
    .await;

    info!("Host setup Successful");
    Ok(())
}

async fn copy_netbench_results_to_s3(
    config: &OrchestratorConfig,
    infra: &InfraDetail,
    ssm_client: &aws_sdk_ssm::Client,
    unique_id: &str,
    server_driver: &NetbenchDriverType,
    client_driver: &NetbenchDriverType,
) -> OrchResult<()> {
    let client_ids = infra.client_ids();
    let server_ids = infra.server_ids();

    let copy_server_netbench = ssm_utils::common::upload_netbench_data_to_s3(
        ssm_client,
        server_ids.clone(),
        unique_id,
        config,
        server_driver,
    )
    .await;
    let copy_client_netbench = ssm_utils::common::upload_netbench_data_to_s3(
        ssm_client,
        client_ids.clone(),
        unique_id,
        config,
        client_driver,
    )
    .await;
    let msg = format!(
        "copy netbench results to s3 for drivers: {}, {}",
        server_driver.trim_driver_name(),
        client_driver.trim_driver_name()
    );
    ssm_utils::common::wait_complete(
        &msg,
        ssm_client,
        vec![copy_server_netbench, copy_client_netbench],
    )
    .await;
    info!("client_server netbench copy results!: Successful");

    Ok(())
}
