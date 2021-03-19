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
TRACEBUILD_BUILD_ID=$(tracebuild id)
TRACEBUILD_BUILD_START=$(tracebuild now)
TRACEBUILD_STEP_ID=$(tracebuild id)
TRACEBUILD_STEP_START=$(tracebuild now)
```

Wrap each command in:

```
tracebuild cmd --build $TRACEBUILD_BUILD_ID [--step $TRACEBUILD_STEP_ID] [--name <name>] [--build-name <build_name>] -- my-cmd --with params
```

After each step:

```
tracebuild step --build $TRACEBUILD_BUILD_ID [--step $PARENT_SPAN_ID] --id $TRACEBUILD_STEP_ID --start-time $TRACEBUILD_STEP_START [--name <step_name>] [--build-name <build_name>] [--status <success|failure>]
```

After the entire build:

```
tracebuild build --id $TRACEBUILD_BUILD_ID --start-time $TRACEBUILD_BUILD_START [--name $TRACEBUILD_BUILD_NAME] [--branch <branch>] [--commit <commit>] [--status <success|failure>]
```

## Configuration

Configure the exporter using environment variables.

| Variable                           | Description                                                                                                                   | Default                |
| ---------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- | ---------------------- |
| OTEL_TRACES_EXPORTER               | OpenTelemetry traces exporter to use. Supported are: otlp, jaeger, none                                                       | otlp                   |
| OTEL_METRICS_EXPORTER              | OpenTelemetry metrics exporter to use. Supported are: prometheus, none                                                        | none                   |
| OTEL_EXPORTER_OTLP_ENDPOINT        | OpenTelemetry Collector endpoint                                                                                              | https://localhost:4317 |
| OTEL_EXPORTER_OTLP_TRACES_ENDPOINT | OpenTelemetry Collector endpoint for traces (takes priority over the generic variable)                                        | https://localhost:4317 |
| OTEL_EXPORTER_JAEGER_AGENT_HOST    | Jaeger agent host                                                                                                             | 127.0.0.1              |
| OTEL_EXPORTER_JAEGER_AGENT_PORT    | Jaeger agent port                                                                                                             | 6831                   |
| OTEL_EXPORTER_JAEGER_ENDPOINT      | Jaeger collector endpoint. If specified, this is used instead of the Jaeger agent. Example: http://localhost:14268/api/traces |                        |
| OTEL_EXPORTER_JAEGER_USER          | Jaeger collector user for basic auth.                                                                                         |                        |
| OTEL_EXPORTER_JAEGER_PASSWORD      | Jaeger collector password for basic auth.                                                                                     |                        |
| OTEL_EXPORTER_PROMETHEUS_HOST      | Prometheus Pushgateway (or compatible) host                                                                                   | 0.0.0.0                |
| OTEL_EXPORTER_PROMETHEUS_PORT      | Prometheus Pushgateway (or compatible) port                                                                                   | 9464                   |

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

- `tracebuild.cmd.duration` (labels: `name`, `build_name`, `exit_code`)
- `tracebuild.step.duration` (labels: `name`, `build_name`, `status`)
- `tracebuild.build.duration` (labels: `name`, `branch`, `status`)

The duration metrics are exported as histograms for Prometheus. Builds can vary in time quite a bit. In order to still provide a way to see how builds change over time, the histogram contains buckets of 5 min intervals from 5 to 45 mins.
