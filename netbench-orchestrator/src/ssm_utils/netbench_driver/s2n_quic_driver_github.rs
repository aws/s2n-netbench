// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{GithubSource, NetbenchDriverType};

pub fn s2n_quic_server_driver() -> NetbenchDriverType {
    let proj_name = "s2n-netbench".to_string();

    let source = GithubSource {
        driver_name: "s2n-netbench-driver-server-s2n-quic".to_string(),
        repo_name: proj_name.clone(),
    };
    NetbenchDriverType::GithubRustProj(source)
}

pub fn s2n_quic_client_driver() -> NetbenchDriverType {
    let proj_name = "s2n-netbench".to_string();

    let source = GithubSource {
        driver_name: "s2n-netbench-driver-client-s2n-quic".to_string(),
        repo_name: proj_name.clone(),
    };
    NetbenchDriverType::GithubRustProj(source)
}
