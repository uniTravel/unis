//! # **unis** 核心库
//!
//!

#![warn(missing_docs)]

pub(crate) mod aggregator;
pub(crate) mod config;
pub mod domain;
pub mod errors;
pub mod kafka;
pub(crate) mod pool;

use bincode::config::{Configuration, Fixint, Limit, LittleEndian};
use bytes::Bytes;

/// bincode 标准配置
pub const BINCODE_CONFIG: Configuration = bincode::config::standard();

/// bincode 定长消息头配置
pub const BINCODE_HEADER: Configuration<LittleEndian, Fixint, Limit<1>> =
    bincode::config::standard()
        .with_fixed_int_encoding()
        .with_limit::<1>();

/// 空 Bytes
pub const EMPTY_BYTES: Bytes = Bytes::new();
