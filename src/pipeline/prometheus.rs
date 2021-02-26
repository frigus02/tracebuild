use opentelemetry::metrics::MetricsError;
use opentelemetry_prometheus::PrometheusExporter;
use prometheus::{proto::MetricFamily, Encoder as _, TextEncoder};
use std::time::Duration;

pub(crate) struct PrometheusPushOnDropExporter {
    exporter: PrometheusExporter,
    endpoint: String,
}

impl Drop for PrometheusPushOnDropExporter {
    fn drop(&mut self) {
        let mut metric_families = self.exporter.registry().gather();
        sanitize_label_names(&mut metric_families);
        if let Err(err) = push_metrics(metric_families, &self.endpoint) {
            opentelemetry::global::handle_error(err);
        }
    }
}

pub(crate) fn new_prometheus_push_on_drop_exporter(
) -> Result<PrometheusPushOnDropExporter, MetricsError> {
    let host = std::env::var("OTEL_EXPORTER_PROMETHEUS_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port = std::env::var("OTEL_EXPORTER_PROMETHEUS_PORT").unwrap_or_else(|_| "9464".into());
    let endpoint = format!("{}:{}", host, port);
    let exporter = opentelemetry_prometheus::exporter()
        .with_default_histogram_boundaries(vec![
            1.,    // 1 sec
            10.,   // 10 secs
            30.,   // 30 secs
            60.,   // 1 min
            300.,  // 5 mins
            600.,  // 10 mins
            900.,  // 15 mins
            1200., // 20 mins
            1500., // 25 mins
            1800., // 30 mins
            2100., // 35 mins
            2400., // 40 mins
            2700., // 45 mins
        ])
        .try_init()?;
    Ok(PrometheusPushOnDropExporter { exporter, endpoint })
}

// Sanitize labels
// Remove once https://github.com/open-telemetry/opentelemetry-rust/pull/462 is in
fn sanitize_label_names(metric_families: &mut Vec<MetricFamily>) {
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

    for mf in metric_families.iter_mut() {
        for m in mf.mut_metric().iter_mut() {
            for l in m.mut_label().iter_mut() {
                l.set_name(sanitize_prometheus_key(l.get_name()));
            }
        }
    }
}

fn push_metrics(metric_families: Vec<MetricFamily>, endpoint: &str) -> Result<(), MetricsError> {
    let mut buffer = vec![];
    let encoder = TextEncoder::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(5))
        .build();
    let _response = agent
        .post(&format!("http://{}/metrics/job/tracebuild", endpoint))
        .set("content-type", encoder.format_type())
        .send_bytes(&buffer)
        .map_err(|err| {
            MetricsError::Other(format!(
                "Failed to send metrics to Prometheus push gateway: {}",
                err
            ))
        })?;
    Ok(())
}
