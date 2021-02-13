use opentelemetry::{sdk::trace::Tracer, trace::TraceError};
use std::borrow::Cow;
use std::time::Duration;

pub enum Uninstall {
    Otlp(opentelemetry_otlp::Uninstall),
    Jaeger(opentelemetry_jaeger::Uninstall),
    Stdout(opentelemetry::sdk::export::trace::stdout::Uninstall),
}

pub fn install_pipeline() -> (Tracer, Uninstall) {
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

fn try_install_chosen_pipeline() -> Result<(Tracer, Uninstall), TraceError> {
    match std::env::var("OTEL_TRACES_EXPORTER")
        .map(Cow::from)
        .unwrap_or_else(|_| "otlp".into())
        .as_ref()
    {
        "otlp" => try_install_otlp_pipeline(),
        "jaeger" => try_install_jaeger_pipeline(),
        exporter => Err(format!(
            "Unsupported exporter {}. Supported are: otlp, jaeger",
            exporter
        )
        .into()),
    }
}

fn try_install_otlp_pipeline() -> Result<(Tracer, Uninstall), TraceError> {
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT")
        .or_else(|_| std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT"))
        .unwrap_or_else(|_| "https://localhost:4317".into());
    let (tracer, uninstall) = opentelemetry_otlp::new_pipeline()
        .with_endpoint(endpoint)
        .with_protocol(opentelemetry_otlp::Protocol::Grpc)
        .with_timeout(Duration::from_secs(5))
        .install()?;
    println!("USING OTLP");
    Ok((tracer, Uninstall::Otlp(uninstall)))
}

fn try_install_jaeger_pipeline() -> Result<(Tracer, Uninstall), TraceError> {
    let (tracer, uninstall) = opentelemetry_jaeger::new_pipeline().from_env().install()?;
    println!("USING JAEGER");
    Ok((tracer, Uninstall::Jaeger(uninstall)))
}

fn install_fallback_pipeline() -> (Tracer, Uninstall) {
    let (tracer, uninstall) = opentelemetry::sdk::export::trace::stdout::new_pipeline().install();
    (tracer, Uninstall::Stdout(uninstall))
}
