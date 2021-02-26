//! A small binary to instrument builds in systems like GitHub Actions, Travis CI, etc. It uses
//! [OpenTelemetry](https://opentelemetry.io/) under the hood, which means you should be able to
//! integrate it in your existing telemetry platform.
#![deny(missing_docs, unreachable_pub, missing_debug_implementations)]

mod cmd;
mod context;
mod id;
mod pipeline;

use opentelemetry::{
    metrics::Meter,
    trace::{FutureExt, Span, SpanId, SpanKind, StatusCode, TraceContextExt, TraceId, Tracer},
    Context, Key, KeyValue, Unit,
};
use std::{
    borrow::Cow,
    fmt::Display,
    str::FromStr,
    time::{Duration, SystemTime},
};
use structopt::StructOpt;

fn parse_build_id(src: &str) -> Result<(TraceId, SpanId), Box<dyn std::error::Error>> {
    let id: id::ID = src.parse()?;
    Ok((id.trace_id(), id.span_id()))
}

fn parse_step_id(src: &str) -> Result<SpanId, Box<dyn std::error::Error>> {
    let id: id::ID = src.parse()?;
    Ok(id.span_id())
}

fn parse_system_time(src: &str) -> Result<SystemTime, Box<dyn std::error::Error>> {
    let secs_since_epoch = u64::from_str_radix(src, 10)?;
    let since_epoch = Duration::from_secs(secs_since_epoch);
    Ok(SystemTime::UNIX_EPOCH
        .checked_add(since_epoch)
        .ok_or("secs is too large")?)
}

