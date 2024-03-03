// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use core::{task::Poll, time::Duration};
use std::{collections::BTreeSet, net::SocketAddr};
use tokio::net::TcpStream;
use tracing::{error, info};

mod error;
mod event;
pub mod netbench;
mod network_utils;
mod states;
mod workflow;

use error::{RussulaError, RussulaResult};
use workflow::WorkflowTrait;

const CONNECT_RETRY_ATTEMPT: usize = 10;

#[derive(Debug, Copy, Clone)]
pub enum WorkflowState {
    /// The workflow has established connection with its peer and
    /// is ready to make progress.
    Ready,

    /// Indicates the workflow's terminal state.
    Done,

    /// Indicates that worker are running and accepting work.
    ///
    /// For netbench this state can be used to confirm that all servers are
    /// running and accepting connection before starting netbench clients.
    /// Should only be called by Coordinators.
    WorkerRunning,
}

/// An instance of a workflow with an established connection to its peer.
struct Host<W: WorkflowTrait> {
    pub addr: SocketAddr,
    pub stream: TcpStream,
    pub workflow: W,
}

/// A Workflow instance.
///
/// An Workflow can be of type Coordinator or Worker. A Coordinator can
/// be used to synchronize multiple workers across different hosts. A Worker
/// communicates with a Coordinator to make progress.
pub struct Workflow<W: WorkflowTrait> {
    /// List of workflow instances to synchronize with.
    instances: Vec<Host<W>>,

    /// Polling frequency when trying to make progress.
    poll_delay: Duration,
}

impl<W: WorkflowTrait + Send> Workflow<W> {
    pub async fn run_till(&mut self, state: WorkflowState) -> RussulaResult<()> {
        while self.poll_state(state).await?.is_pending() {
            tokio::time::sleep(self.poll_delay).await;
        }

        Ok(())
    }

    pub async fn poll_state(&mut self, state: WorkflowState) -> RussulaResult<Poll<()>> {
        // Poll each peer workflow instance.
        //
        // If the peer is already in the desired state then this should be a noop.
        for peer in self.instances.iter_mut() {
            if let Err(err) = peer.workflow.poll_state(&mut peer.stream, state).await {
                if err.is_fatal() {
                    error!("{} {}", err, peer.addr);
                    panic!("{} {}", err, peer.addr);
                }
            }
        }

        // Check that all instances are at the desired state.
        let poll = if self.is_state(state) {
            Poll::Ready(())
        } else {
            Poll::Pending
        };
        Ok(poll)
    }

    /// Check if all instances are at the desired state
    fn is_state(&self, state: WorkflowState) -> bool {
        for peer in self.instances.iter() {
            // All instance must be at the desired state
            if !peer.workflow.is_state(state) {
                return false;
            }
        }
        true
    }
}

/// Build a [Workflow] that is ready to coordinate with it's peers.
///
/// A [Workflow] contains a list of peers it needs to coordinate with. However,
/// since these peers can run on remote hosts and communication happens over a
/// network, establishing a connection is fallible. The builder attempts to
/// establish a connection with each peer, retrying transient error when possible.
pub struct WorkflowBuilder<W: WorkflowTrait> {
    /// Address on which the Coordinator and Worker communicate on.
    ///
    /// The Coordinator gets a list of workers addrs to 'connect' to. This can
    /// be of size >= 1. The Worker gets its own addr to 'listen' on and should
    /// be size = 1.
    // TODO Create different Russula struct for Coordinator/Workers to capture
    // different usage patterns.
    addrs: Vec<(SocketAddr, W)>,
    poll_delay: Duration,
}

impl<W: WorkflowTrait> WorkflowBuilder<W> {
    pub fn new(peer_addr: BTreeSet<SocketAddr>, workflow: W, poll_delay: Duration) -> Self {
        // TODO if worker check that the list is len 1 and points to local addr on which to listen
        let mut addrs = Vec::new();
        peer_addr.into_iter().for_each(|addr| {
            addrs.push((addr, workflow.clone()));
        });
        Self { addrs, poll_delay }
    }

