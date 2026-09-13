pub mod load;
pub mod schema;

pub use load::ConfigError;
pub use schema::{
    Config, Paths, Profile, PythonConfig, SCHEMA_VERSION, ShellConfig, json_schema,
    json_schema_pretty,
};

// later: pub mod migrate;