enum Status {
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

fn record_event_duration(meter: &Meter, name: &str, start_time: SystemTime, labels: &[KeyValue]) {
    let duration = start_time.elapsed().unwrap_or_default();
    match meter
        .f64_value_recorder(name)
        .with_unit(Unit::new("seconds"))
        .try_init()
    {
        Ok(value_recorder) => value_recorder.record(duration.as_secs_f64(), labels),
        Err(err) => eprintln!("Failed to record duration {}: {}", name, err),
    }
}

fn record_event_count(meter: &Meter, name: &str, labels: &[KeyValue]) {
    match meter.u64_counter(name).try_init() {
        Ok(value_recorder) => value_recorder.add(1, labels),
        Err(err) => eprintln!("Failed to record count {}: {}", name, err),
    }
}

#[derive(StructOpt)]
enum Args {
    /// Generates an ID, which can be used as either a span or build id.
    ID,
    /// Generates timestamp, which can be used as a build or span start time.
    Now,
    /// Executes the specified command and reports a span using the configured OpenTelemetry
    /// exporter.
    Cmd {
        /// Build ID
        #[structopt(long = "build", parse(try_from_str = parse_build_id))]
        build: (TraceId, SpanId),
        /// Optional parent step ID
        #[structopt(long = "step", parse(try_from_str = parse_step_id))]
        step: Option<SpanId>,
        /// Optional name. Falls back to cmd + args for traces and cmd for metrics
        #[structopt(long = "name")]
        name: Option<String>,
        /// Command name
        #[structopt(name = "CMD")]
        cmd: String,
        /// Command arguments
        #[structopt(name = "ARGS")]
        args: Vec<String>,
    },
    /// Reports a span using the configured OpenTelemetry exporter with references to the given
    /// build and optional parent step.
    Step {
        /// Build ID
        #[structopt(long = "build", parse(try_from_str = parse_build_id))]
        build: (TraceId, SpanId),
        /// Optional parent step ID
        #[structopt(long = "step", parse(try_from_str = parse_step_id))]
        step: Option<SpanId>,
        /// Step ID
        #[structopt(long = "id", parse(try_from_str = parse_step_id))]
        id: SpanId,
        /// Start time
        #[structopt(long = "start-time", parse(try_from_str = parse_system_time))]
        start_time: SystemTime,
        /// Optional name
        #[structopt(long = "name")]
        name: Option<String>,
        /// Optional status
        #[structopt(long = "status")]
        status: Option<Status>,
    },
    /// Reports a span using the configured OpenTelemetry exporter with the given ID and metadata.
    Build {
        /// Build ID
        #[structopt(long = "id", parse(try_from_str = parse_build_id))]
        id: (TraceId, SpanId),
        /// Start time
        #[structopt(long = "start-time", parse(try_from_str = parse_system_time))]
        start_time: SystemTime,
        /// Optional name
        #[structopt(long = "name")]
        name: Option<String>,
        /// Optional branch name. Included in metrics, so should be low cardinality if metrics are enabled.
        #[structopt(long = "branch")]
        branch: Option<String>,
        /// Optioanl commit SHA
        #[structopt(long = "commit")]
        commit: Option<String>,
        /// Optional status
        #[structopt(long = "status")]
        status: Option<Status>,
    },
}

async fn async_main() -> i32 {
    let args = Args::from_args();
    match args {
        Args::ID => {
            let id = id::ID::generate();
            println!("{}", id);
            0
        }
        Args::Now => {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("System time before UNIX EPOCH");
            println!("{}", now.as_secs());
            0
        }
        Args::Cmd {
            build,
            step,
            name,
            cmd,
            args,
        } => {
            let pipeline = pipeline::install_pipeline();

            let span_name = if let Some(name) = name.clone() {
                format!("cmd - {}", name)
            } else {
                format!("cmd - {} {}", cmd, args.join(" "))
            };
            let span = pipeline
                .tracer
                .span_builder(&span_name)
                .with_parent_context(context::get_parent_context(build, step))
                .with_kind(SpanKind::Client)
                .with_attributes(vec![
                    Key::new("tracebuild.cmd.command").string(cmd.clone()),
                    Key::new("tracebuild.cmd.arguments").array(
                        args.iter()
                            .map(|arg| Cow::from(arg.clone()))
                            .collect::<Vec<_>>(),
                    ),
                ])
                .start(&pipeline.tracer);
            let cx = Context::current_with_span(span);
            let start_time = SystemTime::now();
            let exit_code = match cmd::fork_with_sigterm(cmd.clone(), args)
                .with_context(cx.clone())
                .await
            {
                Ok(exit_status) => {
                    let exit_code = exit_status.code().unwrap_or(1);
                    cx.span()
                        .set_attribute(Key::new("tracebuild.cmd.exit_code").i64(exit_code.into()));
                    exit_code
                }
                Err(err) => {
                    eprintln!("{}", err);
                    cx.span().record_exception(&err);
                    cx.span().set_status(StatusCode::Error, err.to_string());
                    err.suggested_exit_code()
                }
            };

            let mut labels = Vec::new();
            labels.push(Key::new("name").string(name.unwrap_or(cmd)));
            labels.push(Key::new("exit_code").i64(exit_code.into()));
            record_event_count(&pipeline.meter, "tracebuild.cmd.count", &labels);
            record_event_duration(
                &pipeline.meter,
                "tracebuild.cmd.duration",
                start_time,
                &labels,
            );
            exit_code
        }
        Args::Step {
            build,
            step,
            id,
            start_time,
            name,
            status,
        } => {
            let pipeline = pipeline::install_pipeline();

            let span_name: Cow<'static, str> = if let Some(name) = name.clone() {
                format!("step - {}", name).into()
            } else {
                "step".into()
            };
            let span = pipeline
                .tracer
                .span_builder(&span_name)
                .with_parent_context(context::get_parent_context(build, step))
                .with_start_time(start_time)
                .with_span_id(id)
                .with_kind(SpanKind::Internal)
                .start(&pipeline.tracer);
            if let Some(status) = &status {
                span.set_status(status.into(), "".into());
            }

            let mut labels = Vec::new();
            if let Some(name) = name {
                labels.push(Key::new("name").string(name));
            }
            if let Some(status) = status {
                labels.push(Key::new("status").string(status.to_string()));
            }
            record_event_count(&pipeline.meter, "tracebuild.step.count", &labels);
            record_event_duration(
                &pipeline.meter,
                "tracebuild.step.duration",
                start_time,
                &labels,
            );
            0
        }
        Args::Build {
            id,
            start_time,
            name,
            branch,
            commit,
            status,
        } => {
            let pipeline = pipeline::install_pipeline();

            let span_name: Cow<'static, str> = if let Some(name) = name.clone() {
                format!("build - {}", name).into()
            } else {
                "build".into()
            };
            let span = pipeline
                .tracer
                .span_builder(&span_name)
                .with_start_time(start_time)
                .with_trace_id(id.0)
                .with_span_id(id.1)
                .with_kind(SpanKind::Internal)
                .start(&pipeline.tracer);
            if let Some(branch) = branch.clone() {
                span.set_attribute(Key::new("tracebuild.build.branch").string(branch));
            }
            if let Some(commit) = commit {
                span.set_attribute(Key::new("tracebuild.build.commit").string(commit));
            }
            if let Some(status) = &status {
                span.set_status(status.into(), "".into());
            }

            let mut labels = Vec::new();
            if let Some(name) = name {
                labels.push(Key::new("name").string(name));
            }
            if let Some(branch) = branch {
                labels.push(Key::new("branch").string(branch));
            }
            if let Some(status) = status {
                labels.push(Key::new("status").string(status.to_string()));
            }
            record_event_count(&pipeline.meter, "tracebuild.build.count", &labels);
            record_event_duration(
                &pipeline.meter,
                "tracebuild.build.duration",
                start_time,
                &labels,
            );
            0
        }
    }
}

fn main() {
    let exit_code = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async_main());
    std::process::exit(exit_code);
}
