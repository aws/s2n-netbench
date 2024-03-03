#/usr/bin/env bash

# helper script for testing russula with a real netbench process over localhost.
# See the documentation in Makefile (`make net_server_coord`) for how to use.

set -e

PORT=9000 SERVER_0=127.0.0.1:7001 SERVER_1=127.0.0.1:8001 ../target/release/s2n-netbench-collector ../target/release/s2n-netbench-driver-client-s2n-quic --scenario scripts/request_response_multi_2_incast_1GB_req_resp.json
