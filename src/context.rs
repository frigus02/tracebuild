use opentelemetry::{
    trace::{SpanId, TraceContextExt as _, TraceId, TRACE_FLAG_SAMPLED},
    Context,
};

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
