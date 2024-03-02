// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use core::{task::Poll, time::Duration};
use paste::paste;
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

struct ProtocolInstance<P: Protocol> {
    pub addr: SocketAddr,
    pub stream: TcpStream,
    pub protocol: P,
}

/// A coordination framework.
///
/// Russula is a wrapper over a pair of Coordinator and Worker [Protocol]s.
/// A Coordinator can be used to synchronize multiple workers across
/// different hosts.
pub struct Russula<P: Protocol> {
    /// List of protocol instance to synchronize with.
    instance_list: Vec<ProtocolInstance<P>>,

    /// Polling frequency when trying to make progress.
    poll_delay: Duration,
}

macro_rules! state_api {
    {
        $(#[$meta:meta])*
            $state:ident
    } => {paste!{

    $(#[$meta])*
    pub async fn [<run_till_ $state>](&mut self) -> RussulaResult<()> {
        while self.[<poll_ $state>]().await?.is_pending() {
            tokio::time::sleep(self.poll_delay).await;
        }

        Ok(())
    }

    $(#[$meta])*
    pub async fn [<poll_ $state>](&mut self) -> RussulaResult<Poll<()>> {
        // Poll each peer protocol instance.
        //
        // If the peer is already in the desired state then this should be a noop.
        for peer in self.instance_list.iter_mut() {
            if let Err(err) = peer.protocol.[<poll_ $state>](&mut peer.stream).await {
                if err.is_fatal() {
                    error!("{} {}", err, peer.addr);
                    panic!("{} {}", err, peer.addr);
                }
            }
        }

        // Check that all instances are at the desired state.
        let poll = if self.[<is_ $state _state>]() {
            Poll::Ready(())
        } else {
            Poll::Pending
        };
        Ok(poll)
    }

    /// Check if all instances are at the desired state
    fn [< is_ $state _state>](&self) -> bool {
        for peer in self.instance_list.iter() {
            // All instance must be at the desired state
            if !peer.protocol.[< is_ $state _state>]() {
                return false;
            }
        }
        true
    }
}};
}

impl<P: Protocol + Send> Russula<P> {
    state_api!(
        /// Successfully connected to peer instances and ready to make progress.
        ready
    );
    state_api!(
        /// The coordination completed and reached a terminal state.
        done
    );
    state_api!(
        /// Note: Should only be called by Coordinators
        ///
        /// A intermediate state to query if Worker peers are in some
        /// desired state.
        worker_running
    );
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

    pub async fn build(self) -> RussulaResult<Russula<P>> {
        let mut stream_protocol_list = Vec::new();
        for (addr, protocol) in self.protocol_addr_pair_list.into_iter() {
            let mut retry_attempts = 10;
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

        Ok(Russula {
            instance_list: stream_protocol_list,
            poll_delay: self.poll_delay,
        })
    }
}
