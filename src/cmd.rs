use std::{io, process::ExitStatus};
use thiserror::Error;
use tokio::process::{Child, Command};

#[derive(Debug, Error)]
pub(crate) enum ForkError {
    #[error("Failed to fork child program: {0}")]
    FailedToFork(io::Error),
    #[cfg(unix)]
    #[error("Failed to register SIGTERM handler: {0}")]
    FailedToRegisterSignalHandler(io::Error),
    #[error("Child program failed: {0}")]
    IoError(#[from] io::Error),
    #[cfg(not(unix))]
    #[error("Child was killed")]
    Killed,
}

// From https://man.netbsd.org/sysexits.3
const EX_OSERR: i32 = 71;

impl ForkError {
    pub(crate) fn suggested_exit_code(&self) -> i32 {
        match self {
            ForkError::FailedToFork(_) => EX_OSERR,
            #[cfg(unix)]
            ForkError::FailedToRegisterSignalHandler(_) => EX_OSERR,
            ForkError::IoError(err) => err.raw_os_error().unwrap_or(1),
            #[cfg(not(unix))]
            ForkError::Killed => 1,
        }
    }
}

struct TermSignal {
    #[cfg(unix)]
    signal: tokio::signal::unix::Signal,
}

#[cfg(unix)]
impl TermSignal {
    fn new() -> Result<Self, ForkError> {
        use tokio::signal::unix::{signal, SignalKind};

        let signal =
            signal(SignalKind::terminate()).map_err(ForkError::FailedToRegisterSignalHandler)?;
        Ok(Self { signal })
    }

    async fn recv(&mut self) {
        self.signal.recv().await;
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
async fn terminate_child(mut child: Child) -> Result<ExitStatus, ForkError> {
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

pub(crate) async fn fork_with_sigterm(
    cmd: String,
    args: Vec<String>,
) -> Result<ExitStatus, ForkError> {
    let mut child = Command::new(&cmd)
        .args(args)
        .spawn()
        .map_err(ForkError::FailedToFork)?;

    let mut sigterm = TermSignal::new()?;

    tokio::select! {
        ex = child.wait() => ex.map_err(Into::into),
        _ = sigterm.recv() => terminate_child(child).await
    }
}
