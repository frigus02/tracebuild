use opentelemetry::{
    sdk::trace::Tracer,
    trace::{SpanId, TraceContextExt as _, TraceError, TraceId, TRACE_FLAG_SAMPLED},
    Context,
};
use std::time::Duration;

pub enum Uninstall {
    Otlp(opentelemetry_otlp::Uninstall),
    Stdout(opentelemetry::sdk::export::trace::stdout::Uninstall),
}

pub fn install_pipeline() -> (Tracer, Uninstall) {
    if let Err(err) = opentelemetry::global::set_error_handler(|err| {
        eprintln!("OpenTelemetry Error: {}", err);
    }) {
        eprintln!("Failed to install OpenTelemetry error handler: {}", err);
    }

    match try_install_otlp_pipeline() {
        Ok(result) => result,
        Err(err) => {
            eprintln!("Failed to install OTLP pipeline: {}", err);
            install_fallback_pipeline()
        }
    }
}

fn try_install_otlp_pipeline() -> Result<(Tracer, Uninstall), TraceError> {
    let (tracer, uninstall) = opentelemetry_otlp::new_pipeline()
        .with_endpoint("http://localhost:4317")
        .with_protocol(opentelemetry_otlp::Protocol::Grpc)
        .with_timeout(Duration::from_secs(5))
        .install()?;
    Ok((tracer, Uninstall::Otlp(uninstall)))
}

fn install_fallback_pipeline() -> (Tracer, Uninstall) {
    let (tracer, uninstall) = opentelemetry::sdk::export::trace::stdout::new_pipeline().install();
    (tracer, Uninstall::Stdout(uninstall))
}

pub fn get_parent_context(build: (TraceId, SpanId), step: Option<SpanId>) -> Context {
    let span_context = opentelemetry::trace::SpanContext::new(
        build.0,
        step.unwrap_or(build.1),
        TRACE_FLAG_SAMPLED,
        true,
        Default::default(),
    );
    Context::current().with_remote_span_context(span_context)
}