    /// Build a [Workflow]
    ///
    /// Attempt to establish a connection to all peers via [WorkflowTrait::pair_peer].
    pub async fn build(self) -> RussulaResult<Workflow<W>> {
        let mut workflow_instances = Vec::new();
        for (addr, workflow) in self.addrs.into_iter() {
            let mut retry_attempts = CONNECT_RETRY_ATTEMPT;
            loop {
                if retry_attempts == 0 {
                    return Err(RussulaError::NetworkConnectionRefused {
                        dbg: "Failed to connect to peer".to_string(),
                    });
                }
                match workflow.pair_peer(&addr).await {
                    Ok(connect) => {
                        info!("Coordinator: successfully connected to {}", addr);
                        workflow_instances.push(Host {
                            addr,
                            stream: connect,
                            workflow,
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

        Ok(Workflow {
            instances: workflow_instances,
            poll_delay: self.poll_delay,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::russula::netbench::{client, server};
    use futures::future::join_all;
    use std::str::FromStr;

    const POLL_DELAY_DURATION: Duration = Duration::from_secs(1);

    #[tokio::test]
    async fn netbench_server_protocol() {
        let mut worker_addrs = Vec::new();
        let mut workers = Vec::new();
        macro_rules! worker {
            {$port:literal} => {
                let sock = SocketAddr::from_str(&format!("127.0.0.1:{}", $port)).unwrap();
                let worker = tokio::spawn(async move {
                    let worker = WorkflowBuilder::new(
                        BTreeSet::from_iter([sock]),
                        server::WorkerProtocol::new(
                            sock.port().to_string(),
                            netbench::ServerContext::testing(),
                        ),
                        POLL_DELAY_DURATION,
                    );
                    let mut worker = worker.build().await.unwrap();
                    worker
                        .run_till(WorkflowState::Done)
                        .await
                        .unwrap();
                    worker
                });

                workers.push(worker);
                worker_addrs.push(sock);
            };
        }

        worker!(8001);
        worker!(8002);
        worker!(8003);
        worker!(8004);
        worker!(8005);
        worker!(8006);
        worker!(8007);

        let c1 = tokio::spawn(async move {
            let addr = BTreeSet::from_iter(worker_addrs);
            let protocol = server::CoordProtocol::new();
            let coord = WorkflowBuilder::new(addr, protocol, POLL_DELAY_DURATION * 2);
            let mut coord = coord.build().await.unwrap();
            coord.run_till(WorkflowState::Ready).await.unwrap();
            coord
        });
        let join = tokio::join!(c1);
        let mut coord = join.0.unwrap();
        {
            coord.run_till(WorkflowState::WorkerRunning).await.unwrap();
            let simulate_run_time = Duration::from_secs(5);
            tokio::time::sleep(simulate_run_time).await;
        }

        while coord
            .poll_state(WorkflowState::Done)
            .await
            .unwrap()
            .is_pending()
        {
            println!("poll state: Done");
        }

        {
            let worker_join = join_all(workers).await;
            println!("workers done");
            for w in worker_join {
                assert!(w.unwrap().is_state(WorkflowState::Done));
            }
        }
    }

    #[tokio::test]
    async fn netbench_client_protocol() {
        let mut worker_addrs = Vec::new();
        let mut workers = Vec::new();

        macro_rules! worker {
            {$port:literal} => {
                let sock = SocketAddr::from_str(&format!("127.0.0.1:{}", $port)).unwrap();
                let worker = tokio::spawn(async move {
                    let worker = WorkflowBuilder::new(
                        BTreeSet::from_iter([sock]),
                        client::WorkerProtocol::new(
                            sock.port().to_string(),
                            netbench::ClientContext::testing(),
                        ),
                        POLL_DELAY_DURATION,
                    );
                    let mut worker = worker.build().await.unwrap();
                    worker
                        .run_till(WorkflowState::Done)
                        .await
                        .unwrap();
                    worker
                });

                workers.push(worker);
                worker_addrs.push(sock);
            };
        }

        worker!(9001);
        worker!(9002);
        worker!(9003);
        worker!(9004);

        let c1 = tokio::spawn(async move {
            let addr = BTreeSet::from_iter(worker_addrs);

            let protocol = client::CoordProtocol::new();
            let coord = WorkflowBuilder::new(addr, protocol, POLL_DELAY_DURATION);
            let mut coord = coord.build().await.unwrap();
            coord.run_till(WorkflowState::Ready).await.unwrap();
            coord
        });
        let join = tokio::join!(c1);
        let mut coord = join.0.unwrap();
        {
            coord.run_till(WorkflowState::WorkerRunning).await.unwrap();
        }

        while coord
            .poll_state(WorkflowState::Done)
            .await
            .unwrap()
            .is_pending()
        {
            println!("poll state: Done");
        }

        {
            let worker_join = join_all(workers).await;
            println!("worker done");
            for w in worker_join {
                assert!(w.unwrap().is_state(WorkflowState::Done));
            }
        }
    }
}
