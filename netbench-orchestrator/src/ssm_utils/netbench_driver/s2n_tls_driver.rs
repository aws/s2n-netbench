// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{CrateIoSource, NetbenchDriverType};

pub fn s2n_tls_server_driver() -> NetbenchDriverType {
    let source = CrateIoSource {
        krate: "s2n-netbench-driver-s2n-tls".to_string(),
        driver_name: "s2n-netbench-driver-server-s2n-tls".to_string(),
        version: "*".to_string(),
    };
    NetbenchDriverType::CratesIo(source)
}

pub fn s2n_tls_client_driver() -> NetbenchDriverType {
    let source = CrateIoSource {
        krate: "s2n-netbench-driver-s2n-tls".to_string(),
        driver_name: "s2n-netbench-driver-client-s2n-tls".to_string(),
        version: "*".to_string(),
    };
    NetbenchDriverType::CratesIo(source)
}
