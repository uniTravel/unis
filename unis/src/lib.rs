//! # **unis** 核心库
//!
//!

#![warn(missing_docs)]

pub mod aggregator;
pub mod config;
pub mod domain;
pub mod errors;
pub(crate) mod pool;

use bincode::config::{Configuration, Fixint, LittleEndian, NoLimit};
use bytes::Bytes;

/// bincode 标准配置
pub const BINCODE_CONFIG: Configuration = bincode::config::standard();

/// bincode 定长消息头配置
pub const BINCODE_HEADER: Configuration<LittleEndian, Fixint, NoLimit> =
    bincode::config::standard()
        .with_fixed_int_encoding()
        .with_no_limit();

/// 空 Bytes
pub const EMPTY_BYTES: Bytes = Bytes::new();
