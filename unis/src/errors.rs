//! # **unis** 错误定义

use crate::UniResponse;
use thiserror::Error;

/// 命令数据验证错误
#[derive(Debug, Error)]
#[error("{0}")]
pub struct CheckError(pub &'static str);

/// **unis** 错误枚举
#[derive(Debug, Error)]
pub enum UniError {
    /// 命令重复提交
    #[error("命令重复提交")]
    Duplicate,
    /// 命令数据验证错误
    #[error(transparent)]
    CheckError(#[from] CheckError),
    /// 序列化错误
    #[error(transparent)]
    CodeError(#[from] rkyv::rancor::Error),
    /// 消息处理错误
    #[error("消息处理错误：{0}")]
    MsgError(String),
    /// 写入事件流错误
    #[error("写入事件流错误：{0}")]
    WriteError(String),
    /// 读取事件流错误
    #[error("读取事件流错误：{0}")]
    ReadError(String),
    /// 写入命令流错误
    #[error("写入命令流错误错误：{0}")]
    SendError(String),
}

impl UniError {
    /// 转换成 Response 结构
    pub fn response(&self) -> UniResponse {
        match self {
            UniError::CheckError(_) => UniResponse::CheckError,
            UniError::CodeError(_) => UniResponse::CodeError,
            UniError::MsgError(_) => UniResponse::MsgError,
            UniError::WriteError(_) => UniResponse::WriteError,
            UniError::ReadError(_) => UniResponse::ReadError,
            UniError::SendError(_) => UniResponse::SendError,
            UniError::Duplicate => UniResponse::Duplicate,
        }
    }
}

impl From<&str> for UniError {
    #[inline(always)]
    fn from(value: &str) -> Self {
        UniError::MsgError(value.to_owned())
    }
}

impl From<String> for UniError {
    #[inline(always)]
    fn from(value: String) -> Self {
        UniError::MsgError(value)
    }
}
