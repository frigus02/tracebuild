use opentelemetry::trace::{SpanId, TraceId};
use rand::prelude::*;
use std::fmt::Display;
use std::str::FromStr;

pub(crate) struct BuildId {
    trace: u128,
    span: u64,
}

impl BuildId {
    pub(crate) fn generate() -> Self {
        Self {
            trace: rand::thread_rng().gen(),
            span: rand::thread_rng().gen(),
        }
    }

    pub(crate) fn trace_id(&self) -> TraceId {
        TraceId::from_u128(self.trace)
    }

    pub(crate) fn span_id(&self) -> SpanId {
        SpanId::from_u64(self.span)
    }
}

impl FromStr for BuildId {
    type Err = Box<dyn std::error::Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 48 {
            return Err("string len is not 48".into());
        }

        let (s_trace, s_span) = s.split_at(32);
        let trace = u128::from_str_radix(s_trace, 16)?;
        let span = u64::from_str_radix(s_span, 16)?;
        Ok(Self { trace, span })
    }
}

impl Display for BuildId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:032x}{:016x}", self.trace, self.span)
    }
}

pub(crate) struct StepId(BuildId);

impl StepId {
    pub(crate) fn span_id(&self) -> SpanId {
        self.0.span_id()
    }
}

impl FromStr for StepId {
    type Err = Box<dyn std::error::Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        BuildId::from_str(s).map(Self)
    }
}

impl Display for StepId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
