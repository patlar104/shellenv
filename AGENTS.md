# Agent notes

Rust workspace: binary crate `shellenv` at the repo root, library crate `shellenv-core` in `crates/shellenv-core`.

## Build and test

```bash
cargo build
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

`cargo build` writes `target/debug/shellenv`. It does not install the binary.

```bash
./target/debug/shellenv --help
cargo run -- config validate --path config/config.example.toml
cargo install --path .   # optional; puts `shellenv` on PATH via ~/.cargo/bin
```

## Config and execution

- Default config path: `~/.config/shellenv/config.toml` (`load(None)`).
- Example config: `config/config.example.toml`. Schema: `config/schema-v1.json`.
- User-facing command list: [docs/commands.md](docs/commands.md).
- Pipeline: config + profile → `exec::build_env` → selected shell → `exec::run_in_shell` → structured result.
- Allowed `--shell` values: `bash`, `zsh`, `fish`, `pwsh`, `cmd`. The host must provide the executable.
- `[python].strategies` and `[shells].default_unix` / `default_windows` are stored on `Config` but not used by `run` yet.

## Do not

- Invent an install method other than `cargo build` / `cargo install --path .`.
- Treat `run` as direct-process argv execution.
- Fall back to another profile when the requested name is missing.
- Join PATH with a hardcoded `:` or `/`.
