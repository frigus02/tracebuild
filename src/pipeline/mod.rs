mod prometheus;

use opentelemetry::{
    global::BoxedTracer,
    metrics::{Meter, MetricsError},
    trace::TraceError,
};
use std::time::Duration;
use thiserror::Error;

pub(crate) struct Pipeline {
    pub(crate) tracer: BoxedTracer,
    pub(crate) meter: Meter,
    _traces_uninstall: TracesUninstall,
    _metrics_uninstall: MetricsUninstall,
}

impl Pipeline {
    fn new(traces_uninstall: TracesUninstall, metrics_uninstall: MetricsUninstall) -> Self {
        Self {
            tracer: opentelemetry::global::tracer("tracebuild"),
            meter: opentelemetry::global::meter("tracebuild"),
            _traces_uninstall: traces_uninstall,
            _metrics_uninstall: metrics_uninstall,
        }
    }
}

enum TracesUninstall {
    Otlp(opentelemetry_otlp::Uninstall),
    Jaeger(opentelemetry_jaeger::Uninstall),
    None,
}

enum MetricsUninstall {
    Prometheus(prometheus::PrometheusPushOnDropExporter),
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
    let traces_uninstall = match std::env::var("OTEL_TRACES_EXPORTER")
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

    let metrics_uninstall = match std::env::var("OTEL_METRICS_EXPORTER")
        .unwrap_or_else(|_| "none".into())
        .as_ref()
    {
        "prometheus" => try_install_prometheus_metrics_pipeline()?,
        "none" => install_noop_metrics_pipeline(),
        exporter => {
            return Err(PipelineError::Other(format!(
                "Unsupported metrics exporter {}. Supported are: otlp, prometheus, stdout",
                exporter
            )))
        }
    };

    Ok(Pipeline::new(traces_uninstall, metrics_uninstall))
}

fn try_install_otlp_traces_pipeline() -> Result<TracesUninstall, PipelineError> {
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT")
        .or_else(|_| std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT"))
        .unwrap_or_else(|_| "https://localhost:4317".into());
    let (_, uninstall) = opentelemetry_otlp::new_pipeline()
        .with_endpoint(endpoint)
        .with_timeout(Duration::from_secs(5))
        .install()?;
    Ok(TracesUninstall::Otlp(uninstall))
}

fn try_install_jaeger_traces_pipeline() -> Result<TracesUninstall, PipelineError> {
    let (_, uninstall) = opentelemetry_jaeger::new_pipeline().from_env().install()?;
    Ok(TracesUninstall::Jaeger(uninstall))
}

fn try_install_prometheus_metrics_pipeline() -> Result<MetricsUninstall, PipelineError> {
    let exporter = prometheus::new_prometheus_push_on_drop_exporter()?;
    Ok(MetricsUninstall::Prometheus(exporter))
}

fn install_noop_traces_pipeline() -> TracesUninstall {
    TracesUninstall::None
}

fn install_noop_metrics_pipeline() -> MetricsUninstall {
    MetricsUninstall::None
}

fn install_fallback_pipeline() -> Pipeline {
    let traces_uninstall = install_noop_traces_pipeline();
    let metrics_uninstall = install_noop_metrics_pipeline();

    Pipeline::new(traces_uninstall, metrics_uninstall)
}
