use std::path::Path;

/// Single-quote a path for Bash/Zsh `command` invocation.
pub fn quote_executable(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let mut out = String::from("'");
    for ch in raw.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn quotes_paths_with_spaces_and_single_quotes() {
        assert_eq!(
            quote_executable(Path::new("/my tools/shellenv")),
            "'/my tools/shellenv'"
        );
        assert_eq!(
            quote_executable(Path::new("/tmp/foo'bar/shellenv")),
            "'/tmp/foo'\\''bar/shellenv'"
        );
    }
}
