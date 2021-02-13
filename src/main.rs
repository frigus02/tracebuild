mod cmd;
mod id;
mod trace;

use opentelemetry::{
    trace::{FutureExt, Span, SpanId, SpanKind, StatusCode, TraceContextExt, TraceId, Tracer},
    Context, Key,
};
use std::borrow::Cow;
use std::str::FromStr;
use std::time::{Duration, SystemTime};
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

#[derive(StructOpt)]
enum Args {
    ID,
    Now,
    Cmd {
        /// Build ID
        #[structopt(long = "build", parse(try_from_str = parse_build_id))]
        build: (TraceId, SpanId),
        /// Parent step ID
        #[structopt(long = "step", parse(try_from_str = parse_step_id))]
        step: Option<SpanId>,
        /// Command name
        #[structopt(name = "CMD")]
        cmd: String,
        /// Command arguments
        #[structopt(name = "ARGS")]
        args: Vec<String>,
    },
    Step {
        /// Build ID
        #[structopt(long = "build", parse(try_from_str = parse_build_id))]
        build: (TraceId, SpanId),
        /// Parent step ID
        #[structopt(long = "step", parse(try_from_str = parse_step_id))]
        step: Option<SpanId>,
        /// Step ID
        #[structopt(long = "id", parse(try_from_str = parse_step_id))]
        id: SpanId,
        /// Start time
        #[structopt(long = "start-time", parse(try_from_str = parse_system_time))]
        start_time: SystemTime,
        /// Name
        #[structopt(long = "name")]
        name: Option<String>,
        /// Status
        #[structopt(long = "status")]
        status: Option<Status>,
    },
    Build {
        /// Build ID
        #[structopt(long = "id", parse(try_from_str = parse_build_id))]
        id: (TraceId, SpanId),
        /// Start time
        #[structopt(long = "start-time", parse(try_from_str = parse_system_time))]
        start_time: SystemTime,
        /// Name
        #[structopt(long = "name")]
        name: Option<String>,
        /// Branch name
        #[structopt(long = "branch")]
        branch: Option<String>,
        /// Commit SHA
        #[structopt(long = "commit")]
        commit: Option<String>,
        /// Status
        #[structopt(long = "status")]
        status: Option<Status>,
    },
}

#[tokio::main]
async fn main() {
    let args = Args::from_args();
    match args {
        Args::ID => {
            let id = id::ID::generate();
            println!("{}", id);
        }
        Args::Now => {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("System time before UNIX EPOCH");
            println!("{}", now.as_secs());
        }
        Args::Cmd {
            build,
            step,
            cmd,
            args,
        } => {
            let (tracer, uninstall) = trace::install_pipeline();
            let span = tracer
                .span_builder("cmd")
                .with_parent_context(trace::get_parent_context(build, step))
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
            let exit_code = match cmd::fork_with_sigterm(cmd, args)
                .with_context(Context::current_with_span(span.clone()))
                .await
            {
                Ok(exit_status) => {
                    let exit_code = exit_status.code().unwrap_or(1);
                    span.set_attribute(Key::new("tracebuild.cmd.exit_code").i64(exit_code.into()));
                    exit_code
                }
                Err(err) => match err {
                    cmd::ForkError::FailedToFork {
                        err,
                        suggested_exit_code,
                    } => {
                        eprintln!("{}", err);
                        span.record_exception(&err);
                        span.set_status(StatusCode::Error, err.to_string());
                        suggested_exit_code
                    }
                    #[cfg(unix)]
                    cmd::ForkError::FailedToRegisterSignalHandler {
                        err,
                        suggested_exit_code,
                    } => {
                        eprintln!("{}", err);
                        span.record_exception(&err);
                        span.set_status(StatusCode::Error, err.to_string());
                        suggested_exit_code
                    }
                    cmd::ForkError::IoError(err) => {
                        eprintln!("{}", err);
                        span.record_exception(&err);
                        span.set_status(StatusCode::Error, err.to_string());
                        err.raw_os_error().unwrap_or(1)
                    }
                    cmd::ForkError::Killed => {
                        eprintln!("Child was killed");
                        span.set_status(StatusCode::Error, "".into());
                        1
                    }
                },
            };
            drop(span);
            drop(uninstall);
            std::process::exit(exit_code);
        }
        Args::Step {
            build,
            step,
            id,
            start_time,
            name,
            status,
        } => {
            let (tracer, _uninstall) = trace::install_pipeline();
            let span_name = if let Some(name) = name {
                format!("step - {}", name)
            } else {
                "step".into()
            };
            let span = tracer
                .span_builder(&span_name)
                .with_parent_context(trace::get_parent_context(build, step))
                .with_start_time(start_time)
                .with_span_id(id)
                .with_kind(SpanKind::Internal)
                .start(&tracer);
            if let Some(status) = status {
                span.set_status(
                    match status {
                        Status::Success => StatusCode::Ok,
                        Status::Failure => StatusCode::Error,
                    },
                    "".into(),
                );
            }
        }
        Args::Build {
            id,
            start_time,
            name,
            branch,
            commit,
            status,
        } => {
            let (tracer, _uninstall) = trace::install_pipeline();
            let span_name = if let Some(name) = name {
                format!("build - {}", name)
            } else {
                "build".into()
            };
            let span = tracer
                .span_builder(&span_name)
                .with_start_time(start_time)
                .with_trace_id(id.0)
                .with_span_id(id.1)
                .with_kind(SpanKind::Internal)
                .start(&tracer);
            if let Some(branch) = branch {
                span.set_attribute(Key::new("tracebuild.build.branch").string(branch));
            }
            if let Some(commit) = commit {
                span.set_attribute(Key::new("tracebuild.build.commit").string(commit));
            }
            if let Some(status) = status {
                span.set_status(
                    match status {
                        Status::Success => StatusCode::Ok,
                        Status::Failure => StatusCode::Error,
                    },
                    "".into(),
                );
            }
        }
    }
}
