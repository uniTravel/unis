//! # **unis** 的 Kafka 实现
//!
//!

#![warn(missing_docs)]

pub(crate) mod commit;
pub(crate) mod config;

#[cfg(feature = "projector")]
pub mod projector;
#[cfg(feature = "sender")]
pub mod sender;
#[cfg(feature = "subscriber")]
pub mod subscriber;

use bincode::config::{Configuration, Fixint, Limit, LittleEndian};

#[allow(unused)]
const BINCODE_HEADER: Configuration<LittleEndian, Fixint, Limit<4>> = bincode::config::standard()
    .with_fixed_int_encoding()
    .with_limit::<4>();
