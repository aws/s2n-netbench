// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{orchestrator::OrchError, OrchResult};
use aws_sdk_s3 as s3;
use aws_sdk_s3::operation::put_object::PutObjectOutput;

pub async fn upload_object(
    client: &s3::Client,
    bucket_name: &str,
    body: s3::primitives::ByteStream,
    key: &str,
) -> OrchResult<PutObjectOutput> {
    client
        .put_object()
        .bucket(bucket_name)
        .key(key)
        .content_type("text/html")
        .body(body)
        .send()
        .await
        .map_err(|err| OrchError::S3 {
            dbg: err.to_string(),
        })
}
