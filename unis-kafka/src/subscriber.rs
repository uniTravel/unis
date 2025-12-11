//! ## Kafka 订阅者

mod pool;
mod stream;
#[cfg(test)]
mod tests;

pub mod app;
pub mod core;
pub mod reader;

use crate::config::SubscriberConfig;
use std::sync::LazyLock;
use unis::domain::Config;
use uuid::Uuid;

static SUBSCRIBER_CONFIG: LazyLock<SubscriberConfig> = LazyLock::new(|| SubscriberConfig::get());

struct TopicTask {
    pub agg_type: &'static str,
    pub agg_id: Uuid,
}
