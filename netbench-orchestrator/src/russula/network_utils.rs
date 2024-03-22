// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::{RussulaError, RussulaResult};
use bytes::Bytes;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tracing::error;

// Msg.len is represented as a u16
const LEN_PREFIX_BYTES: usize = 2;

pub async fn recv_msg(stream: &mut TcpStream) -> RussulaResult<Msg> {
    stream.readable().await.map_err(log_fatal_error)?;
    read_msg(stream).await
}

async fn read_msg(stream: &mut TcpStream) -> RussulaResult<Msg> {
    let mut len_buf = [0; LEN_PREFIX_BYTES];
    let bytes_read = stream.read_exact(&mut len_buf).await?;
    let expected_payload_len = match bytes_read {
        LEN_PREFIX_BYTES => u16::from_be_bytes(len_buf) as usize,
        _ => {
            let dbg = "read returned 0 bytes.. read closed?".to_string();
            error!(dbg);
            return Err(RussulaError::ReadFail { dbg });
        }
    };

    let mut data = vec![0; expected_payload_len];
    let bytes_read = stream
        .read_exact(&mut data)
        .await
        .map_err(log_fatal_error)?;

    match bytes_read {
        bytes_read if bytes_read == expected_payload_len => Msg::new(data.into()),
        bytes_read => {
            let received_data =
                std::str::from_utf8(&data).unwrap_or("Unable to parse bytes as str!!");
            let dbg = format!(
                "read len: {} but expected_len: {}. data: {}",
                bytes_read, expected_payload_len, received_data
            );
            error!(dbg);
            Err(RussulaError::ReadFail { dbg })
        }
    }
}

pub async fn send_msg(stream: &mut TcpStream, msg: Msg) -> RussulaResult<usize> {
    stream.writable().await.map_err(log_fatal_error)?;
    write_msg(stream, msg).await
}

async fn write_msg(stream: &mut TcpStream, msg: Msg) -> RussulaResult<usize> {
    let data = construct_payload(msg);
    stream.write_all(&data).await?;
    stream.flush().await?;
    Ok(data.len())
}

fn construct_payload(msg: Msg) -> Vec<u8> {
    // size of len prefix + size of msg.data
    let payload_len = LEN_PREFIX_BYTES + msg.len as usize;
    let mut data: Vec<u8> = Vec::with_capacity(payload_len);
    data.extend(msg.len.to_be_bytes());
    data.extend(msg.data);
    data
}

fn log_fatal_error(err: tokio::io::Error) -> RussulaError {
    let russula_err = RussulaError::from(err);
    match &russula_err {
        error if error.is_fatal() => tracing::error!("{}", error),
        _ => (),
    }
    russula_err
}

#[derive(Debug)]
pub struct Msg {
    len: u16,
    data: Bytes,
}

impl Msg {
    pub fn new(data: Bytes) -> RussulaResult<Msg> {
        let _ = std::str::from_utf8(&data).map_err(|err| RussulaError::BadMsg {
            dbg: format!(
                "Failed to parse msg as utf8. len: {} data: {:?}, {err}",
                data.len(),
                data
            ),
        })?;

        Ok(Msg {
            len: data.len() as u16,
            data,
        })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn as_str(&self) -> &str {
        // unwrap is safe since we check that data is a valid utf8 slice when
        // constructing a Msg struct
        std::str::from_utf8(&self.data).unwrap()
    }

    pub fn payload_len(&self) -> u16 {
        self.len
    }
}

impl std::fmt::Display for Msg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let data = std::str::from_utf8(&self.data).expect("expected str");
        write!(f, "Msg [ len: {} data: {} ]", self.len, data)
    }
}
