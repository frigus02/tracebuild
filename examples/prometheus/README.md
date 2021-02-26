# Prometheus Example

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
