//! Configuration-driven, non-interactive command execution.
//!
//! Pipeline:
//!
//! ```text
//! configuration → effective environment → selected shell → structured result
//! ```
//!
//! The execution layer is independent of CLI types. Command output is decoded
//! as strict UTF-8; non-UTF-8 bytes return [`ExecError::Utf8Decode`].

pub mod env;
pub mod shell;

pub use env::build_env;
pub use shell::{ExecResult, Shell, run_in_shell};

/// Errors that can occur while building an execution environment or running a command.
#[derive(Debug, thiserror::Error)]
pub enum ExecError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("execution timed out")]
    Timeout,

    #[error("profile not found: {0}")]
    ProfileNotFound(String),

    #[error("shell executable not found: {0}")]
    ShellNotFound(String),

    #[error("unsupported shell '{found}'; supported shells: bash, zsh, fish, pwsh, cmd")]
    InvalidShell { found: String },

    #[error("invalid PATH: {0}")]
    InvalidPath(String),

    #[error("failed to expand `{value}`: {reason}")]
    Expansion { value: String, reason: String },

    #[error("no command supplied")]
    MissingCommand,

    #[error("failed to spawn {shell}: {source}")]
    SpawnFailed {
        shell: String,
        #[source]
        source: std::io::Error,
    },

    #[error("command output was not valid UTF-8")]
    Utf8Decode(#[from] std::string::FromUtf8Error),

    #[error("failed to capture command output")]
    OutputCapture,
}
