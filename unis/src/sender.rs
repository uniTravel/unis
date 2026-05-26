//! # **unis** 发送者

use crate::{
    UniKey, UniResponse,
    app::Context,
    domain::{Aggregate, CommandEnum, EventEnum},
};
use opentelemetry::trace::Status;
use rkyv::{
    Archive, Deserialize,
    de::Pool,
    rancor::{Error, Strategy},
};
use tokio::sync::{mpsc::error::SendError, oneshot};
use tracing::{Span, error, info};
use tracing_opentelemetry::OpenTelemetrySpanExt;
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

    /// 发送聚合命令
    fn apply(
        &self,
        UniKey {
            agg_id,
            com_id,
            span_id,
        }: UniKey,
        com: C,
    ) -> impl Future<Output = Result<Vec<u8>, UniResponse>> {
        async move {
            let (res_tx, res_rx) = oneshot::channel::<Result<Vec<u8>, UniResponse>>();
            if let Err(e) = self.send(Todo::Reply {
                agg_id,
                com_id,
                span_id,
                com,
                res_tx,
            }) {
                panic!("聚合命令响应处理器已停止工作：{e}");
            }

            let span = Span::current();
            match res_rx.await {
                Ok(Ok(res)) => {
                    info!(%agg_id, "命令处理成功");
                    Ok(res)
                }
                Ok(Err(UniResponse::SendError)) => {
                    error!(%agg_id, error = ?UniResponse::SendError, "命令处理失败");
                    span.set_status(Status::error("发送命令失败"));
                    Err(UniResponse::SendError)
                }
                Ok(Err(UniResponse::ReadError)) => {
                    error!(%agg_id, error = ?UniResponse::ReadError, "命令处理失败");
                    span.set_status(Status::error("加载聚合事件流失败"));
                    Err(UniResponse::ReadError)
                }
                Ok(Err(UniResponse::WriteError)) => {
                    error!(%agg_id, error = ?UniResponse::WriteError, "命令处理失败");
                    span.set_status(Status::error("聚合事件持久化失败"));
                    Err(UniResponse::WriteError)
                }
                Ok(Err(e)) => {
                    error!(%agg_id, error = ?e, "命令处理失败");
                    Err(e)
                }
                Err(e) => {
                    error!(%agg_id, error = ?e, "命令结果反馈通道意外关闭");
                    Err(UniResponse::ResponseError)
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
        com_id: [u8; 16],
        /// 追踪跨度 Id
        span_id: [u8; 8],
        /// 命令
        com: C,
        /// 回复通道
        res_tx: oneshot::Sender<Result<Vec<u8>, UniResponse>>,
    },
    /// 处理响应
    Response {
        /// 聚合 Id
        agg_id: Uuid,
        /// 命令 Id
        com_id: [u8; 16],
        /// 追踪跨度 Id
        span_id: [u8; 8],
        /// 响应
        res: Result<Vec<u8>, UniResponse>,
    },
}
