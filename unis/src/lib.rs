//! # **unis** 核心库
//!
//!

#![warn(missing_docs)]

pub(crate) mod pool;

pub mod aggregator;
pub mod config;
pub mod domain;
pub mod errors;

use bincode::config::Configuration;
use bytes::Bytes;
use uuid::Uuid;

/// bincode 标准配置
pub const BINCODE_CONFIG: Configuration = bincode::config::standard();

/// 空 Bytes
pub const EMPTY_BYTES: Bytes = Bytes::new();

/// 聚合命令结构
pub struct Com {
    /// 聚合Id
    pub agg_id: Uuid,
    /// 命令Id
    pub com_id: Uuid,
    /// 命令数据
    pub com_data: Vec<u8>,
}

/// 命令处理结果枚举
#[repr(u8)]
#[derive(Debug, ::bincode::Encode, ::bincode::Decode)]
pub enum Res {
    /// 成功
    Success = 0,
    /// 重复
    Duplicate = 1,
    /// 失败
    Fail = 2,
}
