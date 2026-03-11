//! # **unis** 订阅者

use crate::{
    app::Context,
    domain::{Aggregate, CommandEnum, EventEnum},
    errors::UniError,
};
use ahash::{AHashMap, AHashSet};
use rkyv::{
    Archive, Deserialize,
    de::Pool,
    rancor::{Error, Strategy},
};
use uuid::Uuid;

/// 恢复命令操作记录特征
pub trait Restore: Send + 'static {
    /// 返回类型
    type Fut: Future<Output = Result<AHashMap<Uuid, AHashSet<Uuid>>, UniError>> + Send;

    /// 从存储恢复命令操作记录
    fn restore(&self, agg_type: &'static str, latest: i64) -> Self::Fut;
}

/// 流写入特征
pub trait Stream: Send + Sync + 'static {
    /// 事件写入流
    fn write(
        &self,
        agg_type: &'static str,
        agg_id: Uuid,
        com_id: Uuid,
        revision: u64,
        evt_data: &[u8],
    ) -> impl Future<Output = Result<(), UniError>> + Send;
    /// 错误反馈写入流
    fn respond(
        &self,
        agg_type: &'static str,
        agg_id: Uuid,
        com_id: Uuid,
        res: &[u8; 1],
        evt_data: &[u8],
    ) -> impl Future<Output = Result<(), UniError>> + Send;
}

/// 订阅者特征
pub trait Subscriber<A, C, E>: 'static
where
    A: Aggregate,
    C: CommandEnum<A = A, E = E>,
    <C as Archive>::Archived: Deserialize<C, Strategy<Pool, Error>>,
    E: EventEnum<A = A>,
    <E as Archive>::Archived: Deserialize<E, Strategy<Pool, Error>>,
{
    #[doc(hidden)]
    fn launch(ctx: &'static Context) -> impl Future<Output = Result<(), String>>;
}
