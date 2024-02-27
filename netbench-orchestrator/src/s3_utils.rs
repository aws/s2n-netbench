// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use aws_sdk_s3 as s3;
use aws_sdk_s3::{
    error::SdkError,
    operation::{
        get_object::{GetObjectError, GetObjectOutput},
        put_object::{PutObjectError, PutObjectOutput},
    },
};
use std::{fs::File, io::prelude::*, path::Path};
use tokio_stream::StreamExt;

pub async fn download_object_to_file<P: AsRef<Path>>(
    client: &s3::Client,
    bucket_name: &str,
    key: &str,
    path: P,
) -> Result<usize, SdkError<GetObjectError>> {
    let mut file = File::create(path).unwrap();

    let mut obj = download_object(client, bucket_name, key).await.unwrap();

    let mut total_size = 0;
    while let Some(bytes) = obj.body.try_next().await.unwrap() {
        total_size += file.write(&bytes).unwrap();
    }

    Ok(total_size)
}

pub async fn download_object(
    client: &s3::Client,
    bucket_name: &str,
    key: &str,
) -> Result<GetObjectOutput, SdkError<GetObjectError>> {
    client
        .get_object()
        .bucket(bucket_name)
        .key(key)
        .send()
        .await
}

pub async fn upload_object(
    client: &s3::Client,
    bucket_name: &str,
    body: s3::primitives::ByteStream,
    key: &str,
) -> Result<PutObjectOutput, SdkError<PutObjectError>> {
    client
        .put_object()
        .bucket(bucket_name)
        .key(key)
        .content_type("text/html")
        .body(body)
        .send()
        .await
}
