// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use core::{task::Poll, time::Duration};
use std::{collections::BTreeSet, net::SocketAddr};
use tokio::net::TcpStream;
use tracing::{error, info};

mod error;
mod event;
mod network_utils;
mod protocol;
mod states;

use error::{RussulaError, RussulaResult};
use protocol::Protocol;

const CONNENT_RETRY_ATTEMPT: usize = 10;

#[derive(Debug, Copy, Clone)]
pub enum ProtocolState {
    /// The endpoint has established connection with its peer and
    /// is ready to make progress.
    Ready,

    /// Indicates the protocol's terminal state.
    Done,

    /// Indicates that worker are running and accepting work.
    ///
    /// For netbench this state can be used to confirm that all servers are
    /// running and accepting connection before starting netbench clients.
    /// Should only be called by Coordinators.
    WorkerRunning,
}

/// An established connection to a running instance of a Russula protocol.
struct ProtocolInstance<P: Protocol> {
    pub addr: SocketAddr,
    pub stream: TcpStream,
    pub protocol: P,
}

/// A Russula Endpoint.
///
/// An Endpoint can be of type Coordinator or Worker. A Coordinator can
/// be used to synchronize multiple workers across different hosts. A Worker
/// communicates with a Coordinator to make progress.
pub struct Endpoint<P: Protocol> {
    /// List of protocol instances to synchronize.
    instance_list: Vec<ProtocolInstance<P>>,

    /// Polling frequency.
    poll_delay: Duration,
}

impl<P: Protocol + Send> Endpoint<P> {
    /// Drive the underlying list of protocols until the desired state.
    ///
    /// For a non-blocking version use [Self::poll_until]
    pub async fn run_untill(&mut self, desired_state: ProtocolState) -> RussulaResult<()> {
        while self.poll_until(desired_state).await?.is_pending() {
            tokio::time::sleep(self.poll_delay).await;
        }

        Ok(())
    }

    /// Poll the underlying list of protocols until the desired state.
    ///
    /// Attempt to drive each instance
    pub async fn poll_until(&mut self, desired_state: ProtocolState) -> RussulaResult<Poll<()>> {
        // Poll each peer protocol instance.
        //
        // If the peer is already in the desired state then this should be a noop.
        for peer in self.instance_list.iter_mut() {
            if let Err(err) = peer
                .protocol
                .poll_state(&mut peer.stream, desired_state)
                .await
            {
                if err.is_fatal() {
                    error!("{} {}", err, peer.addr);
                    panic!("{} {}", err, peer.addr);
                }
            }
        }

        // Check that all instances are at the desired state.
        let poll = if self.is_state(desired_state) {
            Poll::Ready(())
        } else {
            Poll::Pending
        };
        Ok(poll)
    }

    /// Check if all instances are at the desired state
    fn is_state(&self, state: ProtocolState) -> bool {
        for peer in self.instance_list.iter() {
            // All instance must be at the desired state
            if !peer.protocol.is_state(state) {
                return false;
            }
        }
        true
    }
}

type SockProtocol<P> = (SocketAddr, P);
pub struct RussulaBuilder<P: Protocol> {
    /// Address on which the Coordinator and Worker communicate on.
    ///
    /// The Coordinator gets a list of workers addrs to 'connect' to. This can
    /// be of size >= 1. The Worker gets its own addr to 'listen' on and should
    /// be size = 1.
    // TODO Create different Russula struct for Coordinator/Workers to capture
    // different usage patterns.
    protocol_addr_pair_list: Vec<SockProtocol<P>>,
    poll_delay: Duration,
}

impl<P: Protocol> RussulaBuilder<P> {
    pub fn new(peer_addr: BTreeSet<SocketAddr>, protocol: P, poll_delay: Duration) -> Self {
        // TODO if worker check that the list is len 1 and points to local addr on which to listen
        let mut peer_list = Vec::new();
        peer_addr.into_iter().for_each(|addr| {
            peer_list.push((addr, protocol.clone()));
        });
        Self {
            protocol_addr_pair_list: peer_list,
            poll_delay,
        }
    }

    pub async fn build(self) -> RussulaResult<Endpoint<P>> {
        let mut stream_protocol_list = Vec::new();
        for (addr, protocol) in self.protocol_addr_pair_list.into_iter() {
            let mut retry_attempts = CONNENT_RETRY_ATTEMPT;
            loop {
                if retry_attempts == 0 {
                    return Err(RussulaError::NetworkConnectionRefused {
                        dbg: "Failed to connect to peer".to_string(),
                    });
                }
                match protocol.pair_peer(&addr).await {
                    Ok(connect) => {
                        info!("Coordinator: successfully connected to {}", addr);
                        stream_protocol_list.push(ProtocolInstance {
                            addr,
                            stream: connect,
                            protocol,
                        });

                        break;
                    }
                    Err(err) => {
                        error!(
                            "Failed to connect.. wait and retry. Try disabling VPN and check your network connectivity.
                            \nRetry attempts left: {}. addr: {} dbg: {}",
                            retry_attempts, addr, err
                        );
                        println!(
                            "Failed to connect.. wait and retry. Try disabling VPN and check your network connectivity.
                            \nRetry attempts left: {}. addr: {} dbg: {}",
                            retry_attempts, addr, err
                        );
                        tokio::time::sleep(self.poll_delay).await;
                    }
                }
                retry_attempts -= 1
            }
        }

        Ok(Endpoint {
            instance_list: stream_protocol_list,
            poll_delay: self.poll_delay,
        })
    }
}
