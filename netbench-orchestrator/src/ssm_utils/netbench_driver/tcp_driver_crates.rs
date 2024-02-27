// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{CrateIoSource, NetbenchDriverType};

pub fn tcp_server_driver() -> NetbenchDriverType {
    let source = CrateIoSource {
        krate: "s2n-netbench-driver-tcp".to_string(),
        driver_name: "s2n-netbench-driver-server-tcp".to_string(),
        version: "*".to_string(),
    };
    NetbenchDriverType::CratesIo(source)
}

pub fn tcp_client_driver() -> NetbenchDriverType {
    let source = CrateIoSource {
        krate: "s2n-netbench-driver-tcp".to_string(),
        driver_name: "s2n-netbench-driver-client-tcp".to_string(),
        version: "*".to_string(),
    };
    NetbenchDriverType::CratesIo(source)
}
