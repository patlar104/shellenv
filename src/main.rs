use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use shellenv_core::config::load;
use shellenv_core::config::{SCHEMA_VERSION, json_schema_pretty};
use shellenv_core::exec::Shell;

#[derive(Parser, Debug)]
#[command(name = "shellenv", version, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Inspect and validate configuration
    Config(ConfigArgs),
    /// Run a command in a configured, non-interactive shell
    Run(RunArgs),
}

#[derive(Args, Debug)]
struct ConfigArgs {
    #[command(subcommand)]
    command: ConfigCommand,
}

#[derive(Subcommand, Debug)]
enum ConfigCommand {
    /// Print the JSON Schema for the config file format
    Schema,
    /// Print the current config schema version
    SchemaVersion,
    /// Load and validate a config file
    Validate {
        /// Path to the config TOML file. Defaults to ~/.config/shellenv/config.toml
        #[arg(long, short, value_name = "PATH")]
        path: Option<PathBuf>,

        /// Path to the config TOML file (same as `--path`)
        #[arg(value_name = "PATH", conflicts_with = "path")]
        file: Option<PathBuf>,
    },
}

/// Run a command through the selected shell.
///
/// Tokens after `--` are joined with spaces into a shell command string.
/// This is not direct argv execution; quoting is interpreted by the selected
/// shell, not by Shellenv.
#[derive(Args, Debug)]
struct RunArgs {
    /// Path to config.toml (defaults to ~/.config/shellenv/config.toml)
    #[arg(long)]
    config: Option<PathBuf>,

    /// Profile whose environment is applied
    #[arg(long, default_value = "default")]
    profile: String,

    /// Shell used to interpret the command string
    #[arg(long, default_value = "bash")]
    shell: String,

    /// Working directory for the child process
    #[arg(long)]
    cwd: Option<PathBuf>,

    /// Kill the child if it runs longer than this many milliseconds
    #[arg(long)]
    timeout_ms: Option<u64>,

    /// Command string tokens. Place them after `--`.
    #[arg(last = true)]
    command: Vec<String>,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode, Box<dyn std::error::Error>> {
    match cli.command {
        Commands::Config(args) => {
            cmd_config(args)?;
            Ok(ExitCode::SUCCESS)
        }
        Commands::Run(args) => cmd_run(args),
    }
}

fn cmd_config(args: ConfigArgs) -> Result<(), Box<dyn std::error::Error>> {
    match args.command {
        ConfigCommand::Schema => {
            println!("{}", json_schema_pretty()?);
            Ok(())
        }
        ConfigCommand::SchemaVersion => {
            println!("{SCHEMA_VERSION}");
            Ok(())
        }
        ConfigCommand::Validate { path, file } => {
            load::load(path.as_deref().or(file.as_deref()))?;
            println!("OK");
            Ok(())
        }
    }
}

fn cmd_run(args: RunArgs) -> Result<ExitCode, Box<dyn std::error::Error>> {
    if args.command.is_empty() {
        return Err("no command supplied; place the command after --".into());
    }

    let config = load::load(args.config.as_deref())?;
    let shell: Shell = args.shell.parse()?;
    let env = shellenv_core::exec::build_env(&config, &args.profile)?;
    let command = args.command.join(" ");
    let result = shellenv_core::exec::run_in_shell(
        shell,
        &command,
        args.cwd.as_deref(),
        &env,
        args.timeout_ms,
    )?;

    io::stdout().write_all(result.stdout.as_bytes())?;
    io::stdout().flush()?;
    io::stderr().write_all(result.stderr.as_bytes())?;
    io::stderr().flush()?;

    Ok(exit_code_from_child(result.exit_code))
}

/// Child statuses in `0..=255` are forwarded as-is. Any other `i32` maps to `1`.
fn exit_code_from_child(code: i32) -> ExitCode {
    u8::try_from(code)
        .map(ExitCode::from)
        .unwrap_or(ExitCode::from(1))
}
