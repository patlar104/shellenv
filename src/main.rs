use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use shellenv_core::config::load;
use shellenv_core::config::{SCHEMA_VERSION, json_schema_pretty};

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
        /// Path to the config TOML file
        path: PathBuf,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Commands::Config(args) => match args.command {
            ConfigCommand::Schema => {
                println!("{}", json_schema_pretty()?);
                Ok(())
            }
            ConfigCommand::SchemaVersion => {
                println!("{SCHEMA_VERSION}");
                Ok(())
            }
            ConfigCommand::Validate { path } => {
                load::load(Some(&path))?;
                println!("OK");
                Ok(())
            }
        },
    }
}
