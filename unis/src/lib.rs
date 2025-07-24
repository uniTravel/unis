pub mod aggregator;
pub mod domain;
pub mod errors;
pub(crate) mod pool;

use bincode::config::{self, Configuration};

pub const BINCODE_CONFIG: Configuration = config::standard();
