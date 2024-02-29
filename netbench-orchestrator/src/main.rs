// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

// TODO remove
#![allow(dead_code)]

use crate::orchestrator::OrchResult;

mod ec2_utils;
mod orchestrator;
mod s3_utils;
mod ssm_utils;

#[tokio::main(flavor = "current_thread")]
async fn main() -> OrchResult<()> {
    Ok(())
}
