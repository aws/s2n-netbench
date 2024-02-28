// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::{RussulaError, RussulaResult};
use bytes::Bytes;
use tokio::net::TcpStream;
use tracing::error;

pub async fn recv_msg(stream: &TcpStream) -> RussulaResult<Msg> {
    stream.readable().await.map_err(log_fatal_error)?;
    read_msg(stream).await
}

async fn read_msg(stream: &TcpStream) -> RussulaResult<Msg> {
    let mut len_buf = [0; 2];
    let payload_len = stream.try_read(&mut len_buf).map_err(log_fatal_error)?;
    if payload_len == 0 {
        error!("read len 0");
        return Err(RussulaError::ReadFail {
            dbg: "read 0 byters.. read closed?".to_string(),
        });
    }
    let len = u16::from_be_bytes(len_buf);

    let mut data = Vec::with_capacity(len.into());
    let read_bytes = stream.try_read_buf(&mut data).map_err(log_fatal_error)?;
    if read_bytes == 0 {
        error!("read len 0");
        return Err(RussulaError::ReadFail {
            dbg: "read 0 byters.. read closed?".to_string(),
        });
    }

    if read_bytes == len as usize {
        Msg::new(data.into())
    } else {
        let log_received_data =
            std::str::from_utf8(&data).unwrap_or("Unable to parse bytes as str!!");
        error!("received malformed/partial data: {}", log_received_data);
        Err(RussulaError::BadMsg {
            dbg: format!(
                "received a malformed/partial msg. len: {} data: {:?}",
                len, log_received_data
            ),
        })
    }
}

pub async fn send_msg(stream: &TcpStream, msg: Msg) -> RussulaResult<usize> {
    stream.writable().await.map_err(log_fatal_error)?;
    write_msg(stream, msg).await
}

async fn write_msg(stream: &TcpStream, msg: Msg) -> RussulaResult<usize> {
    let mut data: Vec<u8> = Vec::with_capacity((msg.len + 1).into());
    data.extend(msg.len.to_be_bytes());
    data.extend(msg.data);

    let msg = stream.try_write(&data).map_err(log_fatal_error)?;
    Ok(msg)
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
