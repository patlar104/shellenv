# Shellenv

Shellenv provides configuration-driven, predictable command execution across
different shells and environments. It builds a child environment from a TOML
config plus a selected profile, then runs a command non-interactively in a
shell you already have installed.

The requested shell is not bundled with Shellenv; it must exist on the host.

## Docs

- [Commands](docs/commands.md) — allowed CLI commands, shells, flags, and what `run` will execute

## Getting Started

1. Build from source:

   ```bash
   cargo build
   ```

   That writes `target/debug/shellenv`. It does **not** put `shellenv` on your
   `PATH`. Use the binary by path:

   ```bash
   ./target/debug/shellenv --help
   ```

   Or install it into `~/.cargo/bin` (usually already on `PATH`):

   ```bash
   cargo install --path .
   ```

   After install, the commands below can use `shellenv` instead of
   `./target/debug/shellenv`. `cargo run -- <args>` also works from this repo.

2. Install a config (default path: `~/.config/shellenv/config.toml`):

   ```bash
   mkdir -p ~/.config/shellenv
   cp config/config.example.toml ~/.config/shellenv/config.toml
   ```

3. Validate it:

   ```bash
   ./target/debug/shellenv config validate
   ```

4. Run a command:

   ```bash
   ./target/debug/shellenv run --profile default --shell bash -- 'echo hello'
   ```

   `echo hello` is only an example. See [Commands](docs/commands.md) for every
   allowed CLI command and what `run` will execute.

## Validate configuration

With no `--path`, `shellenv config validate` loads the default config
(`~/.config/shellenv/config.toml`). Pass a file explicitly when you want to
check something else:

```bash
shellenv config validate --path config/config.example.toml
shellenv config validate config/config.example.toml
```

Dump the JSON Schema with `shellenv config schema`. The schema version is
printed by `shellenv config schema-version`.

## Execute a command

Full flag, shell, and command-string rules: [Commands](docs/commands.md).

`shellenv run` also loads `~/.config/shellenv/config.toml` by default. Pass
`--config` to use another file. Tokens after `--` are joined into a **shell
command string** (not a direct argv array) and interpreted by the selected
shell.

```bash
shellenv run \
  --profile default \
  --shell bash \
  -- 'echo hello'
```

Optional flags: `--cwd <dir>`, `--timeout-ms <millis>`.

Child stdout and stderr are forwarded as captured. The process exit status is
the child's status when it is in `0..=255`; any other status becomes `1`.
Configuration, spawn, and timeout failures are Shellenv errors (exit `1`) and
are distinct from a command that ran and returned non-zero.

## Select another shell

The named shell must be installed and on `PATH`:

```bash
shellenv run --profile default --shell zsh -- 'echo hello'
shellenv run --profile default --shell fish -- 'echo hello'
shellenv run --profile default --shell pwsh -- 'Write-Output "hello"'
```

On Windows, `cmd` is also supported:

```bash
shellenv run --profile default --shell cmd -- 'echo hello'
```

## Shell Integration

`shellenv init <shell>` prints shell-native helper functions you can load in an
interactive session. Init does **not** load your config; helpers delegate to
`shellenv run`, which loads `~/.config/shellenv/config.toml` as usual.

Load the helpers once per shell startup:

| Shell | Where to add it | How to load |
| --- | --- | --- |
| Bash | `~/.bashrc` | `eval "$(shellenv init bash)"` |
| Zsh | `~/.zshrc` | `eval "$(shellenv init zsh)"` |
| Fish | `~/.config/fish/config.fish` | `shellenv init fish \| source` |
| PowerShell | `$PROFILE` | `shellenv init pwsh \| Out-String \| Invoke-Expression` |

Use `./target/debug/shellenv` instead of `shellenv` when the binary is not on
`PATH`.

### Helpers

- **`shellenv_run`** — runs `shellenv run` with your profile, the init shell,
  and the arguments you pass. Example: `shellenv_run echo hello`.
- **`shellenv_exec_agent`** — same, but adds `--timeout-ms` (default `30000`,
  overridable with `SHELLENV_TIMEOUT_MS`) for bounded agent-style runs.

As with `shellenv run`, tokens after `--` are joined into a shell command string;
the helpers do not execute a raw argv array.

Both helpers read **`SHELLENV_PROFILE`**. When it is unset or empty, they use
the `default` profile:

```bash
export SHELLENV_PROFILE=dev
shellenv_run echo "using dev profile"
```

```fish
set -gx SHELLENV_PROFILE dev
shellenv_run echo "using dev profile"
```

```powershell
$env:SHELLENV_PROFILE = 'dev'
shellenv_run "Write-Output 'using dev profile'"
```

### Which `shellenv` binary runs

When init is generated, the output usually embeds the absolute path of the
`shellenv` binary that ran `init` (via `current_exe`). Generated helpers invoke
that path with `command` (Bash, Zsh, Fish) or `&` (PowerShell) so a local
function or alias named `shellenv` cannot shadow the real executable.

If the path cannot be determined, helpers fall back to resolving the
`shellenv` **executable** on `PATH` (`command shellenv` on Unix shells;
PowerShell resolves an **Application** named `shellenv`, not a function or
alias).

### Baked-path lifecycle

Dynamic loading (for example `eval "$(shellenv init zsh)"` in `.zshrc`) runs
`init` on each new shell, so the embedded path tracks wherever `shellenv` lives
today.

If you **persist** init output to a file instead:

```bash
shellenv init zsh > ~/.config/shellenv/init.zsh
# then in ~/.zshrc: source ~/.config/shellenv/init.zsh
```

that file keeps the path from the moment it was generated. After you move,
reinstall, or replace the `shellenv` binary, regenerate the file (or switch
back to dynamic `eval` / `source`).

Interactive init supports `bash`, `zsh`, `fish`, and `pwsh` only (not `cmd`).
See [Commands](docs/commands.md#init) for CLI details.

## License

MIT. See [LICENSE](LICENSE).
