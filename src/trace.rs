use opentelemetry::{
    sdk::{
        trace::{Config, Tracer},
        Resource,
    },
    trace::{SpanId, TraceContextExt as _, TraceError, TraceId},
    Context, KeyValue,
};
use std::time::Duration;

pub enum Uninstall {
    Otlp(opentelemetry_otlp::Uninstall),
    Stdout(opentelemetry::sdk::export::trace::stdout::Uninstall),
}

pub fn install_pipeline() -> (Tracer, Uninstall) {
    try_install_pipeline().unwrap_or_else(|_| install_fallback_pipeline())
}

fn get_config() -> Config {
    opentelemetry::sdk::trace::config()
        .with_resource(Resource::new(vec![KeyValue::new("key", "value")]))
}

fn try_install_pipeline() -> Result<(Tracer, Uninstall), TraceError> {
    let (tracer, uninstall) = opentelemetry_otlp::new_pipeline()
        .with_timeout(Duration::from_secs(3))
        .with_trace_config(get_config())
        .install()?;
    Ok((tracer, Uninstall::Otlp(uninstall)))
}

fn install_fallback_pipeline() -> (Tracer, Uninstall) {
    let (tracer, uninstall) = opentelemetry::sdk::export::trace::stdout::new_pipeline()
        .with_trace_config(get_config())
        .install();
    (tracer, Uninstall::Stdout(uninstall))
}

pub fn get_parent_context(build: (TraceId, SpanId), step: Option<SpanId>) -> Context {
    let span_context = opentelemetry::trace::SpanContext::new(
        build.0,
        step.unwrap_or(build.1),
        0,
        true,
        Default::default(),
    );
    Context::current().with_remote_span_context(span_context)
}
