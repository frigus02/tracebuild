use futures::{stream::Stream, StreamExt as _};
use opentelemetry::{
    metrics::{Meter, MeterProvider as _, MetricsError},
    sdk::{metrics::PushController, trace::Tracer},
    trace::TraceError,
};
use std::borrow::Cow;
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;

pub(crate) struct Pipeline {
    pub(crate) tracer: Tracer,
    pub(crate) meter: Meter,
    _traces_uninstall: TracesUninstall,
    _metrics_uninstall: MetricsUninstall,
}

enum TracesUninstall {
    Otlp(opentelemetry_otlp::Uninstall),
    Jaeger(opentelemetry_jaeger::Uninstall),
    Stdout(opentelemetry::sdk::export::trace::stdout::Uninstall),
}

enum MetricsUninstall {
    Push(PushController),
    Pull(Box<dyn PushFunction>),
}

impl Drop for MetricsUninstall {
    fn drop(&mut self) {
        if let Self::Pull(push_func) = self {
            if let Err(err) = push_func.push() {
                opentelemetry::global::handle_error(err);
            }
        }
    }
}

trait PushFunction {
    fn push(&mut self) -> Result<(), MetricsError>;
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
        "stdout" => install_stdout_traces_pipeline(),
        exporter => {
            return Err(PipelineError::Other(format!(
                "Unsupported traces exporter {}. Supported are: otlp, jaeger, stdout",
                exporter
            )))
        }
    };

    let (meter, metrics_uninstall) = match std::env::var("OTEL_METRICS_EXPORTER")
        .map(Cow::from)
        .unwrap_or_else(|_| "otlp".into())
        .as_ref()
    {
        "otlp" => try_install_otlp_metrics_pipeline()?,
        "prometheus" => try_install_prometheus_metrics_pipeline()?,
        "stdout" => install_stdout_metrics_pipeline(),
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

fn try_install_otlp_traces_pipeline() -> Result<(Tracer, TracesUninstall), PipelineError> {
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT")
        .or_else(|_| std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT"))
        .unwrap_or_else(|_| "https://localhost:4317".into());
    let (tracer, uninstall) = opentelemetry_otlp::new_pipeline()
        .with_endpoint(endpoint)
        .with_timeout(Duration::from_secs(5))
        .install()?;
    Ok((tracer, TracesUninstall::Otlp(uninstall)))
}

fn try_install_otlp_metrics_pipeline() -> Result<(Meter, MetricsUninstall), PipelineError> {
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

    Ok((
        controller.provider().meter("tracebuild", None),
        MetricsUninstall::Push(controller),
    ))
}

fn try_install_jaeger_traces_pipeline() -> Result<(Tracer, TracesUninstall), PipelineError> {
    let (tracer, uninstall) = opentelemetry_jaeger::new_pipeline().from_env().install()?;
    Ok((tracer, TracesUninstall::Jaeger(uninstall)))
}

struct PrometheusPushFunction {
    exporter: opentelemetry_prometheus::PrometheusExporter,
    endpoint: String,
}

impl PushFunction for PrometheusPushFunction {
    fn push(&mut self) -> Result<(), MetricsError> {
        let metric_families = self.exporter.registry().gather();
        prometheus::push_metrics(
            "tracebuild",
            HashMap::new(),
            &self.endpoint,
            metric_families,
            None,
        )
        .map_err(|err| MetricsError::Other(err.to_string()))
    }
}

fn try_install_prometheus_metrics_pipeline() -> Result<(Meter, MetricsUninstall), PipelineError> {
    let host = std::env::var("OTEL_EXPORTER_PROMETHEUS_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port = std::env::var("OTEL_EXPORTER_PROMETHEUS_PORT").unwrap_or_else(|_| "9464".into());
    let endpoint = format!("{}:{}", host, port);
    let exporter = opentelemetry_prometheus::exporter().try_init()?;
    let meter = exporter.provider()?.meter("tracebuild", None);
    let uninstall = MetricsUninstall::Pull(Box::new(PrometheusPushFunction { exporter, endpoint }));
    Ok((meter, uninstall))
}

fn install_stdout_traces_pipeline() -> (Tracer, TracesUninstall) {
    let (tracer, uninstall) = opentelemetry::sdk::export::trace::stdout::new_pipeline().install();
    (tracer, TracesUninstall::Stdout(uninstall))
}

fn install_stdout_metrics_pipeline() -> (Meter, MetricsUninstall) {
    let controller = opentelemetry::sdk::export::metrics::stdout(tokio::spawn, delayed_interval)
        .try_init()
        .expect("default quantiles configuration is valid");
    (
        controller.provider().meter("tracebuild", None),
        MetricsUninstall::Push(controller),
    )
}

fn install_fallback_pipeline() -> Pipeline {
    let (tracer, traces_uninstall) = install_stdout_traces_pipeline();
    let (meter, metrics_uninstall) = install_stdout_metrics_pipeline();

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
