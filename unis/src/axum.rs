use crate::{
    UniResponse,
    domain::Command,
    i18n,
    request::{self, JsonFormat, RkyvFormat},
};
use axum::{
    body::Bytes,
    extract::{FromRequest, Json, Request},
    http::{StatusCode, header::ACCEPT_LANGUAGE},
    middleware::Next,
    response::{IntoResponse, Response},
};
use rkyv::{
    Archive, Deserialize,
    bytecheck::CheckBytes,
    de::Pool,
    rancor::{Error, Strategy},
    util::AlignedVec,
    validation::{Validator, archive::ArchiveValidator, shared::SharedValidator},
};
use serde::de::DeserializeOwned;
use std::marker::PhantomData;
use tracing::info;
use validator::Validate;

/// 处理键值的中间件
pub async fn key_middleware(mut req: Request, next: Next) -> Response {
    let lang = extract_language(&req);
    match request::extract_key(req.headers()) {
        Some(key) => {
            req.extensions_mut().insert(key.clone());
            let mut res = next.run(req).await;
            let headers = res.headers_mut();
            headers.insert("x-agg-id", key.agg_id.to_string().parse().unwrap());
            headers.insert("x-com-id", key.com_id.to_string().parse().unwrap());
            res
        }
        None => response(UniResponse::KeyError, &lang).into_response(),
    }
}

/// 解析命令请求体
pub struct AxumCommand<T, F>(pub T, pub String, pub PhantomData<F>);

impl<T, S> FromRequest<S> for AxumCommand<T, RkyvFormat>
where
    T: Command + Validate,
    <T as Archive>::Archived: Deserialize<T, Strategy<Pool, Error>>,
    <T as Archive>::Archived:
        for<'m> CheckBytes<Strategy<Validator<ArchiveValidator<'m>, SharedValidator>, Error>>,
    Bytes: FromRequest<S>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let lang = extract_language(&req);
        let bytes = Bytes::from_request(req, state)
            .await
            .map_err(|e| e.into_response())?;

        let required_align = std::mem::align_of::<T::Archived>();
        let com = match bytes.as_ptr().align_offset(required_align) {
            0 => rkyv::from_bytes::<T, Error>(&bytes)
                .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()).into_response())?,
            _ => {
                let mut aligned = AlignedVec::<16>::with_capacity(bytes.len());
                aligned.extend_from_slice(&bytes);
                rkyv::from_bytes::<T, Error>(&aligned)
                    .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()).into_response())?
            }
        };

        com.validate().map_err(|e| {
            let mut result = String::new();
            let err = match i18n::validation(&mut result, &e, &lang) {
                Ok(()) => result,
                Err(_) => e.to_string(),
            };
            (StatusCode::BAD_REQUEST, err).into_response()
        })?;

        Ok(AxumCommand(com, lang, PhantomData))
    }
}

impl<T, S> FromRequest<S> for AxumCommand<T, JsonFormat>
where
    T: Command + Validate + DeserializeOwned,
    Bytes: FromRequest<S>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let lang = extract_language(&req);
        let bytes = Bytes::from_request(req, state)
            .await
            .map_err(|e| e.into_response())?;

        let Json(com) = Json::<T>::from_bytes(&bytes).map_err(|e| e.into_response())?;

        com.validate().map_err(|e| {
            let mut result = String::new();
            let err = match i18n::validation(&mut result, &e, &lang) {
                Ok(()) => result,
                Err(_) => e.to_string(),
            };
            (StatusCode::BAD_REQUEST, err).into_response()
        })?;

        Ok(AxumCommand(com, lang, PhantomData))
    }
}

fn extract_language(req: &Request) -> String {
    req.headers()
        .get(ACCEPT_LANGUAGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|ls| ls.split(',').next())
        .and_then(|l| l.split(';').next())
        .and_then(|t| t.split('-').next())
        .unwrap_or("zh")
        .to_string()
}

