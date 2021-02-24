mod prometheus;

use futures::{stream::Stream, StreamExt as _};
use opentelemetry::{
    global::BoxedTracer,
    metrics::{noop::NoopMeterCore, Meter, MeterProvider as _, MetricsError},
    sdk::metrics::PushController,
    trace::TraceError,
};
use std::{borrow::Cow, sync::Arc, time::Duration};
use thiserror::Error;

pub(crate) struct Pipeline {
    pub(crate) tracer: BoxedTracer,
    pub(crate) meter: Meter,
    _traces_uninstall: TracesUninstall,
    _metrics_uninstall: MetricsUninstall,
}

enum TracesUninstall {
    Otlp(opentelemetry_otlp::Uninstall),
    Jaeger(opentelemetry_jaeger::Uninstall),
    None,
}

enum MetricsUninstall {
    Push(PushController),
    None,
}

#[derive(Debug, Error)]
enum PipelineError {
    #[error("Trace pipeline failed: {0}")]
    TraceError(#[from] TraceError),
    #[error("Metrics pipeline failed: {0}")]
    MetricsError(#[from] MetricsError),
    #[error("Pipeline failed: {0}")]
    Other(String),
}

pub(crate) fn install_pipeline() -> Pipeline {
    if let Err(err) = opentelemetry::global::set_error_handler(|err| {
        eprintln!("OpenTelemetry Error: {}", err);
    }) {
        eprintln!("Failed to install OpenTelemetry error handler: {}", err);
    }

    match try_install_chosen_pipeline() {
        Ok(result) => result,
        Err(err) => {
            eprintln!(
                "Failed to install chosen OpenTelemetry trace exporter pipeline: {}",
                err
            );
            install_fallback_pipeline()
        }
    }
}

fn try_install_chosen_pipeline() -> Result<Pipeline, PipelineError> {
    let (tracer, traces_uninstall) = match std::env::var("OTEL_TRACES_EXPORTER")
        .map(Cow::from)
        .unwrap_or_else(|_| "otlp".into())
        .as_ref()
    {
        "otlp" => try_install_otlp_traces_pipeline()?,
        "jaeger" => try_install_jaeger_traces_pipeline()?,
        "none" => install_noop_traces_pipeline(),
        exporter => {
            return Err(PipelineError::Other(format!(
                "Unsupported traces exporter {}. Supported are: otlp, jaeger, stdout",
                exporter
            )))
        }
    };

    let (meter, metrics_uninstall) = match std::env::var("OTEL_METRICS_EXPORTER")
        .map(Cow::from)
        .unwrap_or_else(|_| "none".into())
        .as_ref()
    {
        "prometheus" => install_prometheus_metrics_pipeline(),
        "none" => install_noop_metrics_pipeline(),
        exporter => {
            return Err(PipelineError::Other(format!(
                "Unsupported metrics exporter {}. Supported are: otlp, prometheus, stdout",
                exporter
            )))
        }
    };

    Ok(Pipeline {
        tracer,
        meter,
        _traces_uninstall: traces_uninstall,
        _metrics_uninstall: metrics_uninstall,
    })
}

fn try_install_otlp_traces_pipeline() -> Result<(BoxedTracer, TracesUninstall), PipelineError> {
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT")
        .or_else(|_| std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT"))
        .unwrap_or_else(|_| "https://localhost:4317".into());
    let (_, uninstall) = opentelemetry_otlp::new_pipeline()
        .with_endpoint(endpoint)
        .with_timeout(Duration::from_secs(5))
        .install()?;
    Ok((
        opentelemetry::global::tracer("tracebuild"),
        TracesUninstall::Otlp(uninstall),
    ))
}

fn try_install_jaeger_traces_pipeline() -> Result<(BoxedTracer, TracesUninstall), PipelineError> {
    let (_, uninstall) = opentelemetry_jaeger::new_pipeline().from_env().install()?;
    Ok((
        opentelemetry::global::tracer("tracebuild"),
        TracesUninstall::Jaeger(uninstall),
    ))
}

fn install_prometheus_metrics_pipeline() -> (Meter, MetricsUninstall) {
    let controller =
        prometheus::build_metrics_pipeline(tokio::spawn, delayed_interval, "tracebuild");
    (
        controller.provider().meter("tracebuild", None),
        MetricsUninstall::Push(controller),
    )
}

fn install_noop_traces_pipeline() -> (BoxedTracer, TracesUninstall) {
    (
        opentelemetry::global::tracer("tracebuild"),
        TracesUninstall::None,
    )
}

fn install_noop_metrics_pipeline() -> (Meter, MetricsUninstall) {
    let meter = Meter::new("tracebuild", None, Arc::new(NoopMeterCore::new()));
    (meter, MetricsUninstall::None)
}

fn install_fallback_pipeline() -> Pipeline {
    let (tracer, traces_uninstall) = install_noop_traces_pipeline();
    let (meter, metrics_uninstall) = install_noop_metrics_pipeline();

    Pipeline {
        tracer,
        meter,
        _traces_uninstall: traces_uninstall,
        _metrics_uninstall: metrics_uninstall,
    }
}

// Skip first immediate tick from tokio
fn delayed_interval(duration: Duration) -> impl Stream<Item = tokio::time::Instant> {
    opentelemetry::util::tokio_interval_stream(duration).skip(1)
}
