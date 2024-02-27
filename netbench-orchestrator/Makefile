# # -------------------- bin orchestrator
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

# -------------------- test russula_cli with netbench. use with scripts/net_client.sh
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

# -------------------- test russula_cli with sim process (scripts/sim_netbench_*.sh)
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
	RUST_LOG=none,russula_cli=info cargo test -- server --nocapture
unit_test_client:
	RUST_LOG=none,russula_cli=info cargo test -- client --nocapture
