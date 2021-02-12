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

Generate IDs

```
BUILD_ID=$(tracebuild id)
SPAN_ID=$(tracebuild id)
```

Wrap each command in:

```
tracebuild cmd --build $BUILD_ID --step $PARENT_SPAN_ID -- my-cmd --with params
```

After each step:

```
tracebuild step --build $BUILD_ID [--step $PARENT_SPAN_ID] --id $SPAN_ID --name $STEP_NAME
```

After the entire build:

```
tracebuild build --id $BUILD_ID --rev $BRANCH --commit --$COMMIT --status $STATUS
```

## Configuration

Configure the exporter using environment variables.

https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/sdk-environment-variables.md
