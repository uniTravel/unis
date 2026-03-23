//! # **unis** 核心库
//!
//!

#![warn(missing_docs)]

#[cfg(feature = "sender")]
mod i18n;
#[cfg(feature = "sender")]
mod request;
#[cfg(feature = "sender")]
mod response;

#[cfg(feature = "subscriber")]
pub mod aggregator;
pub mod app;
pub mod config;
pub mod domain;
pub mod errors;
#[cfg(feature = "sender")]
pub mod sender;
#[cfg(feature = "subscriber")]
pub mod subscriber;

/// # **unis** 宏
pub mod macros {
    pub use unis_macros::*;
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-utils"))]
pub use i18n::validate;
#[cfg(feature = "sender")]
pub use request::{JsonFormat, RkyvFormat, UniCommand, UniKey};
#[cfg(feature = "sender")]
pub use response::I18nResponse;

use crate::domain::CommandEnum;
use rkyv::{
    Archive, Deserialize,
    de::Pool,
    rancor::{Error, Strategy},
};
use uuid::Uuid;

/// 命令消息结构
pub struct Com<C>
where
    C: CommandEnum,
    <C as Archive>::Archived: Deserialize<C, Strategy<Pool, Error>>,
{
    /// 聚合 Id
    pub agg_id: Uuid,
    /// 命令 Id
    pub com_id: Uuid,
    /// 命令数据
    pub com: C,
}

/// 命令处理结果枚举
#[repr(u8)]
#[derive(Debug, PartialEq)]
pub enum UniResponse {
    /// 成功
    Success = 0,
    /// 命令数据验证错误
    ValidateError = 1,
    /// 身份验证错误
    AuthError = 2,
    /// 请求超时
    Timeout = 3,
    /// 被冲突的新命令取代
    Conflict = 4,
    /// 命令已执行成功
    Duplicate = 11,
    /// 命令无法应用到聚合
    CheckError = 12,
    /// 序列化错误
    CodeError = 13,
    /// 消息处理错误
    MsgError = 14,
    /// 写入事件流错误
    WriteError = 15,
    /// 读取事件流错误
    ReadError = 16,
    /// 写入命令流错误
    SendError = 17,
}

impl UniResponse {
    /// 反序列化
    pub fn from_bytes(res_data: &[u8]) -> Self {
        match res_data[0] {
            0 => UniResponse::Success,
            1 => UniResponse::ValidateError,
            2 => UniResponse::AuthError,
            3 => UniResponse::Timeout,
            4 => UniResponse::Conflict,
            11 => UniResponse::Duplicate,
            12 => UniResponse::CheckError,
            13 => UniResponse::CodeError,
            14 => UniResponse::MsgError,
            15 => UniResponse::WriteError,
            16 => UniResponse::ReadError,
            17 => UniResponse::SendError,
            _ => panic!("转换命令处理结果枚举失败"),
        }
    }
    /// 序列化
    pub fn to_bytes(self) -> [u8; 1] {
        [self as u8]
    }
}
