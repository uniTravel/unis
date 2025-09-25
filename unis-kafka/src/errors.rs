//! Kafka错误定义

use thiserror::Error;

/// 订阅者错误
#[derive(Debug, Error)]
pub enum SubscriberError {
    /// Kafka错误
    #[error("Kafka错误")]
    Kafka(#[from] rdkafka::error::KafkaError),
    /// 处理错误
    #[error("处理错误：{0}")]
    Processing(String),
    /// 关闭错误
    #[error("关闭错误: {0}")]
    Shutdown(String),
}

impl From<&str> for SubscriberError {
    fn from(s: &str) -> Self {
        SubscriberError::Processing(s.to_string())
    }
}
