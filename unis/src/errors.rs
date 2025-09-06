//! 错误定义

use thiserror::Error;

/// 领域错误
#[derive(Debug, Error)]
pub enum DomainError {
    /// 命令数据验证错误
    #[error("命令数据验证错误：{0}")]
    CheckError(String),
    /// 序列化错误
    #[error("序列化错误")]
    EncodeError(#[from] bincode::error::EncodeError),
    /// 反序列化错误
    #[error("反序列化错误")]
    DecodeError(#[from] bincode::error::DecodeError),
    /// 获取信号量许可错误
    #[error("获取信号量许可错误")]
    AcquireError(#[from] tokio::sync::AcquireError),
    /// 发送命令错误
    #[error("发送命令错误：{0}")]
    SendError(String),
    /// 写入流存储错误
    #[error("写入流存储错误：{0}")]
    WriteError(String),
    /// 读取流存储错误
    #[error("读取流存储错误：{0}")]
    ReadError(String),
}

/// 配置错误
#[derive(Debug, Error)]
pub enum ConfigError {
    /// 配置加载错误
    #[error("配置加载错误: {0}")]
    LoadError(#[from] config::ConfigError),
    /// 配置验证错误
    #[error("配置 {section}.{key} 验证错误: {message}")]
    ValidationError {
        /// 配置节
        section: String,
        /// 配置键
        key: String,
        /// 错误消息
        message: String,
    },
    /// 配置未初始化
    #[error("配置未初始化")]
    NotInitialized,
    /// 重载配置错误
    #[error("重载配置错误")]
    ReloadFailed,
    /// 获取配置读锁错误
    #[error("获取配置读锁错误")]
    ReadFailed,
    /// 获取配置写锁错误
    #[error("获取配置写锁错误")]
    WriteFailed,
    /// 配置已初始化
    #[error("配置已初始化")]
    AlreadyInitialized,
}
