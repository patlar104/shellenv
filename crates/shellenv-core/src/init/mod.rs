pub(crate) mod posix_quote;

pub mod bash;
pub mod fish;
pub mod pwsh;
pub mod zsh;

use std::path::Path;
use std::str::FromStr;

/// Interactive init targets. Distinct from [`crate::exec::Shell`] (no `cmd`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    Bash,
    Zsh,
    Fish,
    Pwsh,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("unsupported shell '{found}'; supported: bash, zsh, fish, pwsh")]
pub struct UnsupportedShell {
    pub found: String,
}

impl UnsupportedShell {
    pub fn new(found: impl Into<String>) -> Self {
        Self {
            found: found.into(),
        }
    }
}

impl FromStr for ShellKind {
    type Err = UnsupportedShell;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "bash" => Ok(Self::Bash),
            "zsh" => Ok(Self::Zsh),
            "fish" => Ok(Self::Fish),
            "pwsh" => Ok(Self::Pwsh),
            _ => Err(UnsupportedShell::new(value)),
        }
    }
}

/// Generate shell-native init helpers. `binary` is `current_exe` when available.
pub fn generate_init(shell: ShellKind, binary: Option<&Path>) -> String {
    match shell {
        ShellKind::Bash => bash::init_code(binary),
        ShellKind::Zsh => zsh::init_code(binary),
        ShellKind::Fish => fish::init_code(binary),
        ShellKind::Pwsh => pwsh::init_code(binary),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn parses_supported_shells_case_insensitively() {
        assert_eq!(ShellKind::from_str("bash").unwrap(), ShellKind::Bash);
        assert_eq!(ShellKind::from_str("BASH").unwrap(), ShellKind::Bash);
        assert_eq!(ShellKind::from_str("Zsh").unwrap(), ShellKind::Zsh);
        assert_eq!(ShellKind::from_str("fish").unwrap(), ShellKind::Fish);
        assert_eq!(ShellKind::from_str("pwsh").unwrap(), ShellKind::Pwsh);
    }

    #[test]
    fn rejects_unsupported_shells() {
        let err = ShellKind::from_str("ksh").unwrap_err();
        let message = err.to_string();
        assert!(message.contains("ksh"));
        assert!(message.contains("bash"));
        assert!(message.contains("zsh"));
        assert!(message.contains("fish"));
        assert!(message.contains("pwsh"));
        assert!(!message.contains("cmd"));
    }

    #[test]
    fn rejects_cmd() {
        assert!(ShellKind::from_str("cmd").is_err());
    }
}
