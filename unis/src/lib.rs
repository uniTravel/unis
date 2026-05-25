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
use opentelemetry::{
    Context, SpanId, TraceFlags, TraceId,
    trace::{SpanContext, TraceContextExt},
};
#[cfg(feature = "sender")]
pub use request::{JsonFormat, RkyvFormat, UniKey};
use tracing::{Span, error};
use tracing_opentelemetry::OpenTelemetrySpanExt;

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
    /// 聚合命令 Id
    pub com_id: [u8; 16],
    /// 追踪跨度 Id
    pub span_id: [u8; 8],
    /// 命令数据
    pub com: C,
}

/// 为 Span 附加上下文
pub fn span_context(span: Span, com_id: [u8; 16], span_id: [u8; 8]) -> Span {
    let span_context = SpanContext::new(
        TraceId::from_bytes(com_id),
        SpanId::from_bytes(span_id),
        TraceFlags::default(),
        false,
        Default::default(),
    );

    let cx = Context::new().with_remote_span_context(span_context);
    if let Err(e) = span.set_parent(cx) {
        error!(error = ?e, "设置 Span 上下文失败");
    }
    span
}

/// 为 Span 链接上下文
pub fn link_context(span: Span, com_id: [u8; 16], span_id: [u8; 8]) -> Span {
    let span_context = SpanContext::new(
        TraceId::from_bytes(com_id),
        SpanId::from_bytes(span_id),
        TraceFlags::default(),
        false,
        Default::default(),
    );

    span.add_link(span_context);
    span
}
