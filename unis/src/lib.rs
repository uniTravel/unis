//! # **unis** 核心库
//!
//!

#![warn(missing_docs)]

pub mod aggregator;
pub mod config;
pub mod domain;
pub mod errors;

use bincode::config::Configuration;

/// bincode 配置
///
/// 采用 bincode 标准配置。
pub const BINCODE_CONFIG: Configuration = bincode::config::standard();
