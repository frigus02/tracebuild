[![Crates.io](https://img.shields.io/crates/v/tracebuild.svg)](https://crates.io/crates/tracebuild)
[![Workflow Status](https://github.com/frigus02/tracebuild/workflows/CI/badge.svg)](https://github.com/frigus02/tracebuild/actions?query=workflow%3A%22CI%22)

# tracebuild

A small binary to instrument builds in systems like GitHub Actions, Travis CI, etc. It uses [OpenTelemetry](https://opentelemetry.io/) under the hood, which means you should be able to integrate it in your existing telemetry platform.

## Usage

Install the binary in your build:

```
curl -L -o tracebuild https://github.com/frigus02/tracebuild/releases/latest/download/tracebuild-linux-amd64
chmod +x tracebuild
```

Generate IDs and start times

```
BUILD_ID=$(tracebuild id)
STEP_ID=$(tracebuild id)
BUILD_START=$(tracebuild now)
STEP_START=$(tracebuild now)
```

Wrap each command in:

```
tracebuild cmd --build $BUILD_ID [--step $PARENT_SPAN_ID] -- my-cmd --with params
```

After each step:

```
tracebuild step --build $BUILD_ID [--step $PARENT_SPAN_ID] --id $STEP_ID [--name $STEP_NAME]
```

After the entire build:

```
tracebuild build --id $BUILD_ID --branch $BRANCH --commit --$COMMIT --status $STATUS [--name $BUILD_NAME]
```

## Configuration

Configure the exporter using environment variables.

| Variable                           | Description                                                                                                                   | Default                |
| ---------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- | ---------------------- |
| OTEL_TRACES_EXPORTER               | OpenTelemetry trace exporter to use. Supported are: otlp, jaeger                                                              | otlp                   |
| OTEL_EXPORTER_OTLP_ENDPOINT        |                                                                                                                               | https://localhost:4317 |
| OTEL_EXPORTER_OTLP_TRACES_ENDPOINT |                                                                                                                               | https://localhost:4317 |
| OTEL_EXPORTER_JAEGER_AGENT_HOST    |                                                                                                                               | 127.0.0.1              |
| OTEL_EXPORTER_JAEGER_AGENT_PORT    |                                                                                                                               | 6831                   |
| OTEL_EXPORTER_JAEGER_ENDPOINT      | Jaeger collector endpoint. If specified, this is used instead of the Jaeger agent. Example: http://localhost:14268/api/traces |                        |
| OTEL_EXPORTER_JAEGER_USER          | Jaeger collector user for basic auth.                                                                                         |                        |
| OTEL_EXPORTER_JAEGER_PASSWORD      | Jaeger collector password for basic auth.                                                                                     |                        |