fn response(res: UniResponse, lang: &str) -> (StatusCode, String) {
    let (status, code) = match res {
        UniResponse::ValidateError => (StatusCode::BAD_REQUEST, "validate"),
        UniResponse::AuthError => (StatusCode::UNAUTHORIZED, "auth"),
        UniResponse::Timeout => (StatusCode::REQUEST_TIMEOUT, "timeout"),
        UniResponse::Conflict => (StatusCode::CONFLICT, "conflict"),
        UniResponse::KeyError => (StatusCode::BAD_REQUEST, "key"),
        UniResponse::CheckError => (StatusCode::INTERNAL_SERVER_ERROR, "check"),
        UniResponse::CodeError => (StatusCode::INTERNAL_SERVER_ERROR, "code"),
        UniResponse::MsgError => (StatusCode::INTERNAL_SERVER_ERROR, "msg"),
        UniResponse::WriteError => (StatusCode::INTERNAL_SERVER_ERROR, "write"),
        UniResponse::ReadError => (StatusCode::INTERNAL_SERVER_ERROR, "read"),
        UniResponse::SendError => (StatusCode::INTERNAL_SERVER_ERROR, "send"),
        UniResponse::Success => (StatusCode::OK, "_"),
        UniResponse::Duplicate => (StatusCode::ACCEPTED, "accepted"),
    };
    match i18n::RESPONSE
        .get(lang)
        .or(i18n::RESPONSE.get("zh"))
        .and_then(|l| l.get(code))
    {
        Some(r) => (status, r.to_string()),
        None => (status, code.to_string()),
    }
}

/// 转换为 axum 响应
pub fn into(
    res: Result<Vec<u8>, UniResponse>,
    lang: &str,
) -> Result<Vec<u8>, (StatusCode, String)> {
    match res {
        Ok(res) => {
            info!("聚合命令收到成功反馈");
            Ok(res)
        }
        Err(UniResponse::Duplicate) => {
            info!("聚合命令已执行成功");
            Err(response(UniResponse::Duplicate, lang))
        }
        Err(res) => {
            info!("聚合命令收到失败反馈：{res}");
            Err(response(res, lang))
        }
    }
}

/// 为聚合类型构造路由
#[macro_export]
macro_rules! route_builder {
    ($agg:ident, $sender:ty, [$($com:ident), *]) => {
        pub fn rkyv_routes() -> ::axum::Router<std::sync::Arc<$sender>> {
            let mut router = ::axum::Router::new();
            $(
                router = router.route(
                    concat!("/", stringify!($agg), "/", stringify!($com)),
                    ::axum::routing::post($com::<::unis::RkyvFormat>),
                );
            )*
            router.layer(::axum::middleware::from_fn(::unis::key_middleware))
        }

        pub fn json_routes() -> ::axum::Router<std::sync::Arc<$sender>> {
            let mut router = ::axum::Router::new();
            $(
                router = router.route(
                    concat!("/", stringify!($agg), "/", stringify!($com)),
                    ::axum::routing::post($com::<::unis::JsonFormat>),
                );
            )*
            router.layer(::axum::middleware::from_fn(::unis::key_middleware))
        }
    };
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-utils"))]
pub async fn apply(
    app: &'static axum::Router,
    path: &str,
    op: &str,
    agg_id: uuid::Uuid,
    com: impl crate::domain::Command,
) -> (axum::http::StatusCode, axum::body::Bytes) {
    let app = app.clone();
    let com_id = uuid::Uuid::new_v4();
    let span_id = uuid::Uuid::new_v4();
    let path = format!("{path}/{op}");
    let res = tower::ServiceExt::oneshot(
        app,
        axum::http::Request::post(path)
            .header(axum::http::header::CONTENT_TYPE, "application/octet-stream")
            .header(
                "traceparent",
                format!(
                    "00-{}-{:016x}-01",
                    com_id.as_simple(),
                    span_id.as_u64_pair().0,
                ),
            )
            .header("x-agg-id", agg_id.to_string())
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
