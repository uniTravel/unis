//! # **unis** 发送者

use crate::{
    UniResponse,
    app::Context,
    domain::{Aggregate, CommandEnum, EventEnum},
    i18n,
};
use axum::http::StatusCode;
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
    #[inline(always)]
    fn create(
        &self,
        com_id: Uuid,
        com: C,
        lang: &str,
    ) -> impl Future<Output = Result<Vec<u8>, (axum::http::StatusCode, String)>> {
        let agg_id = ::uuid::Uuid::new_v4();
        #[cfg(any(test, feature = "test-utils"))]
        crate::app::test_context().insert(com_id, agg_id);
        self.change(agg_id, com_id, com, lang)
    }

    /// 发送变更聚合命令
    #[instrument(name = "send_command", skip_all, fields(topic = self.topic(), %agg_id, %com_id))]
    fn change(
        &self,
        agg_id: Uuid,
        com_id: Uuid,
        com: C,
        lang: &str,
    ) -> impl Future<Output = Result<Vec<u8>, (axum::http::StatusCode, String)>> {
        async move {
            let (res_tx, res_rx) = oneshot::channel::<Result<Vec<u8>, UniResponse>>();
            if let Err(e) = self.send(Todo::Reply {
                agg_id,
                com_id,
                com,
                res_tx,
            }) {
                panic!("聚合命令响应处理器已停止工作：{e}");
            }

            info!("发送聚合命令");
            match res_rx.await {
                Ok(res) => match res {
                    Ok(res) => {
                        info!("聚合命令收到成功反馈");
                        Ok(res)
                    }
                    Err(UniResponse::Duplicate) => {
                        info!("聚合命令已执行成功");
                        Err((StatusCode::ACCEPTED, agg_id.to_string()))
                    }
                    Err(res) => {
                        info!("聚合命令收到失败反馈：{res}");
                        Err(i18n::response(res, lang))
                    }
                },
                Err(e) => {
                    error!("聚合命令接收反馈错误：{e}");
                    Err(i18n::response(UniResponse::Timeout, lang))
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
        res_tx: oneshot::Sender<Result<Vec<u8>, UniResponse>>,
    },
    /// 处理响应
    Response {
        /// 命令 Id
        com_id: Uuid,
        /// 响应
        res: Result<Vec<u8>, UniResponse>,
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
pub async fn create(
    app: &'static axum::Router,
    path: &str,
    op: &str,
    com: impl crate::domain::Command,
) -> (axum::http::StatusCode, Uuid, axum::body::Bytes) {
    let app = app.clone();
    let com_id = Uuid::new_v4();
    let path = format!("{path}/{op}/{com_id}");
    let res = tower::ServiceExt::oneshot(
        app,
        axum::http::Request::post(path)
            .header(axum::http::header::CONTENT_TYPE, "application/octet-stream")
            .body(axum::body::Body::from(
                rkyv::api::high::to_bytes_in::<Vec<u8>, Error>(&com, Vec::new()).unwrap(),
            ))
            .unwrap(),
    )
    .await
    .unwrap();
    let status = res.status();
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let (_, agg_id) = crate::app::test_context().remove(&com_id).unwrap();
    (status, agg_id, body)
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-utils"))]
pub async fn change(
    app: &'static axum::Router,
    path: &str,
    op: &str,
    agg_id: Uuid,
    com: impl crate::domain::Command,
) -> (axum::http::StatusCode, axum::body::Bytes) {
    let app = app.clone();
    let com_id = Uuid::new_v4();
    let path = format!("{path}/{op}/{agg_id}/{com_id}");
    let res = tower::ServiceExt::oneshot(
        app,
        axum::http::Request::post(path)
            .header(axum::http::header::CONTENT_TYPE, "application/octet-stream")
            .body(axum::body::Body::from(
                rkyv::api::high::to_bytes_in::<Vec<u8>, Error>(&com, Vec::new()).unwrap(),
            ))
            .unwrap(),
    )
    .await
    .unwrap();
    let status = res.status();
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, body)
}
