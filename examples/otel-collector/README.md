# OpenTelemetry Collector Example

Start OpenTelemetry Collector:

```
docker run --rm -it \
    -p 4317:4317 -p 14250:14250 -p 6832:6832/udp -p 6831:6831/udp -p 14268:14268 \
    -v $PWD/config.yaml:/config.yaml \
    --name otelcol \
    otel/opentelemetry-collector \
    --config config.yaml
```

Run tracebuild:

```
tracebuild cmd --build $(tracebuild id) -- ls
```
