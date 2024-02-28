// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::{RussulaError, RussulaResult};
use bytes::BufMut;
use bytes::Bytes;
use tokio::net::TcpStream;
use tracing::error;

trait ReaderWriter {
    fn try_read(&self, buf: &mut [u8]) -> RussulaResult<usize>;
    fn try_read_buf<B: BufMut>(&self, buf: &mut B) -> RussulaResult<usize>;
    fn try_write(&self, buf: &[u8]) -> RussulaResult<usize>;
}

impl ReaderWriter for TcpStream {
    fn try_read(&self, buf: &mut [u8]) -> RussulaResult<usize> {
        self.try_read(buf).map_err(log_fatal_error)
    }

    fn try_read_buf<B: BufMut>(&self, buf: &mut B) -> RussulaResult<usize> {
        self.try_read_buf(buf).map_err(log_fatal_error)
    }

    fn try_write(&self, buf: &[u8]) -> RussulaResult<usize> {
        self.try_write(buf).map_err(log_fatal_error)
    }
}

pub async fn recv_msg(stream: &TcpStream) -> RussulaResult<Msg> {
    stream.readable().await.map_err(log_fatal_error)?;
    read_msg(stream).await
}

async fn read_msg<R: ReaderWriter>(stream: &R) -> RussulaResult<Msg> {
    let msg_len = read_msg_len(stream).await?;

    let mut data = Vec::with_capacity(msg_len.into());
    let read_bytes = stream.try_read_buf(&mut data)?;
    if read_bytes == 0 {
        error!("read len 0");
        return Err(RussulaError::ReadFail {
            dbg: "read 0 byters.. read closed?".to_string(),
        });
    }

    if read_bytes == msg_len as usize {
        Msg::new(data.into())
    } else {
        let log_received_data =
            std::str::from_utf8(&data).unwrap_or("Unable to parse bytes as str!!");
        let dbg = format!(
            "received malformed/partial len: {} data: {}",
            msg_len, log_received_data
        );
        error!(dbg);
        Err(RussulaError::BadMsg { dbg })
    }
}

async fn read_msg_len<R: ReaderWriter>(stream: &R) -> RussulaResult<u16> {
    let mut len_buf = [0; 2];
    let payload_len = stream.try_read(&mut len_buf)?;
    if payload_len == 0 {
        error!("read len 0");
        return Err(RussulaError::ReadFail {
            dbg: "read 0 byters.. read closed?".to_string(),
        });
    }
    let len = u16::from_be_bytes(len_buf);
    Ok(len)
}

pub async fn send_msg(stream: &TcpStream, msg: Msg) -> RussulaResult<usize> {
    stream.writable().await.map_err(log_fatal_error)?;
    write_msg(stream, msg).await
}

async fn write_msg<W: ReaderWriter>(stream: &W, msg: Msg) -> RussulaResult<usize> {
    let data = construct_payload(msg);
    let msg = stream.try_write(&data)?;
    Ok(msg)
}

fn construct_payload(msg: Msg) -> Vec<u8> {
    // msg size + size of len prefix
    let payload_len = msg.len as usize + std::mem::size_of_val(&msg.len);
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

#[derive(Debug, PartialEq)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Arc;
    use std::sync::Mutex;

    struct BufImpl {
        inner: Arc<Mutex<VecDeque<u8>>>,
    }

    impl ReaderWriter for BufImpl {
        fn try_read(&self, buf: &mut [u8]) -> RussulaResult<usize> {
            let mut inner_buf = self.inner.lock().unwrap();
            let mut data = Vec::new();
            for _i in 0..buf.len() {
                let byte = inner_buf.pop_front().unwrap();
                data.push(byte);
            }
            buf.clone_from_slice(&data);

            Ok(buf.len())
        }

        fn try_read_buf<B: BufMut>(&self, buf: &mut B) -> RussulaResult<usize> {
            let mut len = 0;
            let mut inner_buf = self.inner.lock().unwrap();
            while let Some(byte) = inner_buf.pop_front() {
                buf.put_u8(byte);
                len += 1;
            }

            Ok(len)
        }

        fn try_write(&self, buf: &[u8]) -> RussulaResult<usize> {
            let msg = Msg::new(Bytes::copy_from_slice(buf)).unwrap();
            let data = construct_payload(msg);

            let mut inner_buf = self.inner.lock().unwrap();
            inner_buf.extend(data);
            Ok(inner_buf.len())
        }
    }

    #[tokio::test]
    async fn test_read_msg() -> RussulaResult<()> {
        let buffer = BufImpl {
            inner: Arc::new(Mutex::new(VecDeque::new())),
        };

        let data: &[u8] = &[1, 2, 3, 4, 5];
        let msg = Msg::new(data.into()).unwrap();
        assert_eq!(msg.data, data);
        assert_eq!(msg.len, 5);

        let bytes_written = buffer.try_write(&msg.data).unwrap();
        assert_eq!(bytes_written, 7);
        println!("{:?}", buffer.inner.lock().unwrap());

        let recv_msg = read_msg(&buffer).await.unwrap();
        assert_eq!(msg, recv_msg);

        Ok(())
    }
}
