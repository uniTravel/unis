use thiserror::Error;

#[derive(Debug, Error)]
pub enum SubscriberError {
    #[error("Kafka错误")]
    Kafka(#[from] rdkafka::error::KafkaError),
    #[error("处理错误：{0}")]
    Processing(String),
    #[error("关闭错误: {0}")]
    Shutdown(String),
}

impl From<&str> for SubscriberError {
    fn from(s: &str) -> Self {
        SubscriberError::Processing(s.to_string())
    }
}
