//! # **unis** 的 Kafka 实现
//!
//!

pub(crate) mod commit;
pub(crate) mod config;
pub(crate) mod pool;
pub(crate) mod stream;
pub(crate) mod topic;

pub mod errors;
pub mod reader;
pub mod sender;
pub mod subscriber;

use bincode::config::{Configuration, Fixint, Limit, LittleEndian};

/// bincode 定长消息头配置
pub const BINCODE_HEADER: Configuration<LittleEndian, Fixint, Limit<1>> =
    bincode::config::standard()
        .with_fixed_int_encoding()
        .with_limit::<1>();
