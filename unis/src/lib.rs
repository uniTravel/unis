//! # **unis** 核心库
//!
//!

#![warn(missing_docs)]

pub mod aggregator;
pub mod config;
pub mod domain;
pub mod errors;
pub mod pool;

use bincode::config::Configuration;
use bytes::Bytes;
use uuid::Uuid;

/// bincode 标准配置
pub const BINCODE_CONFIG: Configuration = bincode::config::standard();

/// 空 Bytes
pub const EMPTY_BYTES: Bytes = Bytes::new();

/// 命令消息结构
pub struct Com {
    /// 聚合 Id
    pub agg_id: Uuid,
    /// 命令 Id
    pub com_id: Uuid,
    /// 命令数据
    pub com_data: Vec<u8>,
}

/// 命令处理结果枚举
#[repr(u8)]
#[derive(Debug, ::bincode::Encode, ::bincode::Decode)]
pub enum Response {
    /// 成功
    Success = 0,
    /// 重复提交
    Duplicate = 1,
    /// 命令数据验证错误
    CheckError = 2,
    /// 序列化错误
    EncodeError = 3,
    /// 反序列化错误
    DecodeError = 4,
    /// 消息处理错误
    MsgError = 5,
    /// 写入事件流错误
    WriteError = 6,
    /// 读取事件流错误
    ReadError = 7,
    /// 写入命令流错误
    SendError = 8,
}
