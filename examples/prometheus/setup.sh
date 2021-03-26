#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(dirname "$0")"
cd "$SCRIPT_DIR"

python -m webbrowser http://localhost:3000/d/tracebuild

docker run --rm -d --name tracebuild_prom_aggregation_gateway \
	-p 9091:9091 \
	weaveworks/prom-aggregation-gateway -listen ":9091"

docker run --rm -d --name tracebuild_prometheus \
	-p 9090:9090 \
	-v $PWD/prometheus.yml:/etc/prometheus/prometheus.yml \
	prom/prometheus

docker run --rm -d --name tracebuild_grafana \
	-p 3000:3000 \
	-e GF_AUTH_ANONYMOUS_ENABLED=true \
	-e GF_AUTH_ANONYMOUS_ORG_ROLE=Admin \
	-v $PWD/grafana/provisioning/datasources:/etc/grafana/provisioning/datasources \
	-v $PWD/grafana/provisioning/dashboards:/etc/grafana/provisioning/dashboards \
	-v $PWD/grafana/dashboards:/var/lib/grafana/dashboards \
	grafana/grafana

# idle waiting for abort from user
(trap exit SIGINT; read -r -d '' _ </dev/tty)

docker stop tracebuild_grafana
docker stop tracebuild_prometheus
docker stop tracebuild_prom_aggregation_gateway
