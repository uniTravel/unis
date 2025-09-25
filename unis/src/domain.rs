//! **unis** 特征

use crate::{Res, errors::DomainError};
use ahash::{AHashMap, AHashSet};
use bytes::Bytes;
use std::future::Future;
use uuid::Uuid;

/// 聚合特征
pub trait Aggregate: Send + Clone + 'static {
    /// 构造函数
    fn new(id: Uuid) -> Self;
    /// 递增 revision
    fn next(&mut self);
    /// 获取 id
    fn id(&self) -> Uuid;
    /// 获取 revision
    fn revision(&self) -> u64;
}

/// 事件特征
pub trait Event {
    /// 聚合类型
    type A: Aggregate;

    /// 事件应用到聚合
    fn apply(&self, agg: &mut Self::A);
}

/// 命令特征
pub trait Command {
    /// 聚合类型
    type A: Aggregate;
    /// 事件类型
    type E: Event<A = Self::A>;

    /// 检查命令是否合法
    fn check(&self, agg: &Self::A) -> Result<(), DomainError>;
    /// 执行命令，生成相应事件
    fn execute(&self, agg: &Self::A) -> Self::E;
    /// 处理命令
    fn process(&self, na: &mut Self::A) -> Result<Self::E, DomainError> {
        self.check(&na)?;
        let evt = self.execute(&na);
        evt.apply(na);
        Ok(evt)
    }
}

/// 事件枚举
pub trait EventEnum: bincode::Encode + Send + 'static {
    /// 聚合类型
    type A: Aggregate;
}

/// 命令枚举
pub trait CommandEnum {
    /// 聚合类型
    type A: Aggregate;
}

/// 恢复命令操作记录特征
pub trait Restore: Send + Sync + 'static {
    /// 返回类型
    type Fut: Future<Output = Result<AHashMap<Uuid, AHashSet<Uuid>>, DomainError>> + Send;

    /// 从存储恢复命令操作记录
    fn restore(&self, agg_type: &'static str, latest: i64) -> Self::Fut;
}

/// 加载聚合事件流特征
pub trait Load: Send + Sync + Copy + 'static {
    /// 返回类型
    type Fut: Future<Output = Result<Vec<Vec<u8>>, DomainError>> + Send;

    /// 从存储加载聚合事件流
    fn load(&self, agg_type: &'static str, agg_id: Uuid) -> Self::Fut;
}

/// 分发特征
pub trait Dispatch<A, E, L>: Send + Sync + Copy + 'static
where
    A: Aggregate,
    E: EventEnum<A = A>,
    L: Load,
{
    /// 返回类型
    type Fut: Future<Output = Result<(A, E), DomainError>> + Send;

    /// 分发回调函数
    fn dispatch(
        &self,
        agg_type: &'static str,
        agg_id: Uuid,
        com_data: Vec<u8>,
        agg: A,
        loader: L,
    ) -> Self::Fut;
}

/// 流写入特征
pub trait Stream: Send + Sync + 'static {
    /// 领域事件写入流
    fn write(
        &self,
        agg_type: &'static str,
        agg_id: Uuid,
        com_id: Uuid,
        revision: u64,
        evt_data: Bytes,
    ) -> impl Future<Output = Result<(), DomainError>> + Send;
    /// 错误反馈写入流
    fn respond(
        &self,
        agg_type: &'static str,
        agg_id: Uuid,
        com_id: Uuid,
        res: Res,
        evt_data: Bytes,
    ) -> impl Future<Output = Result<(), DomainError>> + Send;
}

/// 配置特征
pub trait Config: Sized + 'static {
    /// 获取配置
    fn get() -> Self;
    /// 重载配置
    fn reload();
}

impl<F, Fut> Restore for F
where
    F: Fn(&'static str, i64) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<AHashMap<Uuid, AHashSet<Uuid>>, DomainError>> + Send + 'static,
{
    type Fut = Fut;

    #[inline]
    fn restore(&self, agg_type: &'static str, latest: i64) -> Self::Fut {
        self(agg_type, latest)
    }
}

impl<F, Fut> Load for F
where
    F: Fn(&'static str, Uuid) -> Fut + Send + Sync + Copy + 'static,
    Fut: Future<Output = Result<Vec<Vec<u8>>, DomainError>> + Send + 'static,
{
    type Fut = Fut;

    #[inline]
    fn load(&self, agg_type: &'static str, agg_id: Uuid) -> Self::Fut {
        self(agg_type, agg_id)
    }
}

impl<A, E, L, F, Fut> Dispatch<A, E, L> for F
where
    A: Aggregate,
    E: EventEnum<A = A>,
    L: Load,
    F: Fn(&'static str, Uuid, Vec<u8>, A, L) -> Fut + Send + Sync + Copy + 'static,
    Fut: Future<Output = Result<(A, E), DomainError>> + Send + 'static,
{
    type Fut = Fut;

    #[inline(always)]
    fn dispatch(
        &self,
        agg_type: &'static str,
        agg_id: Uuid,
        com_data: Vec<u8>,
        agg: A,
        loader: L,
    ) -> Self::Fut {
        self(agg_type, agg_id, com_data, agg, loader)
    }
}
