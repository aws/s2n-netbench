// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{orchestrator::OrchError, OrchResult};
use aws_sdk_s3 as s3;
use aws_sdk_s3::operation::{get_object::GetObjectOutput, put_object::PutObjectOutput};
use std::{fs::File, io::prelude::*, path::Path};

pub async fn download_object_to_file<P: AsRef<Path>>(
    client: &s3::Client,
    bucket_name: &str,
    key: &str,
    path: P,
) -> OrchResult<usize> {
    let path = path.as_ref();
    let mut file = File::create(path).map_err(|err| OrchError::Init {
        dbg: format!("failed to create file {:?}, {err}", path),
    })?;

    let mut obj = download_object(client, bucket_name, key).await?;

    // write to file
    let mut total_size = 0;
    while let Some(bytes) = obj.body.try_next().await.map_err(|err| OrchError::S3 {
        dbg: err.to_string(),
    })? {
        total_size += file.write(&bytes).map_err(|err| OrchError::Init {
            dbg: format!("failed to write to file {:?}, {err}", file),
        })?;
    }

    Ok(total_size)
}

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

async fn download_object(
    client: &s3::Client,
    bucket_name: &str,
    key: &str,
) -> OrchResult<GetObjectOutput> {
    client
        .get_object()
        .bucket(bucket_name)
        .key(key)
        .send()
        .await
        .map_err(|err| OrchError::S3 {
            dbg: err.to_string(),
        })
}
