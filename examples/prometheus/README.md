# Prometheus Example

## Basics

Start Prometheus:

```
docker run --rm -it \
    -p 9090:9090 \
    -v $PWD/prometheus.yml:/etc/prometheus/prometheus.yml \
    prom/prometheus
```

Start Prometheus Pushgateway:

```
docker run --rm -it -p 9091:9091 weaveworks/prom-aggregation-gateway -listen ":9091"
```

Run tracebuild:

```
cargo build
./example.sh
```

See metrics:

```
open http://localhost:9090
```

## Grafana

Start Grafana:

```
docker run --rm -it \
    -p 3000:3000 \
    -e GF_AUTH_ANONYMOUS_ENABLED=true -e GF_AUTH_ANONYMOUS_ORG_ROLE=Admin \
    -v $PWD/grafana/provisioning/datasources:/etc/grafana/provisioning/datasources \
    -v $PWD/grafana/provisioning/dashboards:/etc/grafana/provisioning/dashboards \
    -v $PWD/grafana/dashboards:/var/lib/grafana/dashboards \
    grafana/grafana
```

Open it in your browser:

```
open http://localhost:3000/d/tracebuild
```
