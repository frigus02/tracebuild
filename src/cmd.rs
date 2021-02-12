use nix::{
    sys::signal::{kill, Signal::SIGTERM},
    unistd::Pid,
};
use std::{convert::TryInto as _, io, process::ExitStatus};
use thiserror::Error;
use tokio::{
    process::Command,
    signal::unix::{signal, SignalKind},
};

#[derive(Debug, Error)]
pub enum ForkError {
    #[error("Failed to fork child program {cmd}: {err}")]
    FailedToFork {
        cmd: String,
        err: io::Error,
        suggested_exit_code: i32,
    },
    #[error("Failed to register SIGTERM handler: {err}")]
    FailedToRegisterSignalHandler {
        err: io::Error,
        suggested_exit_code: i32,
    },
    #[error("Child program failed: {0}")]
    IoError(#[from] io::Error),
}

// From https://man.netbsd.org/sysexits.3
const EX_OSERR: i32 = 71;

pub async fn fork_with_sigterm(cmd: String, args: Vec<String>) -> Result<ExitStatus, ForkError> {
    let mut child = match Command::new(&cmd).args(args).spawn() {
        Ok(child) => child,
        Err(err) => {
            return Err(ForkError::FailedToFork {
                cmd,
                err,
                suggested_exit_code: EX_OSERR,
            })
        }
    };

    let mut sigterm = match signal(SignalKind::terminate()) {
        Ok(sigterm) => sigterm,
        Err(err) => {
            return Err(ForkError::FailedToRegisterSignalHandler {
                err,
                suggested_exit_code: EX_OSERR,
            });
        }
    };

    tokio::select! {
        ex = child.wait() => ex.map_err(Into::into),
        _ = sigterm.recv() => {
            if let Some(pid) = child.id() {
                // If the child hasn't already completed, send a SIGTERM.
                if let Err(e) = kill(Pid::from_raw(pid.try_into().expect("Invalid PID")), SIGTERM) {
                    eprintln!("Failed to forward SIGTERM to child process: {}", e);
                }
            }
            // Wait to get the child's exit code.
            child.wait().await.map_err(Into::into)
        }
    }
}
