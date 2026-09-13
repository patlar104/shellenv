# Shellenv Init Adapters Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `shellenv init <shell>` that prints thin, shell-native helpers (`shellenv_run` / `shellenv_exec_agent`) which delegate to `shellenv run`.

**Architecture:** New `shellenv_core::init` module with `ShellKind` and per-shell backends (`bash`/`zsh`/`fish`/`pwsh`). CLI resolves `current_exe()` as `Option<PathBuf>`, passes `Option<&Path>` into `generate_init`, and prints the result. No config load; no startup env export.

**Tech Stack:** Rust, clap, assert_cmd / predicates (existing CLI tests), thiserror (match existing core error style).

**Spec:** `docs/superpowers/specs/2026-09-13-shellenv-init-design.md`

## Global Constraints

- Init does **not** load configuration.
- Helpers only wrap `shellenv run`; no PATH/env/profile logic in shell code.
- Shell target is a **literal** in generated code (`--shell bash`, etc.), not a runtime local.
- Empty env contract: unset and `""` → default; whitespace-only values pass through unchanged.
- Binary path stays `Option<&Path>` until each backend renders quoting; no CLI `to_string_lossy()`.
- No shared quoting across Fish/PowerShell and POSIX. Bash and Zsh may share a tiny POSIX single-quote helper (`init/posix_quote.rs` or similar); Fish and PowerShell keep their own quoters.
- Bash/Zsh/Fish: `command` for executable invocation.
- PowerShell: `&` absolute path when available; on fallback, resolve Application named `shellenv` (not function/alias).
- Never use PowerShell `$profile`; only `$shellenvProfile`, `$shellenvTimeout`, `$shellenvArgs` (if needed).
- Case-insensitive shell names via `FromStr for ShellKind`.
- Supported init shells: `bash`, `zsh`, `fish`, `pwsh` only (not `cmd`).
- Version comment uses `env!("CARGO_PKG_VERSION")` from `shellenv-core`.
- Verify with `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `cargo test --workspace --locked`.

## File structure

| File | Responsibility |
| --- | --- |
| `crates/shellenv-core/src/init/mod.rs` | `ShellKind`, `UnsupportedShell`, `FromStr`, `generate_init`, re-exports |
| `crates/shellenv-core/src/init/posix_quote.rs` | Shared Bash/Zsh single-quote helper only |
| `crates/shellenv-core/src/init/bash.rs` | Bash `init_code` (uses `posix_quote`) |
| `crates/shellenv-core/src/init/zsh.rs` | Zsh `init_code` (uses `posix_quote`) |
| `crates/shellenv-core/src/init/fish.rs` | Fish quoting + `init_code` |
| `crates/shellenv-core/src/init/pwsh.rs` | PowerShell quoting + Application fallback + `init_code` |
| `crates/shellenv-core/src/lib.rs` | `pub mod init;` |
| `src/main.rs` | `InitArgs`, `Commands::Init`, `cmd_init` |
| `tests/cli.rs` | CLI contract tests for init |
| `README.md` | Shell Integration section |
| `docs/commands.md` | Document `init` command |

---

### Task 1: `ShellKind` parsing + `generate_init` dispatcher

**Files:**
- Create: `crates/shellenv-core/src/init/mod.rs`
- Create: `crates/shellenv-core/src/init/bash.rs` (stub `init_code` returning empty or placeholder)
- Create: `crates/shellenv-core/src/init/zsh.rs` (stub)
- Create: `crates/shellenv-core/src/init/fish.rs` (stub)
- Create: `crates/shellenv-core/src/init/pwsh.rs` (stub)
- Modify: `crates/shellenv-core/src/lib.rs`

**Interfaces:**
- Consumes: nothing from later tasks
- Produces: `ShellKind`, `UnsupportedShell`, `impl FromStr for ShellKind`, `generate_init(shell: ShellKind, binary: Option<&Path>) -> String`, stub `*_::init_code(binary: Option<&Path>) -> String`

- [ ] **Step 1: Write failing unit tests for `ShellKind` parsing**

Add to `crates/shellenv-core/src/init/mod.rs` (or write the module with tests first that won't compile until types exist — prefer writing tests in the same file after scaffolding; for TDD, add tests that fail to compile / fail assertions):

```rust
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p shellenv-core --locked init::tests -- --nocapture`

Expected: compile failure (`init` module missing) or test failure.

- [ ] **Step 3: Implement module scaffolding**

`crates/shellenv-core/src/lib.rs` — add:

```rust
pub mod init;
```

`crates/shellenv-core/src/init/mod.rs`:

```rust
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
```

Each stub file (`bash.rs`, `zsh.rs`, `fish.rs`, `pwsh.rs`):

```rust
use std::path::Path;

