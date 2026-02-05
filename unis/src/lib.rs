//! # **unis** 核心库
//!
//!

#![warn(missing_docs)]

mod request;
mod response;

pub mod aggregator;
pub mod config;
pub mod domain;
pub mod errors;
#[cfg(feature = "test-utils")]
pub mod test_utils;

/// # **unis** 宏
pub mod macros {
    pub use unis_macros::*;
}

pub use request::{UniCommand, UniKey};
pub use response::UniResponse;

use crate::domain::CommandEnum;
use rkyv::{
    Archive, Deserialize,
    de::Pool,
    rancor::{Error, Strategy},
};
use uuid::Uuid;

const EMPTY_BYTES: &[u8] = &[];

/// 命令消息结构
pub struct Com<C>
where
    C: CommandEnum,
    <C as Archive>::Archived: Sync + Deserialize<C, Strategy<Pool, Error>>,
{
    /// 聚合 Id
    pub agg_id: Uuid,
    /// 命令 Id
    pub com_id: Uuid,
    /// 命令数据
    pub com: C,
}
