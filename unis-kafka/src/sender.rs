//! # Kafka 发送者

mod app;
mod core;
mod macros;

pub use app::App;
pub use app::context;
#[doc(hidden)]
#[cfg(any(test, feature = "test-utils"))]
pub use app::test_context;
pub use core::Sender;
pub use unis::domain::Request;
pub use unis::{JsonFormat, RkyvFormat, UniCommand, UniKey, UniResponse};

use uuid::Uuid;

#[doc(hidden)]
#[cfg(any(test, feature = "test-utils"))]
pub fn create(path: &str, op: &str, com_id: Uuid) -> String {
    format!("{path}/{op}/{com_id}")
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-utils"))]
pub fn change(path: &str, op: &str, agg_id: Uuid, com_id: Uuid) -> String {
    format!("{path}/{op}/{agg_id}/{com_id}")
}
