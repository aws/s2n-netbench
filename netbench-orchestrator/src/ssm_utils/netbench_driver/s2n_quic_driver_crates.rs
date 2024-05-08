// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{CrateIoSource, NetbenchDriverType};

pub fn s2n_quic_server_driver() -> NetbenchDriverType {
    let source = CrateIoSource {
        krate: "s2n-netbench-driver-s2n-quic".to_string(),
        driver_name: "s2n-netbench-driver-server-s2n-quic".to_string(),
        version: "*".to_string(),
    };
    NetbenchDriverType::CratesIo(source)
}

pub fn s2n_quic_client_driver() -> NetbenchDriverType {
    let source = CrateIoSource {
        krate: "s2n-netbench-driver-s2n-quic".to_string(),
        driver_name: "s2n-netbench-driver-client-s2n-quic".to_string(),
        version: "*".to_string(),
    };
    NetbenchDriverType::CratesIo(source)
}
