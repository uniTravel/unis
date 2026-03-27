//! # **unis** 领域特征

use crate::errors::UniError;
use ahash::AHashSet;
use rkyv::{
    Archive, Deserialize, Serialize,
    api::high::to_bytes_with_alloc,
    de::Pool,
    rancor::{Error, Strategy},
    ser::{
        Serializer,
        allocator::{Arena, ArenaHandle},
        sharing::Share,
    },
    util::AlignedVec,
};
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
    /// 获取聚合类型全称
    fn type_name() -> &'static str;
}

/// 事件特征
pub trait Event: Archive + 'static {
    /// 聚合类型
    type A: Aggregate;

    /// 事件应用到聚合
    fn apply(&self, agg: &mut Self::A);

    /// 处理事件
    #[inline]
    fn process(&self, agg: &mut Self::A) {
        self.apply(agg);
        agg.next();
    }
}

/// 命令特征
pub trait Command: Archive + Sized + Clone + 'static {
    /// 聚合类型
    type A: Aggregate;
    /// 事件类型
    type E: Event<A = Self::A>;

    /// 检查命令是否合法
    fn check(&self, agg: &Self::A) -> Result<(), UniError>;
    /// 执行命令，生成相应事件
    fn apply(self, agg: &Self::A) -> Self::E;

    /// 处理命令
    #[inline]
    fn process(self, na: &mut Self::A) -> Result<Self::E, UniError> {
        self.check(&na)?;
        let evt = self.apply(&na);
        evt.process(na);
        Ok(evt)
    }
}

/// 事件枚举特征
pub trait EventEnum:
    Send
    + Archive
    + Sized
    + for<'m> Serialize<Strategy<Serializer<AlignedVec, ArenaHandle<'m>, Share>, Error>>
    + 'static
where
    <Self as Archive>::Archived: Deserialize<Self, Strategy<Pool, Error>>,
{
    /// 聚合类型
    type A: Aggregate;

    /// 序列化
    #[inline(always)]
    fn to_bytes(&self, arena: &mut Arena) -> Result<AlignedVec, UniError> {
        Ok(to_bytes_with_alloc(self, arena.acquire())?)
    }

    /// 反序列化
    #[inline(always)]
    fn from_bytes(bytes: &[u8]) -> Result<Self, UniError> {
        let mut aligned = AlignedVec::<4096>::new();
        aligned.extend_from_slice(bytes);
        Ok(unsafe { rkyv::from_bytes_unchecked::<Self, Error>(&aligned) }?)
    }
}

/// 命令枚举特征
pub trait CommandEnum:
    Send
    + Archive
    + Sized
    + Sync
    + Clone
    + for<'m> Serialize<Strategy<Serializer<AlignedVec, ArenaHandle<'m>, Share>, Error>>
    + 'static
where
    <Self as Archive>::Archived: Deserialize<Self, Strategy<Pool, Error>>,
    <<Self as CommandEnum>::E as Archive>::Archived:
        Deserialize<<Self as CommandEnum>::E, Strategy<Pool, Error>>,
{
    /// 聚合类型
    type A: Aggregate;
    /// 事件枚举类型
    type E: EventEnum<A = Self::A>;

    /// 执行命令枚举
    fn apply(
        self,
        topic: &'static str,
        agg_id: Uuid,
        agg: Self::A,
        coms: &mut AHashSet<Uuid>,
        loader: impl Load<Self::E>,
    ) -> impl Future<Output = Result<(Self::A, Self::E), UniError>> + Send;

    /// 序列化
    #[inline(always)]
    fn to_bytes(&self, arena: &mut Arena) -> Result<AlignedVec, UniError> {
        Ok(to_bytes_with_alloc(self, arena.acquire())?)
    }

    /// 反序列化
    #[inline(always)]
    fn from_bytes(bytes: &[u8]) -> Result<Self, UniError> {
        let mut aligned = AlignedVec::<4096>::new();
        aligned.extend_from_slice(bytes);
        Ok(unsafe { rkyv::from_bytes_unchecked::<Self, Error>(&aligned) }?)
    }
}

/// 加载事件流特征
pub trait Load<E>: Send + Copy + 'static
where
    E: EventEnum,
    <E as Archive>::Archived: Deserialize<E, Strategy<Pool, Error>>,
{
    /// 返回类型
    type Fut: Future<Output = Result<Vec<(Uuid, E)>, UniError>> + Send;

    /// 从存储加载事件流
    fn load(&self, topic: &'static str, agg_id: Uuid) -> Self::Fut;
}

/// 配置特征
pub trait Config: Sized + 'static {
    /// 获取配置
    fn get() -> Self;
    /// 重载配置
    fn reload();
}
