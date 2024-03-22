# Netbench Orchestrator

Automate netbench runs in the cloud.

## Goals
Often developers of transport protocols are interested in gathering performance data for the protocol
they are developing. Netbench is a tool which can be used to measure this performance data.
However, its often necessary to run Netbench scenarios in the cloud so that the results better match
production systems. The goal of this project is to automate Netbench runs in the cloud.

## Getting started

**Pre-requisites**
- Built and include [netbench](https://github.com/aws/s2n-netbench) utilities (`cargo build`)
  - Include in PATH `export PATH="s2n-netbench/target/release/:$PATH"`. Test with `which s2n-netbench`
- AWS cli is installed. Test with `which aws`
- An AWS account with some infrastructure configured. TODO: provide an easy way to do this
  - Make sure AWS credentials are included in your shell environment
- The ec2 SSH key name is correctly set in state.rs (make this configurable)

**Running**

```
git clone git@github.com:toidiu/netbench_orchestrator.git && cd netbench_orchestrator

# Run the orchestrator
make run_orchestrator
```

## Project Overview
Since the goal of the Orchestrator is to run workloads on remote servers, its best to think
of the project as two components; stuff that runs locally vs remotely.

- The **Orchestrator** runs **locally**, and is responsible for spinning up servers, and
configuring them to run netbench. The orchestrator creates Russula 'Coordinators' to
coordinate the remote netbench runs (done over TCP connection).
- **SSM** and **Russula** run **remotely** on the hosts. SSM is used primarily for async
tasks such as installing/updating dependencies and uploading netbench data to s3. SSM is
also used to start the Russula worker process on each host. Russula is primarily used for
timing sensitive tasks such netbench, which require starting and stopping multiple servers
and clients processes across multiple hosts. Specifically, the 'Worker' component of a
protocol is executed on the remote host, while the 'Coordinator' component is run locally
as part of the Orchestrator.

### Debugging
As discussed in the above overview, there are processes that run locally and those that run
remotely. This sections describes how to go about debugging each component.

#### Local
**Orchestrator**
The Orchestrator is Rust code and ships with [tracing](https://docs.rs/tracing/latest/tracing/)
support. Logs are written to a file `orch_proj/target/russula.log*` file on the host. The
`make run_orchestrator` command enables sane log levels via `RUST_LOG=...` but these can be
changed as desired.

#### Remote
**SSH access**
ec2 accepts the name of a ssh-key when creating a new host. This is set to a default value
under the value `ssh_key_name` in the [state.rs](/src/state.rs) file. By providing this key
it is possible to ssh onto the remote host locally: `ssh -oStrictHostKeyChecking=no ec2-user@x.x.x.x`.
Its also possible to ssh onto a host from the ec2 console on AWS.

Useful command for debugging progress on remote host:
```
watch -n 1 "ls -xm; echo ===; ls -xm bin; echo ===; tail netbench_orchestrator/target/russula.log*; echo ===; ps aux | grep 'cargo\|russula\|netbench\|rustup';"
```

**Russula**
The Worker component of Russula executes on the remote hosts. Russula is Rust code and also
ships with [tracing](https://docs.rs/tracing/latest/tracing/) support. Logs are
written to a file `orch_proj/target/russula.log*` file on the host. It can be quite useful
to disable host cleanup when trying to debug issues on the remote hosts. See the SSH access
section for how to access remote hosts.

**SSM**
SSM executes on the remote host and takes bash commands, which are executed by a 'ssm-agent'
running on the remote host. It's important to note that by default SSM operations are run as
the `root` user. Cloudwatch logging has been enabled for SSM and captures the 'stdout' and
'stderr' output from execution. SSM commands are categorized into[Steps](src/ssm_utils.rs#L22)
and each step emits a `start_step` file on the host when it begins and replaces it with
`fin_step` when it finishes. These files are actually essential to making SSM execution
serialized, but they also help with debugging. SSM failures can be quite painful to debug since
failures can happen silently. See the SSH access section for how to access remote hosts.

## Implementation details

### Russula
Russula is a coordination framework where a single Coordinator can be used to drive
multiple Workers. This is driven by the need to test multiple server/client incast Netbench
scenario.

At its basis an instance of Russula is composed of a pair of Coordinator/Worker Protocols. Currently
its possible to create an instance of NetbenchServer and NetbenchClient, which can be used to run
a multi server/client netbench scenario.

Since Russula is used to run Netbench testing it has the following goals:
- non-blocking: its not acceptable to block since we are trying to do performance testing
- minimal network noise: since we are trying to measure transport protocols, the coordination protocol
should add minimal traffic to the network
- easily configurable: the protocol should allow for new states to allow for expanding use cases
- secure: the protocol should not accept executable code since this opens it up for code execution attack.
- easy to develop: exposes logging and introspection into the peers states to allow for easy debugging
- resilient: should be resilient to errors (network or otherwise); retrying requests when they are considered
non-fatal
