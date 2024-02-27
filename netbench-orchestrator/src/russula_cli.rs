use crate::russula::netbench;
use core::time::Duration;
use russula::{
    netbench::{client, server},
    RussulaBuilder,
};
use std::{collections::BTreeSet, net::SocketAddr};
use structopt::StructOpt;
use tracing::debug;
use tracing_subscriber::EnvFilter;

mod russula;

/// This utility is a convenient CLI wrapper around Russula and can be used to launch
/// different protocols.
///
/// It is used by the Orchestrator to launch Russula Workers on remote hosts.

#[derive(StructOpt, Debug)]
struct Opt {
    #[structopt(long, parse(try_from_str=parse_duration), default_value = "5s")]
    poll_delay: Duration,

    #[structopt(subcommand)]
    protocol: RussulaProtocol,
}

#[allow(clippy::enum_variant_names)]
#[derive(StructOpt, Debug)]
enum RussulaProtocol {
    NetbenchServerWorker {
        /// The port on which the Worker should 'listen' on.
        #[structopt(long)]
        russula_port: u16,

        #[structopt(flatten)]
        ctx: netbench::ServerContext,
    },
    NetbenchClientWorker {
        /// The port on which the Worker should 'listen' on.
        #[structopt(long)]
        russula_port: u16,

        #[structopt(flatten)]
        ctx: netbench::ClientContext,
    },
    NetbenchServerCoordinator {
        /// The list of worker addresses which the Coordinator should
        /// attempt to connect
        #[structopt(long, required = true)]
        russula_worker_addrs: Vec<SocketAddr>,
    },
    NetbenchClientCoordinator {
        /// The list of worker addresses which the Coordinator should
        /// attempt to connect
        #[structopt(long)]
        russula_worker_addrs: Vec<SocketAddr>,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> () {
    let opt = Opt::from_args();

    let file_appender = tracing_appender::rolling::daily("./target", "russula.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(non_blocking)
        .init();

    debug!("{:?}", opt);
    println!("{:?}", opt);
    match &opt.protocol {
        RussulaProtocol::NetbenchServerWorker { ctx, russula_port } => {
            let netbench_ctx = ctx.clone();
            let russula_port = *russula_port;
            run_server_worker(opt, netbench_ctx, russula_port).await
        }
        RussulaProtocol::NetbenchClientWorker { ctx, russula_port } => {
            let netbench_ctx = ctx.clone();
            let russula_port = *russula_port;
            run_client_worker(opt, netbench_ctx, russula_port).await
        }
        RussulaProtocol::NetbenchServerCoordinator {
            russula_worker_addrs,
        } => {
            let w = russula_worker_addrs.clone();
            run_local_server_coordinator(opt, w).await
        }
        RussulaProtocol::NetbenchClientCoordinator {
            russula_worker_addrs,
        } => {
            let w = russula_worker_addrs.clone();
            run_local_client_coordinator(opt, w).await
        }
    };

    println!("cli done");
}

async fn run_server_worker(opt: Opt, netbench_ctx: netbench::ServerContext, russula_port: u16) {
    let uuid = uuid::Uuid::new_v4().to_string();
    let id = format!("{}-{}", uuid, netbench_ctx.trim_driver_name());
    let protocol = server::WorkerProtocol::new(id, netbench_ctx);
    let worker = RussulaBuilder::new(
        BTreeSet::from_iter([local_listen_addr(russula_port)]),
        protocol,
        opt.poll_delay,
    );
    let mut worker = worker.build().await.unwrap();
    worker.run_till_ready().await.unwrap();

    worker.run_till_done().await.unwrap();
}

async fn run_client_worker(opt: Opt, netbench_ctx: netbench::ClientContext, russula_port: u16) {
    let uuid = uuid::Uuid::new_v4().to_string();
    let id = format!("{}-{}", uuid, netbench_ctx.trim_driver_name());
    let protocol = client::WorkerProtocol::new(id, netbench_ctx);
    let worker = RussulaBuilder::new(
        BTreeSet::from_iter([local_listen_addr(russula_port)]),
        protocol,
        opt.poll_delay,
    );
    let mut worker = worker.build().await.unwrap();
    worker.run_till_ready().await.unwrap();

    worker.run_till_done().await.unwrap();
}

async fn run_local_server_coordinator(opt: Opt, russula_worker_addrs: Vec<SocketAddr>) {
    let protocol = server::CoordProtocol::new();
    let coord = RussulaBuilder::new(
        // TODO for local testing.. we only connect to 1 local worker
        BTreeSet::from_iter(russula_worker_addrs),
        protocol,
        opt.poll_delay,
    );
    let mut coord = coord.build().await.unwrap();

    coord.run_till_worker_running().await.unwrap();

    println!("Waiting for user input to continue ... WorkersRunning");
    let mut s = String::new();
    let _ = std::io::stdin().read_line(&mut s);
    println!("Stopping workers ...");

    coord.run_till_done().await.unwrap();
}

async fn run_local_client_coordinator(opt: Opt, russula_worker_addrs: Vec<SocketAddr>) {
    let protocol = client::CoordProtocol::new();
    let coord = RussulaBuilder::new(
        // TODO for local testing.. we only connect to 1 local worker
        BTreeSet::from_iter(russula_worker_addrs),
        protocol,
        opt.poll_delay,
    );
    let mut coord = coord.build().await.unwrap();

    coord.run_till_worker_running().await.unwrap();

    coord.run_till_done().await.unwrap();
}

fn local_listen_addr(russula_port: u16) -> SocketAddr {
    format!("0.0.0.0:{}", russula_port).parse().unwrap()
}

fn parse_duration(s: &str) -> Result<Duration, humantime::DurationError> {
    humantime::parse_duration(s)
}
