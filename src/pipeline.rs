use futures::{stream::Stream, StreamExt as _};
use opentelemetry::{
    metrics::{Meter, MeterProvider as _, MetricsError},
    sdk::{metrics::PushController, trace::Tracer},
    trace::TraceError,
};
use std::borrow::Cow;
use std::time::Duration;
use thiserror::Error;

pub(crate) struct Pipeline {
    pub(crate) tracer: Tracer,
    pub(crate) meter: Meter,
    _controller: PushController,
    _uninstall: Uninstall,
}

enum Uninstall {
    Otlp(opentelemetry_otlp::Uninstall),
    Jaeger(opentelemetry_jaeger::Uninstall),
    Stdout(opentelemetry::sdk::export::trace::stdout::Uninstall),
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
    let (tracer, uninstall) = match std::env::var("OTEL_TRACES_EXPORTER")
        .map(Cow::from)
        .unwrap_or_else(|_| "otlp".into())
        .as_ref()
    {
        "otlp" => try_install_otlp_traces_pipeline()?,
        "jaeger" => try_install_jaeger_traces_pipeline()?,
        "stdout" => install_stdout_traces_pipeline(),
        exporter => {
            return Err(PipelineError::Other(format!(
                "Unsupported traces exporter {}. Supported are: otlp, jaeger, stdout",
                exporter
            )))
        }
    };

    let controller = match std::env::var("OTEL_METRICS_EXPORTER")
        .map(Cow::from)
        .unwrap_or_else(|_| "otlp".into())
        .as_ref()
    {
        "otlp" => try_install_otlp_metrics_pipeline()?,
        "stdout" => install_stdout_metrics_pipeline(),
        exporter => {
            return Err(PipelineError::Other(format!(
                "Unsupported metrics exporter {}. Supported are: otlp, stdout",
                exporter
            )))
        }
    };

    Ok(Pipeline {
        tracer,
        meter: controller.provider().meter("tracebuild", None),
        _controller: controller,
        _uninstall: uninstall,
    })
}

fn try_install_otlp_traces_pipeline() -> Result<(Tracer, Uninstall), PipelineError> {
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT")
        .or_else(|_| std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT"))
        .unwrap_or_else(|_| "https://localhost:4317".into());
    let (tracer, uninstall) = opentelemetry_otlp::new_pipeline()
        .with_endpoint(endpoint)
        .with_timeout(Duration::from_secs(5))
        .install()?;
    Ok((tracer, Uninstall::Otlp(uninstall)))
}

fn try_install_otlp_metrics_pipeline() -> Result<PushController, PipelineError> {
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_METRICS_ENDPOINT")
        .or_else(|_| std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT"))
        .unwrap_or_else(|_| "https://localhost:4317".into());
    let export_config = opentelemetry_otlp::ExporterConfig {
        endpoint,
        ..Default::default()
    };
    let controller = opentelemetry_otlp::new_metrics_pipeline(tokio::spawn, delayed_interval)
        .with_export_config(export_config)
        .with_timeout(Duration::from_secs(5))
        .build()?;

    Ok(controller)
}

fn try_install_jaeger_traces_pipeline() -> Result<(Tracer, Uninstall), PipelineError> {
    let (tracer, uninstall) = opentelemetry_jaeger::new_pipeline().from_env().install()?;
    Ok((tracer, Uninstall::Jaeger(uninstall)))
}

fn install_stdout_traces_pipeline() -> (Tracer, Uninstall) {
    let (tracer, uninstall) = opentelemetry::sdk::export::trace::stdout::new_pipeline().install();
    (tracer, Uninstall::Stdout(uninstall))
}

fn install_stdout_metrics_pipeline() -> PushController {
    opentelemetry::sdk::export::metrics::stdout(tokio::spawn, delayed_interval)
        .try_init()
        .expect("default quantiles configuration is valid")
}

fn install_fallback_pipeline() -> Pipeline {
    let (tracer, uninstall) = install_stdout_traces_pipeline();
    let controller = install_stdout_metrics_pipeline();

    Pipeline {
        tracer,
        meter: controller.provider().meter("tracebuild", None),
        _controller: controller,
        _uninstall: uninstall,
    }
}

// Skip first immediate tick from tokio
fn delayed_interval(duration: Duration) -> impl Stream<Item = tokio::time::Instant> {
    opentelemetry::util::tokio_interval_stream(duration).skip(1)
}
