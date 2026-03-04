//! # Kafka 投影者

mod app;
mod core;

pub use app::App;
pub use app::context;
#[doc(hidden)]
#[cfg(any(test, feature = "test-utils"))]
pub use app::test_context;
pub use unis::domain::Aggregate;

use crate::config::ProjectorConfig;
use std::sync::LazyLock;
use thiserror::Error;
use unis::domain::Config;

static PROJECTOR_CONFIG: LazyLock<ProjectorConfig> = LazyLock::new(|| ProjectorConfig::get());

#[derive(Debug, Error)]
enum ProjectError {
    #[error("{0}")]
    UniError(#[from] unis::errors::UniError),
    #[error("无法获取消费组元数据")]
    MetadataError,
    #[error("{0}")]
    KafkaError(#[from] rdkafka::error::KafkaError),
}
