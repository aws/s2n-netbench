# -------------------- bin orchestrator
# run_orchestrator:
# 	RUST_LOG=none,orchestrator::russula=info,orchestrator=debug cargo run --bin orchestrator -- \
# 					 --cdk-config-file cdk_config.json \
# 					 --client-az us-west-2a \
# 					 --server-az us-west-2b,us-west-2b,us-west-2a,us-west-2a,us-west-2a \
# 					 --server-placement cluster,cluster,cluster,unspecified,cluster \
# 					 --netbench-scenario-file scripts/request_response_multi_5_incast_1GB_req_resp.json \
# 					 # --server-az us-west-2b,us-west-2a \
# 					 # --netbench-scenario-file scripts/request_response_multi_2_incast_1GB_req_resp.json \
# 					 # --netbench-scenario-file scripts/request_response_multi_20_incast_1GB_req_resp.json
# 					 # --netbench-scenario-file scripts/request_response_multi_20_incast_3GB_req_resp.json
# 					 # --netbench-scenario-file scripts/request_response_multi_10_incast_1GB_req_resp.json
# 					 # --netbench-scenario-file scripts/request_response_multi_11_incast_1GB_req_resp.json
# 					 # --placement partition \

# -------------------- test russula_cli with real netbench
#  The following scripts simulate a netbench incast test using 2 servers and 1 client.
#
#  First launch 2 netbench server process (worker1 and worker2) and use the coordinator to
#  orchestrate them. Once the netbench server process is running, will launch a netbench
#  client task manually. To finish the test, tell the coordinator to terminate the workers.
#
# Build netbench project (s2n-netbench root project dir):
# `cargo build --release`
#
# Launch workers (netbench-orchestrator dir):
# `sudo make net_server_worker1`
# `sudo make net_server_worker2`
#
# Launch coordinator (netbench-orchestrator dir):
# `sudo make net_server_coord`
#
# Wait for the netbench process to start. Then launch the netbench client process (netbench-orchestrator dir):
# `./scripts/net_client.sh`
#
# It's possible to launch multiple runs:
# `sudo ./scripts/net_client.sh`
# `sudo ./scripts/net_client.sh`
#
# To finish, go to the coordinator and enter any key to terminate the coordinator and workers.
#
net_server_coord:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- \
					 --poll-delay 2s \
					 netbench-server-coordinator \
					 --russula-worker-addrs 0.0.0.0:7000 \
					 0.0.0.0:8000 \

net_server_worker1:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- \
					 --poll-delay 2s \
					 netbench-server-worker \
					 --russula-port 7000 \
					 --netbench-path ../target/release \
					 --driver s2n-netbench-driver-server-s2n-quic \
					 --scenario ../../netbench-orchestrator/scripts/request_response_multi_2_incast_1GB_req_resp.json \
					 --netbench-port 7001 \

net_server_worker2:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- \
					 --poll-delay 2s \
					 netbench-server-worker \
					 --russula-port 8000 \
					 --netbench-path ../target/release \
					 --driver s2n-netbench-driver-server-s2n-quic \
					 --scenario ../../netbench-orchestrator/scripts/request_response_multi_2_incast_1GB_req_resp.json \
					 --netbench-port 8001 \


# -------------------- test server russula_cli with simulated 'testing' process
test_server_coord:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- \
					 --poll-delay 1s \
					 netbench-server-coordinator \
					 --russula-worker-addrs 0.0.0.0:7000 0.0.0.0:7001 \

test_server_worker1:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- \
					 --poll-delay 1s \
					 netbench-server-worker \
					 --russula-port 7000 \
					 --testing \
					 --driver unused

test_server_worker2:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- \
					 --poll-delay 1s \
					 netbench-server-worker \
					 --russula-port 7001 \
					 --testing \
					 --driver unused

# -------------------- test client russula_cli with simulated 'testing' process
test_client_coord:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli --  \
					 --poll-delay 1s \
					 netbench-client-coordinator \
					 --russula-worker-addrs 0.0.0.0:8000 0.0.0.0:8001 \

test_client_worker1:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- \
					 --poll-delay 1s \
					 netbench-client-worker \
					 --russula-port 8000 \
					 --testing \
					 --driver unused \

test_client_worker2:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- \
					 --poll-delay 1s \
					 netbench-client-worker \
					 --russula-port 8001 \
					 --testing \
					 --driver unused \

# -------------------- test russula
unit_test_server:
	RUST_LOG=none,orchestrator=info cargo test  -- server --nocapture
unit_test_client:
	RUST_LOG=none,orchestrator=info cargo test  -- client --nocapture
