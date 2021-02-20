#!/bin/bash

set -euo pipefail

export OTEL_TRACES_EXPORTER=none
export OTEL_METRICS_EXPORTER=prometheus
export OTEL_EXPORTER_PROMETHEUS_PORT=9091

HERE=$(dirname "$0")
tracebuild="$HERE/../../target/debug/tracebuild"

BUILD_ID=$($tracebuild id)
BUILD_START=$($tracebuild now)

$tracebuild cmd \
	--build $BUILD_ID \
	-- \
	sleep 3

STEP_ID=$($tracebuild id)
STEP_START=$($tracebuild now)
$tracebuild cmd \
	--build $BUILD_ID \
	--step $STEP_ID \
	-- \
	sleep 2
$tracebuild cmd \
	--build $BUILD_ID \
	--step $STEP_ID \
	-- \
	sleep 1
$tracebuild step \
	--build $BUILD_ID \
	--id $STEP_ID \
	--start-time $STEP_START \
	--name build \
	--status success

STEP_ID=$($tracebuild id)
STEP_START=$($tracebuild now)
$tracebuild cmd \
	--build $BUILD_ID \
	--step $STEP_ID \
	-- \
	sleep 4
$tracebuild step \
	--build $BUILD_ID \
	--id $STEP_ID \
	--start-time $STEP_START \
	--name test \
	--status success

$tracebuild build \
	--id $BUILD_ID \
	--start-time $BUILD_START \
	--name example \
	--status success \
	--branch main
