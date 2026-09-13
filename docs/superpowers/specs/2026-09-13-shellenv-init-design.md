# Design: `shellenv init` shell adapters

**Date:** 2026-09-13  
**Status:** Draft for review  
**Approach:** Per-shell modules + `ShellKind` (Approach 1)

## Goal

Add `shellenv init <shell>` so interactive shells can load thin helper functions that delegate all profile, environment, and execution logic to `shellenv run`.

This bridges “call shellenv explicitly” and “shellenv is part of normal shell workflow” without duplicating core behavior in shell scripts.

## Non-goals (v1)

- Automatic environment export at shell startup
- `shellenv env`
- Python interpreter discovery
- Agent JSON protocol (`run-json`)
- Tagging / releasing `v0.1.0`
- `cmd` interactive init
- `insta` snapshot tests of full generated scripts
- Zsh `emulate -L zsh`
- Shared cross-shell quoting abstraction

## Architecture

```text
Interactive shell / Agent / Script
             │
             ▼
      Thin shell adapter
   shellenv_run / shellenv_exec_agent
             │
             ▼
        shellenv binary
             │
             ▼
        shellenv run
             │
             ▼
 Configuration + profile resolution
 Environment/tool/PATH resolution
 Execution policy and timeout handling
             │
             ▼
          Command
```

Init flow:

```text
shellenv init <shell>
        │
        ▼
   CLI (src/main.rs)
   parse shell → ShellKind (FromStr, case-insensitive)
   resolve binary: current_exe() → Option<PathBuf>
        │
        ▼
   shellenv_core::init::generate_init(shell, binary: Option<&Path>)
        │
        ├── bash::init_code(binary)
        ├── zsh::init_code(binary)
        ├── fish::init_code(binary)
        └── pwsh::init_code(binary)
        │
        ▼
   print generated helpers to stdout
```

Rules:

- The init command does **not** load configuration.
- Generated helpers only translate native args/env into `shellenv run`.
- No startup env mutation in v1.
- Bash, Zsh, Fish, and PowerShell adapters stay thin; behavior stays in the binary.

## CLI

### Command shape

```text
shellenv init bash
shellenv init zsh
shellenv init fish
shellenv init pwsh
```

### Types

```rust
#[derive(Args, Debug)]
pub struct InitArgs {
    /// Shell to generate init code for: bash, zsh, fish, pwsh
    #[arg()]
    pub shell: String,
}

enum Commands {
    Config(ConfigArgs),
    Run(RunArgs),
    Init(InitArgs),
}
```

### Parsing

- Accept `bash`, `zsh`, `fish`, `pwsh` **case-insensitively** via `ShellKind: FromStr` (same contract style as `run --shell`).
- Reject everything else, including `cmd` / `ksh`, with:
  - stderr: `unsupported shell: …`
  - stderr: supported list `bash, zsh, fish, pwsh`
  - exit code 1
- On success: print generated code to stdout, exit 0.

Preferred parsing shape:

```rust
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
```

### Binary path resolution

1. Call `std::env::current_exe()`.
2. On success, pass `Some(&Path)` into the generator.
3. On failure, pass `None`; backends fall back to the bare name `shellenv` with shell-specific executable resolution (see Invocation).

**Do not** convert the path to a Rust `String` in the CLI with `to_string_lossy()` before generation. Keep `Path`/`PathBuf` until each shell backend renders invocation + quoting. Lossy conversion may still occur inside a backend when emitting text; that is acceptable for v1 if unavoidable, but the API boundary stays path-typed.

The CLI must **not** pre-quote the executable for every shell. Quoting and invocation syntax are owned by each backend. Do **not** introduce a shared quoting abstraction in v1 — premature sharing across POSIX shells, Fish, and PowerShell is a common source of subtle bugs.

### Baked-path lifecycle

Generated init output may embed the absolute path of the binary that generated it.

- **Dynamic consumption** (preferred): `eval "$(shellenv init zsh)"` refreshes `current_exe()` on each shell startup.
- **Persisted consumption**: writing `shellenv init zsh > ~/.config/shellenv/init.zsh` permanently embeds that path. After moving, reinstalling, or replacing the `shellenv` executable, regenerate the persisted file.

