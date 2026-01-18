//! # **unis** 错误定义

use crate::Response;
use thiserror::Error;

/// **unis** 错误枚举
#[derive(Debug, Error)]
pub enum UniError {
    /// 命令重复提交
    #[error("命令重复提交")]
    Duplicate,
    /// 命令数据验证错误
    #[error("命令数据验证错误：{0}")]
    CheckError(String),
    /// 序列化错误
    #[error("序列化错误")]
    EncodeError(#[from] bincode::error::EncodeError),
    /// 反序列化错误
    #[error("反序列化错误")]
    DecodeError(#[from] bincode::error::DecodeError),
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
    pub fn response(&self) -> Response {
        match self {
            UniError::CheckError(_) => Response::CheckError,
            UniError::EncodeError(_) => Response::EncodeError,
            UniError::DecodeError(_) => Response::DecodeError,
            UniError::MsgError(_) => Response::MsgError,
            UniError::WriteError(_) => Response::WriteError,
            UniError::ReadError(_) => Response::ReadError,
            UniError::SendError(_) => Response::SendError,
            UniError::Duplicate => Response::Duplicate,
        }
    }
}

impl From<&str> for UniError {
    fn from(s: &str) -> Self {
        UniError::MsgError(s.to_owned())
    }
}
