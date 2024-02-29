// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use std::net::IpAddr;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct PubIp(pub IpAddr);
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct PrivIp(pub IpAddr);

impl std::fmt::Display for PrivIp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Display for PubIp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
