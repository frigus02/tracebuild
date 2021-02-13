# OpenTelemetry Collector Example

Start OpenTelemetry Collector:

```
docker run --rm -it \
    -p 4317:4317 \
    -v $PWD/config.yaml:/config.yaml \
    --name otelcol \
    otel/opentelemetry-collector \
    --config config.yaml
```

Run tracebuild:

```
tracebuild cmd --build $(tracebuild id) -- ls
```
