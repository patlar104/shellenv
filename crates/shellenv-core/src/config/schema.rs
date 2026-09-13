use schemars::{JsonSchema, schema::RootSchema, schema_for};
use serde::Deserialize;
use std::collections::HashMap;

/// Current config schema version understood by this crate.
pub const SCHEMA_VERSION: &str = "1";

/// Top-level shellenv configuration (`config.toml`).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct Config {
    /// Config schema version. Must be `"1"`.
    pub version: String,
    /// PATH prepend/append directories.
    pub paths: Paths,
    /// Environment variables applied to all profiles.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Python interpreter discovery settings.
    #[serde(default)]
    pub python: PythonConfig,
    /// Named profiles with extra environment variables.
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
    /// Default shells per platform.
    #[serde(default)]
    pub shells: ShellConfig,
}

/// PATH modifications applied when a shell starts.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct Paths {
    /// Directories inserted at the front of `PATH`.
    #[serde(default)]
    pub prepend: Vec<String>,
    /// Directories appended to `PATH`.
    #[serde(default)]
    pub append: Vec<String>,
}

/// Python interpreter discovery settings.
#[derive(Debug, Deserialize, Default, JsonSchema)]
pub struct PythonConfig {
    /// Ordered lookup strategies: `uv`, `conda`, `venv`, `system`.
    #[serde(default)]
    pub strategies: Vec<String>,
}

/// A named environment profile.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct Profile {
    /// Human-readable description of the profile.
    #[serde(default)]
    pub description: String,
    /// Extra environment variables for this profile.
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// Default interactive shells.
#[derive(Debug, Deserialize, Default, JsonSchema)]
pub struct ShellConfig {
    /// Default interactive shell on Unix (`bash`, `zsh`, `fish`, ...).
    #[serde(default = "default_unix_shell")]
    pub default_unix: String,
    /// Default shell on Windows (`pwsh`, `powershell`, `cmd`).
    #[serde(default = "default_windows_shell")]
    pub default_windows: String,
}

fn default_unix_shell() -> String {
    "bash".to_string()
}
fn default_windows_shell() -> String {
    "pwsh".to_string()
}

/// JSON Schema (draft-07) for [`Config`].
pub fn json_schema() -> RootSchema {
    schema_for!(Config)
}

/// Pretty-printed JSON Schema for [`Config`].
pub fn json_schema_pretty() -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&json_schema())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_schema_pretty_is_object_with_title() {
        let json = json_schema_pretty().unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["title"], "Config");
        assert!(value.get("$schema").is_some());
        assert!(value["properties"].get("version").is_some());
        assert!(value["properties"].get("paths").is_some());
    }
}
