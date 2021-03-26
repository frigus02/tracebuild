#!/bin/bash

set -xeuo pipefail

export OTEL_TRACES_EXPORTER=none
export OTEL_METRICS_EXPORTER=prometheus
export OTEL_EXPORTER_PROMETHEUS_PORT=9091

SCRIPT_DIR="$(dirname "$0")"
ROOT_DIR="$SCRIPT_DIR/../.."
PATH="$ROOT_DIR/target/debug:$PATH"

cd "$ROOT_DIR"
cargo build

export TRACEBUILD_BUILD_ID=$(tracebuild id)
export TRACEBUILD_BUILD_START=$(tracebuild now)
export TRACEBUILD_BUILD_NAME=tracebuild-prometheus-example

export TRACEBUILD_STEP_ID=$(tracebuild id)
export TRACEBUILD_STEP_START=$(tracebuild now)
sleep 1 # download tracebuild
tracebuild step --name Setup --status success

test() {
	export TRACEBUILD_STEP_ID=$(tracebuild id)
	export TRACEBUILD_STEP_START=$(tracebuild now)

	STEP_START=$(tracebuild now)
	sleep $((12 + $RANDOM % 6))
	tracebuild step --step $TRACEBUILD_STEP_ID --id $(tracebuild id) --start-time $STEP_START --name "Install toolchain" --status success

	tracebuild cmd --name "cargo test" -- sleep $((120 + $RANDOM % 60))

	tracebuild step --name test --status success
}

lint() {
	export TRACEBUILD_STEP_ID=$(tracebuild id)
	export TRACEBUILD_STEP_START=$(tracebuild now)

	STEP_START=$(tracebuild now)
	sleep $((12 + $RANDOM % 6))
	tracebuild step --step $TRACEBUILD_STEP_ID --id $(tracebuild id) --start-time $STEP_START --name "Install toolchain" --status success

	tracebuild cmd --name "cargo fmt" -- sleep 1
	tracebuild cmd --name "cargo clippy" -- sleep $((75 + $RANDOM % 30))

	tracebuild step --name lint --status success
}

test &
lint &
wait

tracebuild build --name "tracebuild - Example Prometheus" --status success --branch main
