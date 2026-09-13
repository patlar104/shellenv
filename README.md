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

## License

MIT. See [LICENSE](LICENSE).
