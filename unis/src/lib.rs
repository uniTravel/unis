//! # **unis** 核心库
//!
//!

#![warn(missing_docs)]

pub(crate) mod aggregator;
pub(crate) mod config;
pub mod domain;
pub mod errors;
pub mod kafka;
pub(crate) mod pool;

use ahash::AHashMap;
use bincode::config::{Configuration, Fixint, Limit, LittleEndian};
use bytes::Bytes;
use domain::{Aggregate, Replay, Stream};
use errors::DomainError;
use tokio::time::Instant;
use uuid::Uuid;

/// bincode 标准配置
pub const BINCODE_CONFIG: Configuration = bincode::config::standard();

/// bincode 定长消息头配置
pub const BINCODE_HEADER: Configuration<LittleEndian, Fixint, Limit<1>> =
    bincode::config::standard()
        .with_fixed_int_encoding()
        .with_limit::<1>();

/// 空 Bytes
pub const EMPTY_BYTES: Bytes = Bytes::new();

/// 泛型的聚合加载函数
///
/// 构造聚合器时，具体化的聚合加载函数作为参数，系统会自动将其转为闭包传入。
pub fn loader<A, R, S>(
    agg_type: &'static str,
    agg_id: Uuid,
    caches: &mut AHashMap<Uuid, (A, Instant)>,
    replayer: &R,
) -> Result<(A, Instant), DomainError>
where
    A: Aggregate,
    R: Replay<A = A>,
    S: Stream,
{
    if let Some(o) = caches.remove(&agg_id) {
        return Ok(o);
    } else {
        let mut oa = A::new(agg_id);
        let ds = S::read(agg_type, agg_id)?;
        for evt_data in ds {
            replayer.replay(&mut oa, evt_data)?;
        }
        Ok((oa, Instant::now()))
    }
}
