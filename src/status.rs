use opentelemetry::trace::StatusCode;
use std::{fmt::Display, str::FromStr};

pub(crate) enum Status {
    Success,
    Failure,
}

impl Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Status::Success => "success",
            Status::Failure => "failure",
        })
    }
}

impl FromStr for Status {
    type Err = Box<dyn std::error::Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "success" => Ok(Status::Success),
            "failure" => Ok(Status::Failure),
            _ => Err("invalid status; valid are: success, failure".into()),
        }
    }
}

impl From<&Status> for StatusCode {
    fn from(status: &Status) -> Self {
        match status {
            Status::Success => StatusCode::Ok,
            Status::Failure => StatusCode::Error,
        }
    }
}
