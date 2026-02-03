//! # Kafka 订阅者

mod app;
mod core;
mod pool;
mod reader;
mod stream;
#[cfg(test)]
mod tests;

pub use app::App;
pub use app::context;
#[doc(hidden)]
#[cfg(any(test, feature = "test-utils"))]
pub use app::test_context;

use crate::config::SubscriberConfig;
use std::sync::LazyLock;
use unis::domain::Config;
use uuid::Uuid;

static SUBSCRIBER_CONFIG: LazyLock<SubscriberConfig> = LazyLock::new(|| SubscriberConfig::get());

struct TopicTask {
    pub agg_type: &'static str,
    pub agg_id: Uuid,
}
