use crate::id::{BuildId, StepId};
use opentelemetry::{
    trace::{TraceContextExt as _, TRACE_FLAG_SAMPLED},
    Context,
};

pub(crate) fn get_parent_context(build: BuildId, step: Option<StepId>) -> Context {
    let span_context = opentelemetry::trace::SpanContext::new(
        build.trace_id(),
        step.map(|s| s.span_id()).unwrap_or_else(|| build.span_id()),
        TRACE_FLAG_SAMPLED,
        true,
        Default::default(),
    );
    Context::current().with_remote_span_context(span_context)
}
