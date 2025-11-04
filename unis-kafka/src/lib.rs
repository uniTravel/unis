//! # **unis** 的 Kafka 实现
//!
//!

pub(crate) mod commit;
pub(crate) mod config;
pub(crate) mod pool;
pub(crate) mod stream;
pub(crate) mod topic;

pub mod reader;
pub mod sender;
pub mod subscriber;

#[cfg(test)]
mod tests;

use bincode::config::{Configuration, Fixint, Limit, LittleEndian};
use rdkafka::{ClientConfig, admin::AdminClient, client::DefaultClientContext};
use std::sync::LazyLock;

const BINCODE_HEADER: Configuration<LittleEndian, Fixint, Limit<4>> = bincode::config::standard()
    .with_fixed_int_encoding()
    .with_limit::<4>();

static ADMIN: LazyLock<AdminClient<DefaultClientContext>> = LazyLock::new(|| {
    ClientConfig::new()
        .set(
            "bootstrap.servers",
            &subscriber::SUBSCRIBER_CONFIG.bootstrap,
        )
        .create()
        .expect("管理客户端创建失败")
});
