pub mod aggregator;
pub mod config;
pub mod domain;
pub mod errors;

use bincode::config::Configuration;

pub const BINCODE_CONFIG: Configuration = bincode::config::standard();