pub fn init_code(_binary: Option<&Path>) -> String {
    String::new()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p shellenv-core --locked init::tests -- --nocapture`

Expected: PASS for the three parsing tests.

- [ ] **Step 5: Commit**

```bash
git add crates/shellenv-core/src/lib.rs crates/shellenv-core/src/init/
git commit -m "$(cat <<'EOF'
feat(core): add ShellKind and init module scaffolding

Introduce case-insensitive init shell parsing separate from exec::Shell.
EOF
)"
```

---

### Task 2: Bash init backend

**Files:**
- Modify: `crates/shellenv-core/src/init/bash.rs`
- Test: unit tests in `bash.rs`

**Interfaces:**
- Consumes: `generate_init` / `ShellKind` from Task 1
- Produces: `bash::init_code(binary: Option<&Path>) -> String` emitting literal `--shell bash`, helpers, version comment, `command`-based invocation

- [ ] **Step 1: Write failing quoting + content tests**

In `bash.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn quotes_paths_with_spaces_and_single_quotes() {
        assert_eq!(quote_executable(Path::new("/my tools/shellenv")), "'/my tools/shellenv'");
        assert_eq!(
            quote_executable(Path::new("/tmp/foo'bar/shellenv")),
            "'/tmp/foo'\\''bar/shellenv'"
        );
    }

    #[test]
    fn init_code_contains_helpers_literal_shell_and_version() {
        let code = init_code(Some(Path::new("/opt/shellenv")));
        assert!(code.contains(&format!("# Generated by shellenv {}", env!("CARGO_PKG_VERSION"))));
        assert!(code.contains("shellenv_run()"));
        assert!(code.contains("shellenv_exec_agent()"));
        assert!(code.contains("--shell bash"));
        assert!(code.contains("--timeout-ms"));
        assert!(code.contains("30000"));
        assert!(code.contains("command '/opt/shellenv'"));
        assert!(!code.contains("local shell="));
    }

    #[test]
    fn fallback_uses_command_shellenv() {
        let code = init_code(None);
        assert!(code.contains("command shellenv"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p shellenv-core --locked init::bash -- --nocapture`

Expected: FAIL (missing `quote_executable` / empty `init_code`).

- [ ] **Step 3: Implement bash backend**

```rust
use std::path::Path;

pub fn init_code(binary: Option<&Path>) -> String {
    let version = env!("CARGO_PKG_VERSION");
    let exe = match binary {
        Some(path) => format!("command {}", quote_executable(path)),
        None => "command shellenv".to_string(),
    };

    format!(
        r#"# Generated by shellenv {version}

shellenv_run() {{
    local profile="${{SHELLENV_PROFILE:-default}}"

    {exe} run \
        --profile "$profile" \
        --shell bash \
        -- "$@"
}}

shellenv_exec_agent() {{
    local profile="${{SHELLENV_PROFILE:-default}}"

    {exe} run \
        --profile "$profile" \
        --shell bash \
        --timeout-ms "${{SHELLENV_TIMEOUT_MS:-30000}}" \
        -- "$@"
}}
"#
    )
}

/// Single-quote a path for Bash `command` invocation.
fn quote_executable(path: &Path) -> String {
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p shellenv-core --locked init::bash -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/shellenv-core/src/init/bash.rs
git commit -m "$(cat <<'EOF'
feat(init): generate bash shellenv_run helpers

Emit thin wrappers that call shellenv run with a literal --shell bash.
EOF
)"
```

---

### Task 3: Zsh init backend

**Files:**
- Modify: `crates/shellenv-core/src/init/zsh.rs`

**Interfaces:**
- Consumes: same `Option<&Path>` contract as bash
- Produces: `zsh::init_code` with literal `--shell zsh` (no `emulate -L zsh`)

- [ ] **Step 1: Write failing tests**

Mirror bash tests in `zsh.rs`, asserting `--shell zsh`, `shellenv_run()`, version comment, quoting for `/my tools/shellenv` and `/tmp/foo'bar/shellenv`, and `command shellenv` fallback. Do **not** assert `emulate`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p shellenv-core --locked init::zsh -- --nocapture`

Expected: FAIL.

- [ ] **Step 3: Implement zsh backend**

Same structure as bash, with `--shell zsh`. Reuse `crate::init::posix_quote::quote_executable` (shared with bash only; do not duplicate).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p shellenv-core --locked init::zsh -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/shellenv-core/src/init/zsh.rs
git commit -m "$(cat <<'EOF'
feat(init): generate zsh shellenv_run helpers

Match bash semantics with a literal --shell zsh target.
EOF
)"
```

---

### Task 4: Fish init backend

**Files:**
- Modify: `crates/shellenv-core/src/init/fish.rs`

**Interfaces:**
- Consumes: `Option<&Path>`
- Produces: Fish `function` helpers with native empty-env semantics and `$argv`

- [ ] **Step 1: Write failing tests**

Assert:

- `# Generated by shellenv <version>`
- `function shellenv_run` / `function shellenv_exec_agent`
- `--shell fish`
- `--timeout-ms` and `30000`
- path with spaces quoted appropriately for Fish
- fallback contains `command shellenv`
- empty-profile idiom that treats empty string as default (e.g. `test -n "$SHELLENV_PROFILE"` pattern from the original design)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p shellenv-core --locked init::fish -- --nocapture`

Expected: FAIL.

- [ ] **Step 3: Implement fish backend**

```rust
use std::path::Path;

pub fn init_code(binary: Option<&Path>) -> String {
    let version = env!("CARGO_PKG_VERSION");
    let exe = match binary {
        Some(path) => format!("command {}", quote_executable(path)),
        None => "command shellenv".to_string(),
    };

    format!(
        r#"# Generated by shellenv {version}

function shellenv_run
    set -l profile (test -n "$SHELLENV_PROFILE"; and echo "$SHELLENV_PROFILE"; or echo "default")

    {exe} run \
        --profile $profile \
        --shell fish \
        -- $argv
end

function shellenv_exec_agent
    set -l profile (test -n "$SHELLENV_PROFILE"; and echo "$SHELLENV_PROFILE"; or echo "default")
    set -l timeout (test -n "$SHELLENV_TIMEOUT_MS"; and echo "$SHELLENV_TIMEOUT_MS"; or echo "30000")

    {exe} run \
        --profile $profile \
        --shell fish \
        --timeout-ms $timeout \
        -- $argv
end
"#
    )
}

fn quote_executable(path: &Path) -> String {
    // Fish single-quoted string: escape ' as \'
    let raw = path.to_string_lossy();
    let mut out = String::from("'");
    for ch in raw.chars() {
        if ch == '\'' {
            out.push_str("\\'");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}
```

Confirm Fish single-quote escaping while implementing; adjust `quote_executable` + unit tests if Fish requires a different form, but keep per-backend ownership.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p shellenv-core --locked init::fish -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/shellenv-core/src/init/fish.rs
git commit -m "$(cat <<'EOF'
feat(init): generate fish shellenv_run helpers

Use Fish-native empty-env checks and $argv forwarding.
EOF
)"
```

---

### Task 5: PowerShell init backend

**Files:**
- Modify: `crates/shellenv-core/src/init/pwsh.rs`

**Interfaces:**
- Consumes: `Option<&Path>`
- Produces: PowerShell functions with Application fallback, literal `--shell pwsh`, no `$profile`

- [ ] **Step 1: Write failing tests**

Assert:

- version comment
- `function shellenv_run` / `function shellenv_exec_agent`
- `--shell pwsh`
- `--timeout-ms` and `30000`
- `$shellenvProfile` / `$shellenvTimeout` present
- `$profile` / `$PROFILE` **not** used as locals (avoid matching automatic profile variable assignments; asserting absence of `$shellenvShell` and no `local`-style `$profile =` is enough — specifically assert code does **not** contain `$profile =` or `$PROFILE =` as assignment targets for the Shellenv profile)
- absolute path case uses `& '`…`'` (or `& "…"`) form for `/opt/shellenv` and paths with spaces / single quotes
- fallback contains `Get-Command shellenv -CommandType Application` (or equivalent Application resolution) and does **not** rely on bare `& shellenv` alone

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p shellenv-core --locked init::pwsh -- --nocapture`

Expected: FAIL.

- [ ] **Step 3: Implement pwsh backend**

Conceptual generated shape when `binary` is `Some`:

```powershell
# Generated by shellenv 0.1.0

function shellenv_run {
    $shellenvProfile = if ($null -ne $env:SHELLENV_PROFILE -and $env:SHELLENV_PROFILE -ne '') {
        $env:SHELLENV_PROFILE
    } else {
        "default"
    }

    & '/opt/shellenv' run `
        --profile $shellenvProfile `
        --shell pwsh `
        -- @args
}

function shellenv_exec_agent {
    $shellenvProfile = if ($null -ne $env:SHELLENV_PROFILE -and $env:SHELLENV_PROFILE -ne '') {
        $env:SHELLENV_PROFILE
    } else {
        "default"
    }

    $shellenvTimeout = if ($null -ne $env:SHELLENV_TIMEOUT_MS -and $env:SHELLENV_TIMEOUT_MS -ne '') {
        $env:SHELLENV_TIMEOUT_MS
    } else {
        "30000"
    }

    & '/opt/shellenv' run `
        --profile $shellenvProfile `
        --shell pwsh `
        --timeout-ms $shellenvTimeout `
        -- @args
}
```

When `binary` is `None`, replace `& '…'` with Application resolution, e.g.:

```powershell
$shellenvBinary = (Get-Command shellenv -CommandType Application -ErrorAction Stop).Source
& $shellenvBinary run `
```

Use PowerShell single-quoted escaping (double single-quotes inside `'…'`). Prefer splatting/`@args` or `$args` forwarding that works for advanced functions; if `@args` is wrong for a simple `function`, use:

```powershell
$shellenvArgs = $args
& $shellenvBinary run --profile $shellenvProfile --shell pwsh -- @shellenvArgs
```

Pick the form that correctly forwards unbound arguments in PowerShell 7; cover it in the unit/content assertions.

Empty-string contract: `$env:SHELLENV_PROFILE -ne ''` must treat empty as default; do **not** trim whitespace.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p shellenv-core --locked init::pwsh -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/shellenv-core/src/init/pwsh.rs
git commit -m "$(cat <<'EOF'
feat(init): generate PowerShell shellenv_run helpers

Invoke absolute paths with &; resolve Application on bare-name fallback.
EOF
)"
```

---

### Task 6: Wire `shellenv init` CLI

**Files:**
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `shellenv_core::init::{generate_init, ShellKind}`
- Produces: `Commands::Init(InitArgs)`, `cmd_init` → stdout code / exit 1 on unsupported shell

- [ ] **Step 1: Write failing CLI tests (unknown + one success)**

Append to `tests/cli.rs` (use existing `Command::cargo_bin("shellenv")` pattern; there is no `shellenv_cmd` helper today — create a small helper or inline):

```rust
#[test]
fn init_bash_prints_code() {
    Command::cargo_bin("shellenv")
        .unwrap()
        .args(["init", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("shellenv_run"))
        .stdout(predicate::str::contains("--shell bash"));
}

#[test]
fn init_unknown_shell_errors() {
    Command::cargo_bin("shellenv")
        .unwrap()
        .args(["init", "ksh"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported shell"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --locked --test cli init_bash_prints_code init_unknown_shell_errors -- --nocapture`

Expected: FAIL (`unrecognized subcommand` / similar).

- [ ] **Step 3: Implement CLI wiring**

Add:

```rust
#[derive(Args, Debug)]
struct InitArgs {
    /// Shell to generate init code for: bash, zsh, fish, pwsh
    #[arg()]
    shell: String,
}
```

Extend `Commands`:

```rust
Init(InitArgs),
```

In `run`:

```rust
Commands::Init(args) => cmd_init(args),
```

Implement:

```rust
fn cmd_init(args: InitArgs) -> Result<ExitCode, Box<dyn std::error::Error>> {
    use shellenv_core::init::generate_init;
    use std::str::FromStr;

    let shell = match shellenv_core::init::ShellKind::from_str(&args.shell) {
        Ok(shell) => shell,
        Err(_) => {
            eprintln!("error: unsupported shell: {}", args.shell);
            eprintln!("supported: bash, zsh, fish, pwsh");
            return Ok(ExitCode::from(1));
        }
    };

    let exe = std::env::current_exe().ok();
    let code = generate_init(shell, exe.as_deref());
    print!("{code}");
    Ok(ExitCode::SUCCESS)
}
```

Do **not** call `to_string_lossy()` in `cmd_init`.

- [ ] **Step 4: Run the two CLI tests**

Run: `cargo test --locked --test cli init_bash_prints_code init_unknown_shell_errors -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs tests/cli.rs
git commit -m "$(cat <<'EOF'
feat(cli): add shellenv init subcommand

Print generated shell helpers for a requested init shell.
EOF
)"
```

---

### Task 7: Complete CLI contract matrix

**Files:**
- Modify: `tests/cli.rs`

**Interfaces:**
- Consumes: working `shellenv init` from Task 6
- Produces: full required test matrix from the spec

- [ ] **Step 1: Add remaining CLI tests**

```rust
#[test]
fn init_zsh_prints_code() { /* success + shellenv_run + shellenv_exec_agent + version + --shell zsh + --timeout-ms + 30000 */ }

#[test]
fn init_fish_prints_code() { /* same for fish */ }

#[test]
fn init_pwsh_prints_code() { /* same for pwsh */ }

#[test]
fn init_bash_case_insensitive() {
    Command::cargo_bin("shellenv")
        .unwrap()
        .args(["init", "BASH"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--shell bash"));
}
```

Factor a helper if useful:

```rust
fn assert_init_shell(shell_arg: &str, literal_shell: &str) {
    let version = env!("CARGO_PKG_VERSION");
    Command::cargo_bin("shellenv")
        .unwrap()
        .args(["init", shell_arg])
        .assert()
        .success()
        .stdout(predicate::str::contains("shellenv_run"))
        .stdout(predicate::str::contains("shellenv_exec_agent"))
        .stdout(predicate::str::contains(format!(
            "# Generated by shellenv {version}"
        )))
        .stdout(predicate::str::contains(format!("--shell {literal_shell}")))
        .stdout(predicate::str::contains("--timeout-ms"))
        .stdout(predicate::str::contains("30000"));
}
```

Note: CLI test `CARGO_PKG_VERSION` is the **binary** crate version; core comment uses **core** crate version. Both are `0.1.0` in this workspace — keep them aligned. If they ever diverge, assert using the core version string or a shared constant; for v1 asserting `env!("CARGO_PKG_VERSION")` from the binary test crate is acceptable while versions match.

- [ ] **Step 2: Run full CLI init tests**

Run: `cargo test --locked --test cli init_ -- --nocapture`

Expected: all PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/cli.rs
git commit -m "$(cat <<'EOF'
test(cli): cover shellenv init contract matrix

Assert helpers, version comment, literal shells, and case folding.
EOF
)"
```

---

### Task 8: Documentation

**Files:**
- Modify: `README.md`
- Modify: `docs/commands.md`

**Interfaces:**
- Consumes: final CLI behavior
- Produces: user-facing Shell Integration docs + commands reference

- [ ] **Step 1: Update README**

Add a **Shell Integration** section before License with:

- eval/source examples for bash, zsh, fish, pwsh
- what `shellenv_run` / `shellenv_exec_agent` do
- `SHELLENV_PROFILE` example
- baked-path lifecycle note (dynamic `eval` vs persisted file must be regenerated after move/reinstall)
- note that helpers prefer the generating binary path, else resolve `shellenv` on `PATH` (PowerShell: Application)

- [ ] **Step 2: Update `docs/commands.md`**

- Add `shellenv init <shell>` to the allowed commands table
- New section documenting supported shells, case-insensitivity, no config load, path embedding / regenerate note

- [ ] **Step 3: Full verification**

Run:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Expected: all succeed.

- [ ] **Step 4: Commit**

```bash
git add README.md docs/commands.md
git commit -m "$(cat <<'EOF'
docs: document shellenv init shell integration

Explain how to load helpers and the baked binary-path lifecycle.
EOF
)"
```

---

## Spec coverage self-check

| Spec requirement | Task |
| --- | --- |
| `ShellKind` separate from `exec::Shell` | 1 |
| `FromStr` case-insensitive parsing | 1 |
| Per-shell modules + `generate_init` | 1–5 |
| Literal `--shell <name>` | 2–5, 7 |
| Empty / unset env defaults; no trim | 2–5 (semantics in generated code) |
| `command` for bash/zsh/fish | 2–4 |
| PowerShell Application fallback | 5 |
| No `$profile` locals | 5 |
| `Option<&Path>` until backend | 1, 6 |
| No shared quoting abstraction | 2–5 |
| Quoting unit tests for spaces/quotes | 2–5 |
| CLI `init` + exit 1 unsupported | 6–7 |
| CLI matrix including `BASH` | 7 |
| README + commands.md + path lifecycle | 8 |
| No startup env export / no `cmd` init | Global + non-goals |

## Placeholder / consistency scan

- No TBD/TODO left in tasks.
- Signatures consistent: `init_code(Option<&Path>) -> String`, `generate_init(ShellKind, Option<&Path>) -> String`.
- PowerShell argument-forwarding form (`@args` vs `$shellenvArgs`) left as an implementer choice with a correctness requirement — verify against PowerShell 7 while implementing Task 5.
