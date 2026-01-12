//! # Kafka 投影者

pub mod app;
pub(self) mod core;

pub use app::App;
pub use app::context;
use unis::config::build_config;

use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::atomic::AtomicBool;
use thiserror::Error;

static EXIT: AtomicBool = AtomicBool::new(false);
static CLOSED: AtomicBool = AtomicBool::new(false);

static CONFIG: LazyLock<config::Config> =
    LazyLock::new(|| build_config(PathBuf::from(env!("CARGO_MANIFEST_DIR"))));

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
