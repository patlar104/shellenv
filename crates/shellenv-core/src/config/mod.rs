pub mod load;
pub mod schema;

pub use load::ConfigError;
pub use schema::{Config, Paths, Profile, PythonConfig, SCHEMA_VERSION, ShellConfig};

// later: pub mod migrate;
