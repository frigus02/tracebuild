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
    Pull(Box<dyn PushFunction>),
    None,
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
        .unwrap_or_else(|_| "otlp".into())
        .as_ref()
    {
        "otlp" => try_install_otlp_metrics_pipeline()?,
        "prometheus" => try_install_prometheus_metrics_pipeline()?,
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

fn try_install_jaeger_traces_pipeline() -> Result<(BoxedTracer, TracesUninstall), PipelineError> {
    let (_, uninstall) = opentelemetry_jaeger::new_pipeline().from_env().install()?;
    Ok((
        opentelemetry::global::tracer("tracebuild"),
        TracesUninstall::Jaeger(uninstall),
    ))
}

struct PrometheusPushFunction {
    exporter: opentelemetry_prometheus::PrometheusExporter,
    endpoint: String,
}

impl PushFunction for PrometheusPushFunction {
    fn push(&mut self) -> Result<(), MetricsError> {
        use prometheus::{Encoder as _, TextEncoder};

        let mut metric_families = self.exporter.registry().gather();

        // Sanitize labels
        // This should be done in OpenTelemetry Prometheus exporter
        for mf in metric_families.iter_mut() {
            for m in mf.mut_metric().iter_mut() {
                for l in m.mut_label().iter_mut() {
                    l.set_name(sanitize_prometheus_key(l.get_name()));
                }
            }
        }

        let mut buffer = vec![];
        let encoder = TextEncoder::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();

        let agent = ureq::AgentBuilder::new()
            .timeout_read(Duration::from_secs(5))
            .timeout_write(Duration::from_secs(5))
            .build();
        let response = agent
            .post(&format!("http://{}/metrics/job/tracebuild", self.endpoint))
            .set("content-type", encoder.format_type())
            .send_bytes(&buffer)
            .map_err(|err| MetricsError::Other(err.to_string()))?;
        match response.status() {
            200 | 202 => Ok(()),
            status => Err(MetricsError::Other(format!(
                "unexpected status code {}",
                status
            ))),
        }
    }
}

fn sanitize_prometheus_key<T: AsRef<str>>(raw: T) -> String {
    let mut escaped = raw
        .as_ref()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .peekable();

    let prefix = if escaped.peek().map_or(false, |c| c.is_ascii_digit()) {
        "key_"
    } else if escaped.peek().map_or(false, |&c| c == '_') {
        "key"
    } else {
        ""
    };

    prefix.chars().chain(escaped).take(100).collect()
}

fn try_install_prometheus_metrics_pipeline() -> Result<(Meter, MetricsUninstall), PipelineError> {
    let host = std::env::var("OTEL_EXPORTER_PROMETHEUS_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port = std::env::var("OTEL_EXPORTER_PROMETHEUS_PORT").unwrap_or_else(|_| "9464".into());
    let endpoint = format!("{}:{}", host, port);
    let exporter = opentelemetry_prometheus::exporter()
        .with_default_histogram_boundaries(vec![0., 1., 10., 100., 1000.])
        .try_init()?;
    let meter = exporter.provider()?.meter("tracebuild", None);
    let uninstall = MetricsUninstall::Pull(Box::new(PrometheusPushFunction { exporter, endpoint }));
    Ok((meter, uninstall))
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
