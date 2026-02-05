//! # Kafka 发送者

mod app;
mod core;

pub use app::App;
pub use app::context;
#[doc(hidden)]
#[cfg(any(test, feature = "test-utils"))]
pub use app::test_context;
pub use core::Sender;
