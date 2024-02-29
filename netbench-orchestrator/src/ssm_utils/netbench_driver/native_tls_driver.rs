// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{CrateIoSource, NetbenchDriverType};

pub fn native_tls_server_driver() -> NetbenchDriverType {
    let source = CrateIoSource {
        krate: "s2n-netbench-driver-native-tls".to_string(),
        driver_name: "s2n-netbench-driver-server-native-tls".to_string(),
        version: "*".to_string(),
    };
    NetbenchDriverType::CratesIo(source)
}

pub fn native_tls_client_driver() -> NetbenchDriverType {
    let source = CrateIoSource {
        krate: "s2n-netbench-driver-native-tls".to_string(),
        driver_name: "s2n-netbench-driver-client-native-tls".to_string(),
        version: "*".to_string(),
    };
    NetbenchDriverType::CratesIo(source)
}
