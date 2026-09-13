use serde::Deserialize;
use std::collections::HashMap;

/// Current config schema version understood by this crate.
pub const SCHEMA_VERSION: &str = "1";

#[derive(Debug, Deserialize)]
pub struct Config {
    pub version: String,
    pub paths: Paths,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub python: PythonConfig,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
    #[serde(default)]
    pub shells: ShellConfig,
}

#[derive(Debug, Deserialize)]
pub struct Paths {
    #[serde(default)]
    pub prepend: Vec<String>,
    #[serde(default)]
    pub append: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct PythonConfig {
    #[serde(default)]
    pub strategies: Vec<String>, // ["uv", "conda", "venv", "system"]
}

#[derive(Debug, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ShellConfig {
    #[serde(default = "default_unix_shell")]
    pub default_unix: String,
    #[serde(default = "default_windows_shell")]
    pub default_windows: String,
}

fn default_unix_shell() -> String {
    "bash".to_string()
}
fn default_windows_shell() -> String {
    "pwsh".to_string()
}
