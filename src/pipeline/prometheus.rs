use futures::Stream;
use opentelemetry::{
    global, labels,
    metrics::{Descriptor, MetricsError, NumberKind},
    sdk::{
        export::metrics::{
            CheckpointSet, ExportKind, ExportKindFor, ExportKindSelector, Exporter, Points, Record,
            Sum,
        },
        metrics::{
            aggregators::{ArrayAggregator, SumAggregator},
            controllers, selectors, PushController, PushControllerWorker,
        },
    },
    KeyValue,
};
use prometheus::{proto::MetricFamily, Encoder as _, TextEncoder};
use std::{sync::Arc, time::Duration};

pub(crate) fn build_metrics_pipeline<SP, SO, I, IO, IOI>(
    spawn: SP,
    interval: I,
    push_job_name: &str,
) -> PushController
where
    SP: Fn(PushControllerWorker) -> SO,
    I: Fn(Duration) -> IO,
    IO: Stream<Item = IOI> + Send + 'static,
{
    let host = std::env::var("OTEL_EXPORTER_PROMETHEUS_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port = std::env::var("OTEL_EXPORTER_PROMETHEUS_PORT").unwrap_or_else(|_| "9464".into());
    let export_kind_selector = ExportKindSelector::Cumulative;
    let exporter =
        PrometheusExporter::new(&host, &port, push_job_name, export_kind_selector.clone());

    let controller = controllers::push(
        selectors::simple::Selector::Exact,
        export_kind_selector,
        exporter,
        spawn,
        interval,
    )
    .build();
    global::set_meter_provider(controller.provider());
    controller
}

#[derive(Clone, Debug)]
struct PrometheusExporter {
    agent: ureq::Agent,
    endpoint: String,
    export_kind_selector: Arc<dyn ExportKindFor + Send + Sync>,
}

impl PrometheusExporter {
    fn new<T: ExportKindFor + Send + Sync + 'static>(
        host: &str,
        port: &str,
        push_job_name: &str,
        export_selector: T,
    ) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(5))
            .build();
        let endpoint = format!("http://{}:{}/metrics/job/{}", host, port, push_job_name);

        Self {
            agent,
            endpoint,
            export_kind_selector: Arc::new(export_selector),
        }
    }
}

impl ExportKindFor for PrometheusExporter {
    fn export_kind_for(&self, descriptor: &Descriptor) -> ExportKind {
        self.export_kind_selector.export_kind_for(descriptor)
    }
}

impl Exporter for PrometheusExporter {
    fn export(&self, checkpoint_set: &mut dyn CheckpointSet) -> Result<(), MetricsError> {
        let mut metric_families: Vec<MetricFamily> = Vec::new();
        checkpoint_set.try_for_each(self.export_kind_selector.as_ref(), &mut |record| {
            match otel_record_into_prom_metric_family(record) {
                Ok(metric_family) => {
                    metric_families.push(metric_family);
                    Ok(())
                }
                Err(err) => Err(err),
            }
        })?;
        push_metrics(&self.agent, &self.endpoint, metric_families)
    }
}

fn push_metrics(
    agent: &ureq::Agent,
    endpoint: &str,
    metric_families: Vec<MetricFamily>,
) -> Result<(), MetricsError> {
    let mut buffer = vec![];
    let encoder = TextEncoder::new();
    encoder
        .encode(&metric_families, &mut buffer)
        .map_err(|err| MetricsError::Other(format!("Failed to encode metric families: {}", err)))?;

    let _response = agent
        .post(endpoint)
        .set("content-type", encoder.format_type())
        .send_bytes(&buffer)
        .map_err(|err| {
            MetricsError::Other(format!("Failed to send metrics to push gateway: {}", err))
        })?;
    Ok(())
}

fn otel_record_into_prom_metric_family(record: &Record) -> Result<MetricFamily, MetricsError> {
    let agg = record.aggregator().ok_or(MetricsError::NoDataCollected)?;
    let number_kind = record.descriptor().number_kind();

    let name = sanitize(record.descriptor().name().to_owned());
    let help = record
        .descriptor()
        .description()
        .cloned()
        .unwrap_or_else(|| record.descriptor().name().to_owned());

    let mut label_values = Vec::new();
    merge_labels(record, &mut label_values);

    if let Some(sum) = agg.as_any().downcast_ref::<SumAggregator>() {
        build_counter(sum, number_kind, name, help, label_values)
    } else if let Some(arr) = agg.as_any().downcast_ref::<ArrayAggregator>() {
        build_gauge(arr, number_kind, name, help, label_values)
    } else {
        Err(MetricsError::Other("unexpected aggregator".into()))
    }
}

fn build_gauge(
    arr: &ArrayAggregator,
    kind: &NumberKind,
    name: String,
    help: String,
    labels: Vec<KeyValue>,
) -> Result<MetricFamily, MetricsError> {
    let points = arr.points()?;
    let last_value = points.last().ok_or(MetricsError::NoDataCollected)?;

    let mut g = prometheus::proto::Gauge::default();
    g.set_value(last_value.to_f64(kind));

    let mut m = prometheus::proto::Metric::default();
    m.set_label(protobuf::RepeatedField::from_vec(
        labels.into_iter().map(build_label_pair).collect(),
    ));
    m.set_gauge(g);

    let mut mf = MetricFamily::default();
    mf.set_name(name);
    mf.set_help(help);
    mf.set_field_type(prometheus::proto::MetricType::GAUGE);
    mf.set_metric(protobuf::RepeatedField::from_vec(vec![m]));

    Ok(mf)
}

fn build_counter(
    sum: &SumAggregator,
    kind: &NumberKind,
    name: String,
    help: String,
    labels: Vec<KeyValue>,
) -> Result<MetricFamily, MetricsError> {
    let sum = sum.sum()?;

    let mut c = prometheus::proto::Counter::default();
    c.set_value(sum.to_f64(kind));

    let mut m = prometheus::proto::Metric::default();
    m.set_label(protobuf::RepeatedField::from_vec(
        labels.into_iter().map(build_label_pair).collect(),
    ));
    m.set_counter(c);

    let mut mf = MetricFamily::default();
    mf.set_name(name);
    mf.set_help(help);
    mf.set_field_type(prometheus::proto::MetricType::COUNTER);
    mf.set_metric(protobuf::RepeatedField::from_vec(vec![m]));

    Ok(mf)
}

fn build_label_pair(label: KeyValue) -> prometheus::proto::LabelPair {
    let mut lp = prometheus::proto::LabelPair::new();
    lp.set_name(label.key.into());
    lp.set_value(label.value.to_string());
    lp
}

fn merge_labels(record: &Record<'_>, values: &mut Vec<KeyValue>) {
    // Duplicate keys are resolved by taking the record label value over the resource value.
    let iter = labels::merge_iters(record.labels().iter(), record.resource().iter());
    for (key, value) in iter {
        values.push(KeyValue::new(sanitize(key.as_str()), value.clone()));
    }
}

/// sanitize returns a string that is truncated to 100 characters if it's too
/// long, and replaces non-alphanumeric characters to underscores.
fn sanitize<T: AsRef<str>>(raw: T) -> String {
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