Document this consequence in the README Shell Integration section.

## Core module layout

```text
crates/shellenv-core/src/init/mod.rs
crates/shellenv-core/src/init/bash.rs
crates/shellenv-core/src/init/zsh.rs
crates/shellenv-core/src/init/fish.rs
crates/shellenv-core/src/init/pwsh.rs
```

```rust
pub enum ShellKind {
    Bash,
    Zsh,
    Fish,
    Pwsh,
}

pub fn generate_init(shell: ShellKind, binary: Option<&Path>) -> String {
    match shell {
        ShellKind::Bash => bash::init_code(binary),
        ShellKind::Zsh => zsh::init_code(binary),
        ShellKind::Fish => fish::init_code(binary),
        ShellKind::Pwsh => pwsh::init_code(binary),
    }
}
```

`ShellKind` is intentionally separate from `exec::Shell` so interactive init does not imply `Cmd` support. Keeping a distinct type makes “interactive integration targets” a separate domain and prevents `Cmd` from leaking into APIs that cannot support it.

Export the `init` module from `shellenv-core` (`lib.rs`).

## Generated helper semantics

Every backend defines:

1. `shellenv_run`
2. `shellenv_exec_agent`

Plus a leading comment:

```text
# Generated by shellenv <version>
```

Version comes from `env!("CARGO_PKG_VERSION")` in the core crate (workspace versions are aligned at `0.1.0`).

### Shared semantics (not shared literal syntax)

| Concern | Semantics |
| --- | --- |
| Profile | If `SHELLENV_PROFILE` is set and non-empty, use it; otherwise `"default"` |
| Shell target | Emit the literal shell name in generated code (`--shell bash`, etc.). Do **not** assign it to a runtime local — it is immutable generated data |
| Timeout (`shellenv_exec_agent` only) | If `SHELLENV_TIMEOUT_MS` is set and non-empty, use it; otherwise `"30000"` |
| Delegation | Invoke the resolved binary’s `run` subcommand with `--profile`, literal `--shell <kind>`, optional `--timeout-ms`, then `--` and the user’s arguments |

**Empty-string contract:** “Empty” means a zero-length string. Unset and empty are treated identically (use the default). Whitespace-only values are considered supplied and are passed through unchanged — adapters must not trim. Validation of profile names belongs deeper in `shellenv` if added later.

Examples:

| Value | Result |
| --- | --- |
| unset `SHELLENV_PROFILE` | `"default"` |
| `SHELLENV_PROFILE=""` | `"default"` |
| `SHELLENV_PROFILE="   "` | `"   "` (passed through) |
| unset / empty `SHELLENV_TIMEOUT_MS` | `"30000"` |

Bash/Zsh `${VAR:-default}` already matches this. Fish and PowerShell must deliberately preserve the same semantics with native idioms. Do not paste Bash parameter expansion into Fish/PowerShell templates.

### Invocation (recursion / shadowing protection)

All backends must invoke the real `shellenv` **executable**, never a user-defined function or alias of the same name.

| Shell | When `binary` is `Some(path)` | When falling back (`None`) |
| --- | --- | --- |
| Bash / Zsh / Fish | `command` + shell-quoted absolute path | `command shellenv` |
| PowerShell | `&` + PowerShell-quoted absolute path | Resolve an **Application** named `shellenv` (not a function/alias), then `&` that path |

PowerShell bare `& shellenv` is **not** sufficient for the fallback: it participates in normal command resolution and may hit a function or alias. The backend must resolve an Application/executable. One valid approach (not mandatory exact text):

```powershell
$shellenvBinary = (Get-Command shellenv -CommandType Application -ErrorAction Stop).Source
& $shellenvBinary run ...
```

The required behavior is: fallback resolves an Application/executable, not a PowerShell function or alias.

### PowerShell naming

PowerShell variables are case-insensitive. **Do not** use `$profile` (collides with automatic `$PROFILE`). Locals are only for genuinely dynamic values:

- `$shellenvProfile`
- `$shellenvTimeout`
- `$shellenvArgs` — only if needed to forward unbound arguments

