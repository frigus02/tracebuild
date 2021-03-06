//! A small binary to instrument builds in systems like GitHub Actions, Travis CI, etc. It uses
//! [OpenTelemetry](https://opentelemetry.io/) under the hood, which means you should be able to
//! integrate it in your existing telemetry platform.
#![deny(missing_docs, unreachable_pub, missing_debug_implementations)]

mod cmd;
mod context;
mod id;
mod pipeline;
mod status;
mod timestamp;

use id::{BuildId, StepId};
use opentelemetry::{
    metrics::Meter,
    trace::{FutureExt, Span, SpanKind, StatusCode, TraceContextExt, Tracer},
    Context, Key, KeyValue, Unit,
};
use status::Status;
use std::borrow::Cow;
use structopt::StructOpt;
use timestamp::Timestamp;

fn record_event_duration(meter: &Meter, name: &str, start_time: Timestamp, labels: &[KeyValue]) {
    let duration = start_time.system_time().elapsed().unwrap_or_default();
    match meter
        .f64_value_recorder(name)
        .with_unit(Unit::new("seconds"))
        .try_init()
    {
        Ok(value_recorder) => value_recorder.record(duration.as_secs_f64(), labels),
        Err(err) => eprintln!("Failed to record duration {}: {}", name, err),
    }
}

#[derive(StructOpt)]
enum Args {
    /// Generates an ID, which can be used as either a span or build id.
    Id,
    /// Generates timestamp, which can be used as a build or span start time.
    Now,
    /// Executes the specified command and reports a span using the configured OpenTelemetry
    /// exporter.
    Cmd {
        /// Build ID
        #[structopt(long = "build", env = "TRACEBUILD_BUILD_ID")]
        build: BuildId,
        /// Optional parent step ID
        #[structopt(long = "step", env = "TRACEBUILD_STEP_ID")]
        step: Option<StepId>,
        /// Optional name. Falls back to cmd + args. Included in metrics, so should be low cardinality if metrics are enabled.
        #[structopt(long = "name")]
        name: Option<String>,
        /// Optional build name
        #[structopt(long = "build-name", env = "TRACEBUILD_BUILD_NAME")]
        build_name: Option<String>,
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
        #[structopt(long = "build", env = "TRACEBUILD_BUILD_ID")]
        build: BuildId,
        /// Optional parent step ID
        #[structopt(long = "step")]
        step: Option<StepId>,
        /// Step ID
        #[structopt(long = "id", env = "TRACEBUILD_STEP_ID")]
        id: StepId,
        /// Start time
        #[structopt(long = "start-time", env = "TRACEBUILD_STEP_START")]
        start_time: Timestamp,
        /// Optional name
        #[structopt(long = "name")]
        name: Option<String>,
        /// Optional build name
        #[structopt(long = "build-name", env = "TRACEBUILD_BUILD_NAME")]
        build_name: Option<String>,
        /// Optional status
        #[structopt(long = "status")]
        status: Option<Status>,
    },
    /// Reports a span using the configured OpenTelemetry exporter with the given ID and metadata.
    Build {
        /// Build ID
        #[structopt(long = "id", env = "TRACEBUILD_BUILD_ID")]
        id: BuildId,
        /// Start time
        #[structopt(long = "start-time", env = "TRACEBUILD_BUILD_START")]
        start_time: Timestamp,
        /// Optional name
        #[structopt(long = "name", env = "TRACEBUILD_BUILD_NAME")]
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

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    pipeline::install_pipeline();
    let tracer = pipeline::tracer();
    let meter = pipeline::meter();

    let args = Args::from_args();
    let exit_code = match args {
        Args::Id => {
            let id = BuildId::generate();
            println!("{}", id);
            0
        }
        Args::Now => {
            let now = Timestamp::now();
            println!("{}", now);
            0
        }
        Args::Cmd {
            build,
            step,
            name,
            build_name,
            cmd,
            args,
        } => {
            let name = name.unwrap_or_else(|| format!("{} {}", cmd, args.join(" ")));
            let span = tracer
                .span_builder(&format!("cmd - {}", name))
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
                .start(&tracer);
            let cx = Context::current_with_span(span);
            let start_time = Timestamp::now();
            let exit_code = match cmd::fork_with_sigterm(cmd, args)
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

            let mut labels = vec![Key::new("name").string(name)];
            if let Some(build_name) = build_name {
                labels.push(Key::new("build_name").string(build_name));
            }
            labels.push(Key::new("exit_code").i64(exit_code.into()));
            record_event_duration(&meter, "tracebuild.cmd.duration", start_time, &labels);
            exit_code
        }
        Args::Step {
            build,
            step,
            id,
            start_time,
            name,
            build_name,
            status,
        } => {
            let span_name: Cow<'static, str> = if let Some(name) = name.clone() {
                format!("step - {}", name).into()
            } else {
                "step".into()
            };
            let span = tracer
                .span_builder(&span_name)
                .with_parent_context(context::get_parent_context(build, step))
                .with_start_time(start_time.system_time())
                .with_span_id(id.span_id())
                .with_kind(SpanKind::Internal)
                .start(&tracer);
            if let Some(status) = &status {
                span.set_status(status.into(), "".into());
            }

            let mut labels = Vec::new();
            if let Some(name) = name {
                labels.push(Key::new("name").string(name));
            }
            if let Some(build_name) = build_name {
                labels.push(Key::new("build_name").string(build_name));
            }
            if let Some(status) = status {
                labels.push(Key::new("status").string(status.to_string()));
            }
            record_event_duration(&meter, "tracebuild.step.duration", start_time, &labels);
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
            let span_name: Cow<'static, str> = if let Some(name) = name.clone() {
                format!("build - {}", name).into()
            } else {
                "build".into()
            };
            let span = tracer
                .span_builder(&span_name)
                .with_start_time(start_time.system_time())
                .with_trace_id(id.trace_id())
                .with_span_id(id.span_id())
                .with_kind(SpanKind::Internal)
                .start(&tracer);
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
            record_event_duration(&meter, "tracebuild.build.duration", start_time, &labels);
            0
        }
    };

    pipeline::shutdown_pipeline();
    std::process::exit(exit_code);
}
