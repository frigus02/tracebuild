#!/bin/bash

set -euo pipefail

python -m webbrowser http://localhost:16686

docker run --rm -it \
    -p 6831:6831/udp -p 6832:6832/udp -p 16686:16686 -p 14268:14268 \
    jaegertracing/all-in-one
