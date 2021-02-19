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
docker run --rm -it -p 9091:9091 prom/pushgateway
```

Run tracebuild:

```
export OTEL_TRACES_EXPORTER=stdout
export OTEL_METRICS_EXPORTER=prometheus
export OTEL_EXPORTER_PROMETHEUS_PORT=9091
tracebuild cmd --build $(tracebuild id) -- ls
```

See metrics:

```
open http://localhost:9090
```
