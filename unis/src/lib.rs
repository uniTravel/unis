//! # **unis** 核心库
//!
//!

#![warn(missing_docs)]

#[cfg(feature = "axum")]
mod axum;
#[cfg(feature = "sender")]
mod i18n;
#[cfg(feature = "sender")]
mod request;
mod response;

#[cfg(feature = "subscriber")]
pub mod aggregator;
pub mod app;
pub mod config;
pub mod domain;
pub mod errors;
#[cfg(feature = "sender")]
pub mod sender;
#[cfg(feature = "subscriber")]
pub mod subscriber;

/// # **unis** 宏
pub mod macros {
    pub use unis_macros::*;
}

pub use crate::response::UniResponse;
#[doc(hidden)]
#[cfg(any(test, feature = "test-utils"))]
#[cfg(feature = "axum")]
pub use axum::apply;
#[cfg(feature = "axum")]
pub use axum::{AxumCommand, into, key_middleware};
#[doc(hidden)]
#[cfg(any(test, feature = "test-utils"))]
#[cfg(feature = "sender")]
pub use i18n::validate;
#[cfg(feature = "sender")]
pub use request::{JsonFormat, RkyvFormat, UniKey};

use crate::domain::CommandEnum;
use rkyv::{
    Archive, Deserialize,
    de::Pool,
    rancor::{Error, Strategy},
};
use uuid::Uuid;

/// 命令消息结构
pub struct Com<C>
where
    C: CommandEnum,
    <C as Archive>::Archived: Deserialize<C, Strategy<Pool, Error>>,
{
    /// 聚合 Id
    pub agg_id: Uuid,
    /// 命令 Id
    pub com_id: Uuid,
    /// 命令数据
    pub com: C,
}
