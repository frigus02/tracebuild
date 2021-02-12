use std::{io, process::ExitStatus};
use thiserror::Error;
use tokio::process::{Child, Command};

#[derive(Debug, Error)]
pub enum ForkError {
    #[error("Failed to fork child program: {err}")]
    FailedToFork {
        err: io::Error,
        suggested_exit_code: i32,
    },
    #[cfg(unix)]
    #[error("Failed to register SIGTERM handler: {err}")]
    FailedToRegisterSignalHandler {
        err: io::Error,
        suggested_exit_code: i32,
    },
    #[error("Child program failed: {0}")]
    IoError(#[from] io::Error),
    #[error("Child was killed")]
    Killed,
}

// From https://man.netbsd.org/sysexits.3
const EX_OSERR: i32 = 71;

struct TermSignal {
    #[cfg(unix)]
    signal: tokio::signal::unix::Signal,
}

#[cfg(unix)]
impl TermSignal {
    fn new() -> Result<Self, ForkError> {
        use tokio::signal::unix::{signal, SignalKind};

        let signal = signal(SignalKind::terminate()).map_err(|err| {
            ForkError::FailedToRegisterSignalHandler {
                err,
                suggested_exit_code: EX_OSERR,
            }
        })?;
        Ok(Self { signal })
    }

    async fn recv(&mut self) {
        self.signal.recv().await
    }
}

#[cfg(not(unix))]
impl TermSignal {
    #[allow(clippy::unnecessary_wraps)]
    fn new() -> Result<Self, ForkError> {
        Ok(Self {})
    }

    async fn recv(&mut self) {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl-c")
    }
}

#[cfg(unix)]
async fn terminate_child(child: Child) -> Result<ExitStatus, ForkError> {
    use nix::{
        sys::signal::{kill, Signal::SIGTERM},
        unistd::Pid,
    };
    use std::convert::TryInto as _;

    if let Some(pid) = child.id() {
        // If the child hasn't already completed, send a SIGTERM.
        if let Err(e) = kill(Pid::from_raw(pid.try_into().expect("Invalid PID")), SIGTERM) {
            eprintln!("Failed to forward SIGTERM to child process: {}", e);
        }
    }
    // Wait to get the child's exit code.
    child.wait().await.map_err(Into::into)
}

#[cfg(not(unix))]
async fn terminate_child(mut child: Child) -> Result<ExitStatus, ForkError> {
    child.kill().await?;
    Err(ForkError::Killed)
}

pub async fn fork_with_sigterm(cmd: String, args: Vec<String>) -> Result<ExitStatus, ForkError> {
    let mut child = match Command::new(&cmd).args(args).spawn() {
        Ok(child) => child,
        Err(err) => {
            return Err(ForkError::FailedToFork {
                err,
                suggested_exit_code: EX_OSERR,
            })
        }
    };

    let mut sigterm = TermSignal::new()?;

    tokio::select! {
        ex = child.wait() => ex.map_err(Into::into),
        _ = sigterm.recv() => terminate_child(child).await
    }
}
