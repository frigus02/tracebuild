# Jaeger Example

Start Jaeger:

```
docker run --rm -it \
    -p 6831:6831/udp -p 6832:6832/udp -p 16686:16686 -p 14268:14268 \
    jaegertracing/all-in-one
```

Run tracebuild example script:

```
cargo build
./example.sh
```

See traces:

```
open http://localhost:16686
```
