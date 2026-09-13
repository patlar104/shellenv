use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::schema::{Config, SCHEMA_VERSION};

/// Errors that can occur while locating, reading, parsing, or validating a config.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("could not determine home directory")]
    HomeDirNotFound,

    #[error("config file not found: {path}")]
    NotFound { path: PathBuf },

    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to parse {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("unsupported config version `{found}`; expected `{expected}`")]
    UnsupportedVersion {
        found: String,
        expected: &'static str,
    },
}

/// Default config location: `~/.config/shellenv/config.toml`.
pub fn default_path() -> Result<PathBuf, ConfigError> {
    let home = dirs::home_dir().ok_or(ConfigError::HomeDirNotFound)?;
    Ok(home.join(".config").join("shellenv").join("config.toml"))
}

/// Load and validate a config from `path`, or from [`default_path`] when `path` is `None`.
pub fn load(path: Option<&Path>) -> Result<Config, ConfigError> {
    match path {
        Some(path) => load_from_path(path),
        None => load_from_path(default_path()?),
    }
}

/// Load and validate a config from an explicit file path.
pub fn load_from_path(path: impl AsRef<Path>) -> Result<Config, ConfigError> {
    let path = path.as_ref();
    let contents = fs::read_to_string(path).map_err(|source| match source.kind() {
        io::ErrorKind::NotFound => ConfigError::NotFound {
            path: path.to_path_buf(),
        },
        _ => ConfigError::Read {
            path: path.to_path_buf(),
            source,
        },
    })?;

    let config: Config = toml::from_str(&contents).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })?;

    validate(&config)?;
    Ok(config)
}

/// Check semantic constraints on an already-parsed config.
pub fn validate(config: &Config) -> Result<(), ConfigError> {
    if config.version != SCHEMA_VERSION {
        return Err(ConfigError::UnsupportedVersion {
            found: config.version.clone(),
            expected: SCHEMA_VERSION,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_config(dir: &tempfile::TempDir, contents: &str) -> PathBuf {
        let path = dir.path().join("config.toml");
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn default_path_is_under_dot_config_shellenv() {
        let path = default_path().unwrap();
        assert_eq!(path.file_name().unwrap(), "config.toml");
        assert_eq!(path.parent().unwrap().file_name().unwrap(), "shellenv");
        assert_eq!(
            path.parent()
                .unwrap()
                .parent()
                .unwrap()
                .file_name()
                .unwrap(),
            ".config"
        );
    }

    #[test]
    fn load_valid_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(
            &dir,
            r#"
version = "1"

[paths]
prepend = ["~/bin"]
append = ["/usr/local/bin"]

[env]
FOO = "bar"

[python]
strategies = ["uv", "system"]

[profiles.dev]
description = "development"
env = { DEBUG = "1" }

[shells]
default_unix = "zsh"
default_windows = "pwsh"
"#,
        );

        let config = load(Some(&path)).unwrap();
        assert_eq!(config.version, "1");
        assert_eq!(config.paths.prepend, vec!["~/bin"]);
        assert_eq!(config.paths.append, vec!["/usr/local/bin"]);
        assert_eq!(config.env.get("FOO"), Some(&"bar".to_string()));
        assert_eq!(config.python.strategies, vec!["uv", "system"]);
        assert_eq!(config.profiles["dev"].description, "development");
        assert_eq!(config.shells.default_unix, "zsh");
    }

    #[test]
    fn load_missing_file() {
        let path = PathBuf::from("/no/such/shellenv-config.toml");
        let err = load(Some(&path)).unwrap_err();
        assert!(matches!(err, ConfigError::NotFound { .. }));
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn load_invalid_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(&dir, "version = ");
        let err = load(Some(&path)).unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
        assert!(err.to_string().contains("failed to parse"));
    }

    #[test]
    fn load_missing_required_field() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(&dir, "[paths]\n");
        let err = load(Some(&path)).unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
    }

    #[test]
    fn load_unsupported_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(
            &dir,
            r#"
version = "2"
[paths]
"#,
        );
        let err = load(Some(&path)).unwrap_err();
        match err {
            ConfigError::UnsupportedVersion { found, expected } => {
                assert_eq!(found, "2");
                assert_eq!(expected, "1");
            }
            other => panic!("expected UnsupportedVersion, got {other:?}"),
        }
    }
}
