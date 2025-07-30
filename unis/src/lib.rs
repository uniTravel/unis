pub mod aggregator;
pub mod domain;
pub mod errors;

use bincode::config::{self, Configuration};

pub const BINCODE_CONFIG: Configuration = config::standard();
