//! Failures and the exit status each one maps to.
//!
//! The statuses follow `jud` for `run`, `check` and `eval` (0 done, 1 a
//! call or a recording failed, 2 wrong before any call, 3 a bar not met)
//! and the `judge-gate` block for `gate` (0 pass, 1 blocked, 2 gate error,
//! 3 warning).

use std::process::ExitCode;

/// Exit statuses, as the gate and `jud` define them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// Done; or pass.
    Ok = 0,
    /// A call, a collector or a recording failed; or the gate blocked.
    Failed = 1,
    /// Wrong before anything was asked: configuration, rubric, manifest,
    /// an invalid verdict file.
    Usage = 2,
    /// A bar not met (`eval`); a warning (`gate`).
    Warn = 3,
}

impl From<Exit> for ExitCode {
    fn from(exit: Exit) -> Self {
        ExitCode::from(exit as u8)
    }
}

/// A failure with the status it exits with. The message goes to stderr as
/// `ci-judge: MESSAGE`, stdout stays for results.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct Failure {
    pub exit: Exit,
    pub message: String,
}

impl Failure {
    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            exit: Exit::Usage,
            message: message.into(),
        }
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            exit: Exit::Failed,
            message: message.into(),
        }
    }

    pub fn warn(message: impl Into<String>) -> Self {
        Self {
            exit: Exit::Warn,
            message: message.into(),
        }
    }
}

impl From<anyhow::Error> for Failure {
    /// Anything unclassified is a usage error: it happened before, or
    /// instead of, a call.
    fn from(error: anyhow::Error) -> Self {
        match error.downcast::<Failure>() {
            Ok(failure) => failure,
            Err(error) => Self::usage(format!("{error:#}")),
        }
    }
}

impl From<std::io::Error> for Failure {
    fn from(error: std::io::Error) -> Self {
        Self::usage(error.to_string())
    }
}

impl From<judgment::jud::Error> for Failure {
    fn from(error: judgment::jud::Error) -> Self {
        Self::usage(format!("jud document: {error}"))
    }
}

pub type Result<T> = std::result::Result<T, Failure>;
