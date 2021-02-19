#!/bin/bash

set -euo pipefail

docker build -t frigus02/tracebuild-example-app-insights-otelcol .
docker push frigus02/tracebuild-example-app-insights-otelcol
