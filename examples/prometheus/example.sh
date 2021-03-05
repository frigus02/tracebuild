#!/bin/bash

set -xeuo pipefail

export OTEL_TRACES_EXPORTER=none
export OTEL_METRICS_EXPORTER=prometheus
export OTEL_EXPORTER_PROMETHEUS_PORT=9091

HERE=$(dirname "$0")
tracebuild="$HERE/../../target/debug/tracebuild"

BUILD_ID=$($tracebuild id)
BUILD_START=$($tracebuild now)

STEP_ID=$($tracebuild id)
STEP_START=$($tracebuild now)
sleep 1 # download tracebuild
$tracebuild step --build $BUILD_ID --id $STEP_ID --start-time $STEP_START --name Setup --status success

test() {
	TEST_STEP_ID=$($tracebuild id)
	TEST_STEP_START=$($tracebuild now)

	STEP_START=$($tracebuild now)
	sleep $((12 + $RANDOM % 6))
	$tracebuild step --build $BUILD_ID --step $TEST_STEP_ID --id $($tracebuild id) --start-time $STEP_START --name "Install toolchain" --status success

	$tracebuild cmd --build $BUILD_ID --step $STEP_ID --name "cargo test" -- sleep $((120 + $RANDOM % 60))

	$tracebuild step --build $BUILD_ID --id $TEST_STEP_ID --start-time $TEST_STEP_START --name test --status success
}

lint() {
	LINT_STEP_ID=$($tracebuild id)
	LINT_STEP_START=$($tracebuild now)

	STEP_START=$($tracebuild now)
	sleep $((12 + $RANDOM % 6))
	$tracebuild step --build $BUILD_ID --step $LINT_STEP_ID --id $($tracebuild id) --start-time $STEP_START --name "Install toolchain" --status success

	$tracebuild cmd --build $BUILD_ID --step $STEP_ID --name "cargo fmt" -- sleep 1
	$tracebuild cmd --build $BUILD_ID --step $STEP_ID --name "cargo clippy" -- sleep $((75 + $RANDOM % 30))

	$tracebuild step --build $BUILD_ID --id $LINT_STEP_ID --start-time $LINT_STEP_START --name lint --status success
}

test &
lint &
wait

$tracebuild build --id $BUILD_ID --start-time $BUILD_START --name "tracebuild - Example Prometheus" --status success --branch main
