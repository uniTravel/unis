//! # **unis** 发送者

use crate::{
    UniResponse,
    app::Context,
    domain::{Aggregate, CommandEnum, EventEnum},
};
use rkyv::{
    Archive, Deserialize,
    de::Pool,
    rancor::{Error, Strategy},
};
use tokio::sync::{mpsc::error::SendError, oneshot};
use tracing::{error, info, instrument};
use uuid::Uuid;

/// 发送者特征
pub trait Sender<A, C, E>: Sized + 'static
where
    A: Aggregate,
    C: CommandEnum<A = A, E = E>,
    <C as Archive>::Archived: Deserialize<C, Strategy<Pool, Error>>,
    E: EventEnum<A = A>,
    <E as Archive>::Archived: Deserialize<E, Strategy<Pool, Error>>,
{
    #[doc(hidden)]
    fn new(ctx: &'static Context) -> impl Future<Output = Result<Self, String>>;
    /// 获取聚合类型主题
    fn topic(&self) -> &'static str;
    /// 请求处理回复
    fn send(&self, todo: Todo<A, C, E>) -> Result<(), SendError<Todo<A, C, E>>>;

    /// 发送创建聚合命令
    #[inline]
    fn create(&self, com_id: Uuid, com: C) -> impl Future<Output = UniResponse> {
        async move {
            let agg_id = Uuid::new_v4();
            self.change(agg_id, com_id, com).await
        }
    }

    /// 发送变更聚合命令
    #[instrument(name = "send_command", skip_all, fields(topic = self.topic(), %agg_id, %com_id))]
    fn change(&self, agg_id: Uuid, com_id: Uuid, com: C) -> impl Future<Output = UniResponse> {
        async move {
            let (res_tx, res_rx) = oneshot::channel::<UniResponse>();
            if let Err(e) = self.send(Todo::Reply {
                agg_id,
                com_id,
                com,
                res_tx,
            }) {
                error!("聚合命令请求反馈错误：{e}");
                panic!("响应处理器停止工作");
            }

            info!("发送聚合命令");
            match res_rx.await {
                Ok(res) => {
                    info!("聚合命令收到反馈：{res}");
                    res
                }
                Err(e) => {
                    error!("聚合命令接收反馈错误：{e}");
                    UniResponse::Timeout
                }
            }
        }
    }
}

/// 命令积压项
pub enum Todo<A, C, E>
where
    A: Aggregate,
    C: CommandEnum<A = A, E = E>,
    <C as Archive>::Archived: Deserialize<C, Strategy<Pool, Error>>,
    E: EventEnum<A = A>,
    <E as Archive>::Archived: Deserialize<E, Strategy<Pool, Error>>,
{
    /// 处理回复
    Reply {
        /// 聚合 Id
        agg_id: Uuid,
        /// 命令 Id
        com_id: Uuid,
        /// 命令
        com: C,
        /// 回复通道
        res_tx: oneshot::Sender<UniResponse>,
    },
    /// 处理响应
    Response {
        /// 命令 Id
        com_id: Uuid,
        /// 响应
        res: UniResponse,
    },
}

/// 为聚合类型构造路由
#[macro_export]
macro_rules! route_builder {
    ($agg:ident, $format:ty, [$($cr:ident), *], [$($ch:ident), *]) => {{
        let mut router = Router::new();
        $(
            router = router.route(
                concat!("/", stringify!($agg), "/", stringify!($cr), "/{com_id}"),
                post($cr::<$format>),
            );
        )*
        $(
            router = router.route(
                concat!("/", stringify!($agg), "/", stringify!($ch), "/{agg_id}/{com_id}"),
                post($ch::<$format>),
            );
        )*
        router
    }};
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-utils"))]
pub fn create(path: &str, op: &str, com_id: uuid::Uuid) -> String {
    format!("{path}/{op}/{com_id}")
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-utils"))]
pub fn change(path: &str, op: &str, agg_id: uuid::Uuid, com_id: uuid::Uuid) -> String {
    format!("{path}/{op}/{agg_id}/{com_id}")
}
