use std::collections::{BTreeMap, HashMap};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::config::Config;

use super::ExecError;

/// Build the child environment from `config` plus a strictly resolved profile.
///
/// Starts from the current process environment, then applies in order:
/// 1. `config.env` (sorted by key, values expanded once)
/// 2. profile `env` values, overriding matching keys
/// 3. `config.paths.prepend` inserted at the front of `PATH`
/// 4. `config.paths.append` appended to `PATH`
///
/// `~` / `$HOME` / `${VAR}` expansion is a single pass. Missing variables and
/// unsupported `~user` forms return [`ExecError::Expansion`].
pub fn build_env(
    config: &Config,
    profile_name: &str,
) -> Result<BTreeMap<String, String>, ExecError> {
    build_env_from(config, profile_name, std::env::vars())
}

/// Like [`build_env`], but starts from an explicit base environment.
pub(crate) fn build_env_from(
    config: &Config,
    profile_name: &str,
    base: impl IntoIterator<Item = (String, String)>,
) -> Result<BTreeMap<String, String>, ExecError> {
    let profile = config
        .profiles
        .get(profile_name)
        .ok_or_else(|| ExecError::ProfileNotFound(profile_name.to_string()))?;

    let mut env: BTreeMap<String, String> = base.into_iter().collect();
    apply_vars(&mut env, &config.env)?;
    apply_vars(&mut env, &profile.env)?;
    apply_paths(&mut env, &config.paths.prepend, &config.paths.append)?;
    Ok(env)
}

fn path_key(env: &BTreeMap<String, String>) -> String {
    env.keys()
        .find(|key| key.eq_ignore_ascii_case("PATH"))
        .cloned()
        .unwrap_or_else(|| "PATH".to_string())
}

fn apply_vars(
    dest: &mut BTreeMap<String, String>,
    vars: &HashMap<String, String>,
) -> Result<(), ExecError> {
    let mut keys: Vec<&String> = vars.keys().collect();
    keys.sort();
    for key in keys {
        let expanded = expand(&vars[key], dest)?;
        dest.insert(key.clone(), expanded);
    }
    Ok(())
}

fn apply_paths(
    dest: &mut BTreeMap<String, String>,
    prepend: &[String],
    append: &[String],
) -> Result<(), ExecError> {
    let key = path_key(dest);
    let existing = dest.get(&key).cloned().unwrap_or_default();
    let mut entries: Vec<PathBuf> = Vec::new();

    for value in prepend {
        let expanded = expand(value, dest)?;
        if !expanded.is_empty() {
            entries.push(PathBuf::from(expanded));
        }
    }
    entries.extend(std::env::split_paths(&existing));
    for value in append {
        let expanded = expand(value, dest)?;
        if !expanded.is_empty() {
            entries.push(PathBuf::from(expanded));
        }
    }

    let joined =
        std::env::join_paths(&entries).map_err(|err| ExecError::InvalidPath(err.to_string()))?;
    dest.insert(key, os_to_string(joined, "PATH")?);
    Ok(())
}

fn expand(input: &str, env: &BTreeMap<String, String>) -> Result<String, ExecError> {
    let with_home = expand_tilde(input, env)?;
    expand_vars(&with_home, env)
}

fn home_from_env(env: &BTreeMap<String, String>) -> Result<String, ExecError> {
    env.get("HOME")
        .or_else(|| env.get("USERPROFILE"))
        .cloned()
        .ok_or_else(|| ExecError::Expansion {
            value: "~".into(),
            reason: "HOME is not set".into(),
        })
}

fn expand_tilde(input: &str, env: &BTreeMap<String, String>) -> Result<String, ExecError> {
    if input == "~" {
        return home_from_env(env);
    }
    if let Some(rest) = input
        .strip_prefix("~/")
        .or_else(|| input.strip_prefix("~\\"))
    {
        let home = home_from_env(env)?;
        return Ok(Path::new(&home).join(rest).to_string_lossy().into_owned());
    }
    if input.starts_with('~') {
        return Err(ExecError::Expansion {
            value: input.to_string(),
            reason: "~user expansion is not supported; use ~/ or $HOME".into(),
        });
    }
    Ok(input.to_string())
}

fn expand_vars(input: &str, env: &BTreeMap<String, String>) -> Result<String, ExecError> {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '$' {
            out.push(chars[i]);
            i += 1;
            continue;
        }

        if i + 1 < chars.len() && chars[i + 1] == '{' {
            i += 2;
            let start = i;
            while i < chars.len() && chars[i] != '}' {
                i += 1;
            }
            if i >= chars.len() {
                return Err(ExecError::Expansion {
                    value: input.to_string(),
                    reason: "unterminated ${...} substitution".into(),
                });
            }
            let name: String = chars[start..i].iter().collect();
            i += 1;
            out.push_str(lookup_var(input, env, &name)?);
            continue;
        }

        if i + 1 < chars.len() && is_ident_start(chars[i + 1]) {
            i += 1;
            let start = i;
            i += 1;
            while i < chars.len() && is_ident_continue(chars[i]) {
                i += 1;
            }
            let name: String = chars[start..i].iter().collect();
            out.push_str(lookup_var(input, env, &name)?);
            continue;
        }

        out.push('$');
        i += 1;
    }
    Ok(out)
}

