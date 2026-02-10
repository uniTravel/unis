//! # Kafka 投影者

mod app;
mod core;

pub use app::App;
pub use app::context;
#[doc(hidden)]
#[cfg(any(test, feature = "test-utils"))]
pub use app::test_context;

use std::sync::LazyLock;
use std::sync::atomic::AtomicBool;
use thiserror::Error;
use unis::config::build_config;

static EXIT: AtomicBool = AtomicBool::new(false);
static CLOSED: AtomicBool = AtomicBool::new(false);

static CONFIG: LazyLock<config::Config> = LazyLock::new(|| build_config());

#[derive(Debug, Error)]
enum ProjectError {
    #[error("{0}")]
    UniError(#[from] unis::errors::UniError),
    #[error("无法获取消费组元数据")]
    MetadataError,
    #[error("生产者不存在")]
    ProducerNotFound,
    #[error("{0}")]
    KafkaError(#[from] rdkafka::error::KafkaError),
}
