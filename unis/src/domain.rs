//! **unis** 特征

use crate::errors::DomainError;
use std::collections::HashMap;
use time::OffsetDateTime;
use uuid::Uuid;

/// 聚合特征
pub trait Aggregate {
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

/// 分发特征
pub trait Dispatch<A, L, R, S>
where
    A: Aggregate,
    L: Load<A, R, S>,
    R: Replay<A = A>,
    S: Stream<A = A>,
{
    /// 分发回调函数
    fn dispatch(
        &mut self,
        agg_id: Uuid,
        com_data: Vec<u8>,
        caches: &mut HashMap<Uuid, (A, OffsetDateTime)>,
        loader: &L,
        replayer: &R,
        stream: &S,
    ) -> Result<((A, OffsetDateTime), A, Vec<u8>), DomainError>;
}

/// 加载聚合特征
pub trait Load<A, R, S>
where
    A: Aggregate,
    R: Replay<A = A>,
    S: Stream<A = A>,
{
    /// 从存储获取聚合
    fn load(
        &self,
        agg_id: Uuid,
        caches: &mut HashMap<Uuid, (A, OffsetDateTime)>,
        replayer: &R,
        stream: &S,
    ) -> Result<(A, OffsetDateTime), DomainError>;
}

/// 重播特征
pub trait Replay {
    /// 聚合类型
    type A: Aggregate;

    /// 重播事件到聚合
    fn replay(&self, agg: &mut Self::A, evt_data: Vec<u8>) -> Result<(), DomainError>;
}

/// 流特征
pub trait Stream {
    /// 聚合类型
    type A: Aggregate;

    /// 写入流
    fn write(
        &self,
        agg_id: Uuid,
        com_id: Uuid,
        revision: u64,
        evt_data: Vec<u8>,
    ) -> Result<(), DomainError>;
    /// 从流读取
    fn read(&self, agg_id: Uuid) -> Result<Vec<Vec<u8>>, DomainError>;
}