fn lookup_var<'a>(
    input: &str,
    env: &'a BTreeMap<String, String>,
    name: &str,
) -> Result<&'a str, ExecError> {
    if !is_valid_ident(name) {
        return Err(ExecError::Expansion {
            value: input.to_string(),
            reason: format!("invalid variable name `{name}`"),
        });
    }
    env.get(name)
        .map(String::as_str)
        .ok_or_else(|| ExecError::Expansion {
            value: input.to_string(),
            reason: format!("undefined variable `{name}`"),
        })
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn is_valid_ident(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if is_ident_start(c) => chars.all(is_ident_continue),
        _ => false,
    }
}

fn os_to_string(value: OsString, what: &str) -> Result<String, ExecError> {
    value
        .into_string()
        .map_err(|_| ExecError::InvalidPath(format!("{what} contains non-UTF-8 bytes")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn parse_config(toml: &str) -> Config {
        toml::from_str(toml).expect("valid test config")
    }

    fn base_env(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    fn path_entries(env: &BTreeMap<String, String>) -> Vec<PathBuf> {
        let key = path_key(env);
        std::env::split_paths(env.get(&key).expect("PATH present")).collect()
    }

    #[test]
    fn missing_profile_is_an_error() {
        let config = parse_config(
            r#"
version = "1"
[paths]
[profiles.default]
"#,
        );
        let err = build_env_from(&config, "definitely-does-not-exist", base_env(&[])).unwrap_err();
        assert!(
            matches!(err, ExecError::ProfileNotFound(name) if name == "definitely-does-not-exist")
        );
    }

    #[test]
    fn profile_env_overrides_base_env() {
        let config = parse_config(
            r#"
version = "1"
[paths]
[env]
MY_VAR = "base"
[profiles.test_exec.env]
MY_VAR = "from_profile"
"#,
        );
        let env = build_env_from(&config, "test_exec", base_env(&[("MY_VAR", "parent")])).unwrap();
        assert_eq!(env.get("MY_VAR").map(String::as_str), Some("from_profile"));
    }

    #[test]
    fn prepend_path_is_placed_before_existing_path() {
        let config = parse_config(
            r#"
version = "1"
[paths]
prepend = ["/opt/shellenv-prepend"]
append = ["/opt/shellenv-append"]
[profiles.default]
"#,
        );
        let existing = std::env::join_paths(["/usr/bin", "/bin"]).unwrap();
        let existing = existing.to_string_lossy().into_owned();
        let env = build_env_from(&config, "default", base_env(&[("PATH", &existing)])).unwrap();

        let entries = path_entries(&env);
        assert_eq!(entries[0], PathBuf::from("/opt/shellenv-prepend"));
        assert_eq!(entries[1], PathBuf::from("/usr/bin"));
        assert_eq!(entries[2], PathBuf::from("/bin"));
        assert_eq!(entries[3], PathBuf::from("/opt/shellenv-append"));
    }

    #[test]
    fn tilde_in_prepend_expands_using_home() {
        let config = parse_config(
            r#"
version = "1"
[paths]
prepend = ["~/bin"]
[profiles.default]
"#,
        );
        let existing = std::env::join_paths(["/usr/bin"]).unwrap();
        let existing = existing.to_string_lossy().into_owned();
        let env = build_env_from(
            &config,
            "default",
            base_env(&[("HOME", "/tmp/fake-home"), ("PATH", &existing)]),
        )
        .unwrap();

        let entries = path_entries(&env);
        assert_eq!(entries[0], PathBuf::from("/tmp/fake-home").join("bin"));
        assert_eq!(entries[1], PathBuf::from("/usr/bin"));
    }

    #[test]
    fn dollar_home_in_prepend_expands_once() {
        let config = parse_config(
            r#"
version = "1"
[paths]
prepend = ["$HOME/.local/bin"]
[profiles.default]
"#,
        );
        let existing = std::env::join_paths(["/usr/bin"]).unwrap();
        let existing = existing.to_string_lossy().into_owned();
        let env = build_env_from(
            &config,
            "default",
            base_env(&[("HOME", "/tmp/fake-home"), ("PATH", &existing)]),
        )
        .unwrap();

        let entries = path_entries(&env);
        assert_eq!(
            entries[0],
            PathBuf::from("/tmp/fake-home").join(".local").join("bin")
        );
    }

    #[test]
    fn undefined_variable_is_an_expansion_error() {
        let config = parse_config(
            r#"
version = "1"
[paths]
[env]
NEEDLE = "$DOES_NOT_EXIST"
[profiles.default]
"#,
        );
        let err = build_env_from(&config, "default", base_env(&[])).unwrap_err();
        assert!(matches!(err, ExecError::Expansion { .. }));
        assert!(err.to_string().contains("DOES_NOT_EXIST"));
    }

    #[test]
    fn values_are_not_expanded_recursively() {
        let config = parse_config(
            r#"
version = "1"
[paths]
[env]
MY_VAR = "$LITERAL"
[profiles.default]
"#,
        );
        let env = build_env_from(
            &config,
            "default",
            base_env(&[("LITERAL", "$HOME"), ("HOME", "/tmp/fake-home")]),
        )
        .unwrap();
        assert_eq!(env.get("MY_VAR").map(String::as_str), Some("$HOME"));
    }
}
