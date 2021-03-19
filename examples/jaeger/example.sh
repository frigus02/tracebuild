#!/bin/bash

set -euo pipefail

export OTEL_TRACES_EXPORTER=jaeger

HERE=$(dirname "$0")
tracebuild="$HERE/../../target/debug/tracebuild"

export TRACEBUILD_BUILD_ID=$($tracebuild id)
export TRACEBUILD_BUILD_START=$($tracebuild now)

$tracebuild cmd -- sleep 2

export TRACEBUILD_STEP_ID=$($tracebuild id)
export TRACEBUILD_STEP_START=$($tracebuild now)
$tracebuild cmd -- sleep 1
$tracebuild cmd -- sleep 1
$tracebuild step \
	--name build \
	--status success

export TRACEBUILD_STEP_ID=$($tracebuild id)
export TRACEBUILD_STEP_START=$($tracebuild now)
$tracebuild cmd -- sleep 1
$tracebuild cmd -- sleep 1
$tracebuild step \
	--name test \
	--status success

$tracebuild build \
	--name example \
	--status success \
	--commit $(git rev-parse HEAD)
