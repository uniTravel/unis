//! # Kafka 投影者

mod app;
mod core;

pub use app::App;
pub use app::context;
#[doc(hidden)]
#[cfg(any(test, feature = "test-utils"))]
pub use app::test_context;
pub use unis::domain::Aggregate;

use std::sync::LazyLock;
use thiserror::Error;
use unis::config::build_config;

static CONFIG: LazyLock<config::Config> = LazyLock::new(|| build_config());

#[derive(Debug, Error)]
enum ProjectError {
    #[error("{0}")]
    UniError(#[from] unis::errors::UniError),
    #[error("无法获取消费组元数据")]
    MetadataError,
    #[error("{0}")]
    KafkaError(#[from] rdkafka::error::KafkaError),
}