Do **not** introduce `$shellenvShell`; emit `pwsh` literally in `--shell pwsh`.

### Argument forwarding

- Bash/Zsh: `"$@"` after `--`
- Fish: `$argv` after `--`
- PowerShell: forward the function’s unbound arguments; use `$shellenvArgs` if capturing `$args`

### Example shapes (conceptual)

Bash (Zsh is the same shape with `--shell zsh` and no runtime shell local):

```bash
# Generated by shellenv 0.1.0

shellenv_run() {
    local profile="${SHELLENV_PROFILE:-default}"

    command '/path/to/shellenv' run \
        --profile "$profile" \
        --shell bash \
        -- "$@"
}

shellenv_exec_agent() {
    local profile="${SHELLENV_PROFILE:-default}"

    command '/path/to/shellenv' run \
        --profile "$profile" \
        --shell bash \
        --timeout-ms "${SHELLENV_TIMEOUT_MS:-30000}" \
        -- "$@"
}
```

Fish and PowerShell backends emit equivalent behavior with native syntax, path quoting, and (for PowerShell fallback) Application resolution.

## Testing

### CLI matrix (`tests/cli.rs`)

| Test | Required |
| --- | --- |
| `init bash` succeeds | Yes |
| `init zsh` succeeds | Yes |
| `init fish` succeeds | Yes |
| `init pwsh` succeeds | Yes |
| `init BASH` succeeds (case-insensitive) and emits `--shell bash` | Yes |
| unknown `ksh` fails; stderr contains `unsupported shell` | Yes |
| both helper names emitted (`shellenv_run`, `shellenv_exec_agent`) | Yes |
| generated version comment `# Generated by shellenv <version>` | Yes |
| correct **literal** shell target (`--shell bash` / `zsh` / `fish` / `pwsh`) | Yes |
| `--timeout-ms` and `30000` appear in agent helper | Yes |
| full `insta` snapshots | No |

Stdout must be non-empty on success. Literal `--shell <name>` assertions are valid because backends emit the shell name directly in the generated source (not via a runtime variable).

### Unit tests for quoting

If a backend’s path-quoting helper is nontrivial, add unit tests for paths containing spaces and quotes, for example:

- `/my tools/shellenv`
- `/tmp/foo'bar/shellenv`

Executable-path quoting is one of the few pieces of real shell-specific logic in these adapters; CLI smoke tests alone are unlikely to catch quoting bugs.

## Documentation

### README

Add a **Shell Integration** section:

- Bash: `eval "$(shellenv init bash)"` in `~/.bashrc`
- Zsh: `eval "$(shellenv init zsh)"` in `~/.zshrc`
- Fish: `shellenv init fish | source` in `~/.config/fish/config.fish`
- PowerShell: `shellenv init pwsh | Out-String | Invoke-Expression` in `$PROFILE`
- Explain `shellenv_run` and `shellenv_exec_agent`
- Show selecting a profile via `SHELLENV_PROFILE` (with shell-appropriate examples)
- Note that helpers call the generating binary when `current_exe` is available; otherwise they resolve the `shellenv` executable on `PATH` (with PowerShell Application resolution)
- Document baked-path lifecycle: if init output is saved to a file rather than generated dynamically from the shell rc, regenerate that file after moving, reinstalling, or replacing the `shellenv` executable

### `docs/commands.md`

- Add `shellenv init <shell>` to the allowed commands table
- Document supported shells (`bash`, `zsh`, `fish`, `pwsh`; not `cmd`)
- Note case-insensitive shell names
- Note that init prints sourceable helpers and does not load config
- Briefly note possible absolute-path embedding and the regenerate-after-move consequence

## Error handling

| Case | Behavior |
| --- | --- |
| Unsupported shell name | stderr message + supported list; exit 1 |
| `current_exe` failure | fall back to bare `shellenv` resolution in generated code; still succeed |
| Generation itself | pure string construction; no I/O errors expected |

## Future work (explicitly later)

1. Optional `shellenv env` + evaluate once from init
2. Python strategies → `SHELLENV_PYTHON`
3. `shellenv run-json`
4. Tag `v0.1.0` once init + run + tests + docs are stable
