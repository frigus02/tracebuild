mod prometheus;

use opentelemetry::{
    global::BoxedTracer,
    metrics::{Meter, MetricsError},
    trace::TraceError,
};
use std::sync::Mutex;
use thiserror::Error;

pub(crate) fn tracer() -> BoxedTracer {
    opentelemetry::global::tracer("tracebuild")
}

pub(crate) fn meter() -> Meter {
    opentelemetry::global::meter("tracebuild")
}

lazy_static::lazy_static! {
    static ref GLOBAL_PROMETHEUS_EXPORTER: Mutex<Option<prometheus::PrometheusPushOnDropExporter>> = Mutex::new(None);
}

fn set_global_prometheus_exporter(exporter: Option<prometheus::PrometheusPushOnDropExporter>) {
    let mut global_exporter = GLOBAL_PROMETHEUS_EXPORTER
        .lock()
        .expect("GLOBAL_PROMETHEUS_EXPORTER Mutex poisoned");
    *global_exporter = exporter;
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

pub(crate) fn install_pipeline() {
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
        }
    };
}

pub(crate) fn shutdown_pipeline() {
    opentelemetry::global::shutdown_tracer_provider();

    set_global_prometheus_exporter(None);
    opentelemetry::global::set_meter_provider(
        opentelemetry::metrics::noop::NoopMeterProvider::default(),
    );
}

fn try_install_chosen_pipeline() -> Result<(), PipelineError> {
    match std::env::var("OTEL_TRACES_EXPORTER")
        .unwrap_or_else(|_| "otlp".into())
        .as_ref()
    {
        "otlp" => try_install_otlp_traces_pipeline()?,
        "jaeger" => try_install_jaeger_traces_pipeline()?,
        "none" => {}
        exporter => {
            return Err(PipelineError::Other(format!(
                "Unsupported traces exporter {}. Supported are: otlp, jaeger, stdout",
                exporter
            )))
        }
    };

    match std::env::var("OTEL_METRICS_EXPORTER")
        .unwrap_or_else(|_| "none".into())
        .as_ref()
    {
        "prometheus" => try_install_prometheus_metrics_pipeline()?,
        "none" => {}
        exporter => {
            return Err(PipelineError::Other(format!(
                "Unsupported metrics exporter {}. Supported are: otlp, prometheus, stdout",
                exporter
            )))
        }
    };

    Ok(())
}

fn try_install_otlp_traces_pipeline() -> Result<(), PipelineError> {
    let _tracer = opentelemetry_otlp::new_pipeline()
        .with_env()
        .with_tonic()
        .install_batch(opentelemetry::runtime::Tokio)?;
    Ok(())
}

fn try_install_jaeger_traces_pipeline() -> Result<(), PipelineError> {
    let _tracer =
        opentelemetry_jaeger::new_pipeline().install_batch(opentelemetry::runtime::Tokio)?;
    Ok(())
}

fn try_install_prometheus_metrics_pipeline() -> Result<(), PipelineError> {
    let exporter = prometheus::new_prometheus_push_on_drop_exporter()?;
    set_global_prometheus_exporter(Some(exporter));
    Ok(())
}
