[![Crates.io](https://img.shields.io/crates/v/tracebuild.svg)](https://crates.io/crates/tracebuild)
[![Workflow Status](https://github.com/frigus02/tracebuild/workflows/CI/badge.svg)](https://github.com/frigus02/tracebuild/actions?query=workflow%3A%22CI%22)

# tracebuild

A small binary to instrument builds in systems like GitHub Actions, Travis CI, etc. It uses [OpenTelemetry](https://opentelemetry.io/) under the hood, which means you should be able to integrate it in your existing distributed tracing or metrics system.

## Usage

Install the binary in your build:

```
curl -L -o tracebuild https://github.com/frigus02/tracebuild/releases/latest/download/tracebuild-linux-amd64
chmod +x tracebuild
```

Generate IDs and start times:

```
BUILD_ID=$(tracebuild id)
STEP_ID=$(tracebuild id)
BUILD_START=$(tracebuild now)
STEP_START=$(tracebuild now)
```

Wrap each command in:

```
tracebuild cmd --build $BUILD_ID [--step $PARENT_SPAN_ID] [--name <name>] -- my-cmd --with params
```

After each step:

```
tracebuild step --build $BUILD_ID [--step $PARENT_SPAN_ID] --id $STEP_ID --start-time $STEP_START [--name $STEP_NAME] [--status <success|failure>]
```

After the entire build:

```
tracebuild build --id $BUILD_ID --start-time $BUILD_START [--name $BUILD_NAME] [--branch $BRANCH] [--commit --$COMMIT] [--status <success|failure>]
```

## Configuration

Configure the exporter using environment variables.

| Variable                            | Description                                                                                                                   | Default                |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- | ---------------------- |
| OTEL_TRACES_EXPORTER                | OpenTelemetry traces exporter to use. Supported are: otlp, jaeger, none                                                       | otlp                   |
| OTEL_METRICS_EXPORTER               | OpenTelemetry metrics exporter to use. Supported are: prometheus, none                                                        | none                   |
| OTEL_EXPORTER_OTLP_ENDPOINT         |                                                                                                                               | https://localhost:4317 |
| OTEL_EXPORTER_OTLP_TRACES_ENDPOINT  |                                                                                                                               | https://localhost:4317 |
| OTEL_EXPORTER_OTLP_METRICS_ENDPOINT |                                                                                                                               | https://localhost:4317 |
| OTEL_EXPORTER_JAEGER_AGENT_HOST     |                                                                                                                               | 127.0.0.1              |
| OTEL_EXPORTER_JAEGER_AGENT_PORT     |                                                                                                                               | 6831                   |
| OTEL_EXPORTER_JAEGER_ENDPOINT       | Jaeger collector endpoint. If specified, this is used instead of the Jaeger agent. Example: http://localhost:14268/api/traces |                        |
| OTEL_EXPORTER_JAEGER_USER           | Jaeger collector user for basic auth.                                                                                         |                        |
| OTEL_EXPORTER_JAEGER_PASSWORD       | Jaeger collector password for basic auth.                                                                                     |                        |
| OTEL_EXPORTER_PROMETHEUS_HOST       | Prometheus Pushgateway (or compatible) host                                                                                   | 0.0.0.0                |
| OTEL_EXPORTER_PROMETHEUS_PORT       | Prometheus Pushgateway (or compatible) port                                                                                   | 9464                   |

### Tracing examples

Tracebuild currently works best for tracing systems. See examples for:

- [OpenTelemetry Collector](./examples/otel-collector/)
- [Azure Application Insights](./examples/app-insights/) (using the OpenTelemetry Collector)
- [Jaeger](./examples/jaeger/)

### Metrics examples

The OpenTelemetry metrics specification is still experimental and so is the support for metrics in tracebuild.

Currently the only supported system is Prometheus. The challenge here is that tracebuild only runs for a short amount of time, which doesn't play well with Prometheus' pull-based metrics aggregation. In order to use tracebuild you need to start a [Prometheus Pushgateway](https://github.com/prometheus/pushgateway), which cashes the tracebuild metrics until Prometheus scrapes them. If your build commands run faster than the Prometheus scrape interval you probably want the push gateway to aggregate metrics. For this reason the Prometheus example here uses Weaveworks' [Prometheus Aggregation Gateway](https://github.com/weaveworks/prom-aggregation-gateway):

- [Prometheus](./examples/prometheus/)

Tracebuild exports the following metrics:

- `tracebuild.cmd.count` (labels: `name`, `exit_code`)
- `tracebuild.cmd.duration` (labels: `name`, `exit_code`)
- `tracebuild.step.count` (labels: `name`, `status`)
- `tracebuild.step.duration` (labels: `name`, `status`)
- `tracebuild.build.count` (labels: `name`, `branch`, `status`)
- `tracebuild.build.duration` (labels: `name`, `branch`, `status`)

The duration metrics are exported as histograms for Prometheus. Builds can vary in time quite a bit. In order to still provide a way to see how builds change over time, the histogram contains buckets of 5 min intervals from 5 to 45 mins.
