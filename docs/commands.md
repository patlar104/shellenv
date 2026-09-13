# Commands

Shellenv has a small CLI. There is **no allowlist of programs** such as `echo hello`.
`shellenv run` takes a command string and the selected shell executes it.

If `shellenv` is not on your `PATH`, use `./target/debug/shellenv` after
`cargo build`, or `cargo run -- <args>`.

## Allowed CLI commands

| Command | What it does |
| --- | --- |
| `shellenv --help` | Show top-level usage |
| `shellenv --version` | Print the package version |
| `shellenv config schema` | Print the config JSON Schema |
| `shellenv config schema-version` | Print the schema version (`1`) |
| `shellenv config validate` | Load and validate a config file |
| `shellenv run -- …` | Run a command in a configured shell |

Anything else is rejected by the CLI (`unrecognized subcommand`).

## `config validate`

```bash
shellenv config validate
shellenv config validate --path ~/.config/shellenv/config.toml
shellenv config validate config/config.example.toml
```

With no path, this loads `~/.config/shellenv/config.toml`. `--path` and a
positional path are equivalent; do not pass both.

## `run`

```bash
shellenv run [options] -- <command>
```

| Flag | Default | Meaning |
| --- | --- | --- |
| `--config <file>` | `~/.config/shellenv/config.toml` | Config to load |
| `--profile <name>` | `default` | Profile whose `env` is applied |
| `--shell <name>` | `bash` | Shell that interprets the command |
| `--cwd <dir>` | current directory | Working directory for the child |
| `--timeout-ms <n>` | none | Kill the child after this many milliseconds |

The command **must** come after `--`. Tokens after `--` are joined with spaces
into one **shell command string**. Quoting is handled by the selected shell,
not by Shellenv.

Empty command (nothing after `--`) is an error.

### Allowed shells

These names are accepted by `--shell`. The executable must already be on the
host `PATH`. Shellenv does not bundle shells.

| `--shell` | Executable | Non-interactive invocation |
| --- | --- | --- |
| `bash` | `bash` | `--noprofile --norc -c <command>` |
| `zsh` | `zsh` | `-f -c <command>` |
| `fish` | `fish` | `-c <command>` |
| `pwsh` | `pwsh` | `-NoLogo -NoProfile -NonInteractive -Command <command>` |
| `cmd` | `cmd` | `/C <command>` |

Any other name is an error, for example `definitely-not-a-shell`. Names are
case-insensitive (`BASH` is `bash`).

Interactive startup files (`.bashrc`, `.zshrc`, PowerShell profiles, and so on)
are not loaded. Environment comes from the process environment plus the
Shellenv config and selected profile.

### What command strings are allowed

There is no built-in list of allowed programs. If the profile exists and the
shell can be spawned, the string is handed to that shell.

Examples (bash):

```bash
shellenv run --profile default --shell bash -- 'echo hello'
shellenv run --profile default --shell bash -- 'echo "editor=$EDITOR"'
shellenv run --profile dev --shell bash -- 'echo "debug=$DEBUG"'
shellenv run --profile default --shell bash -- 'python3 --version'
shellenv run --profile default --shell bash -- 'ls -la && pwd'
shellenv run --profile default --shell bash --cwd /tmp -- 'pwd'
shellenv run --profile default --shell bash --timeout-ms 2000 -- 'sleep 1; echo done'
```

Use the selected shell’s own syntax. Bash `&&` will not work under `pwsh`;
PowerShell `Write-Output` will not work under `bash`.

The command is **not** executed as a raw argv array. This is valid:

```bash
shellenv run --profile default --shell bash -- echo hello
```

and is the same as `'echo hello'` because the tokens are joined with spaces.

### Exit status

| Situation | Exit status |
| --- | --- |
| Child exited `0..=255` | that status |
| Child status outside `0..=255` | `1` |
| Missing profile, invalid shell, timeout, missing command, config error | `1` (Shellenv error on stderr) |

Stdout from the child is written to stdout. Stderr from the child is written
to stderr.

### Profiles

`--profile` must name a profile in the loaded config. There is no silent
fallback. The example config includes `default`, `dev`, and `prod`.
