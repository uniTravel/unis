//! ## Kafka 订阅者

pub(crate) mod pool;
pub(crate) mod stream;
#[cfg(test)]
pub(crate) mod tests;

pub mod app;
pub mod core;
pub mod reader;

use crate::config::SubscriberConfig;
use std::sync::LazyLock;
use unis::domain::Config;
use uuid::Uuid;

pub(crate) static SUBSCRIBER_CONFIG: LazyLock<SubscriberConfig> =
    LazyLock::new(|| SubscriberConfig::get());

pub(crate) struct TopicTask {
    pub agg_type: &'static str,
    pub agg_id: Uuid,
}

pub use app::App;
