use crate::{UniResponse, domain::Command, i18n};
use axum::{
    body::Bytes,
    extract::{FromRequest, Json, Request},
    http::{HeaderMap, StatusCode, header::ACCEPT_LANGUAGE},
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
use uuid::Uuid;
use validator::Validate;

/// 命令请求的路径参数
#[derive(Clone, Debug)]
pub struct UniKey {
    /// 聚合 Id
    pub agg_id: Uuid,
    /// 聚合命令 Id
    pub com_id: Uuid,
}

fn parse_traceparent(tp: &str) -> Result<Uuid, String> {
    if tp.is_empty() {
        return Err("traceparent 为空".into());
    }

    let parts: Vec<&str> = tp.split('-').collect();
    if parts.len() != 4 {
        return Err(format!(
            "格式错误，需要 4 段，实际 {} 段: '{}'",
            parts.len(),
            tp
        ));
    }

    if parts[0] != "00" {
        return Err(format!("不支持的版本号: '{}'", parts[0]));
    }

    if parts[1].len() != 32 {
        return Err(format!(
            "trace_id 长度错误，需要 32 hex，实际 {} 字符: '{}'",
            parts[1].len(),
            parts[1]
        ));
    }

    if !parts[1].chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("trace_id 包含非法字符: '{}'", parts[1]));
    }

    if parts[1] == "00000000000000000000000000000000" {
        return Err("trace_id 为全零，无效".into());
    }

    let uuid_str = format!(
        "{}-{}-{}-{}-{}",
        &parts[1][0..8],
        &parts[1][8..12],
        &parts[1][12..16],
        &parts[1][16..20],
        &parts[1][20..32]
    );
    Uuid::parse_str(&uuid_str).map_err(|e| format!("无法解析为 UUID: {}", e))
}

fn extract_key(headers: &HeaderMap) -> Option<UniKey> {
    if let Some(tp) = headers.get("traceparent").and_then(|v| v.to_str().ok()) {
        match parse_traceparent(tp) {
            Ok(com_id) => {
                let agg_id = headers
                    .get("x-agg-id")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| Uuid::parse_str(s).ok())
                    .unwrap_or(Uuid::new_v4());
                return Some(UniKey { agg_id, com_id });
            }
            Err(e) => {
                tracing::warn!("traceparent 格式错误: value='{}', error={}", tp, e);
                return None;
            }
        }
    }

    None
}

/// 处理键值的中间件
pub async fn key_middleware(mut req: Request, next: Next) -> Response {
    let lang = extract_language(&req);
    match extract_key(req.headers()) {
        Some(key) => {
            req.extensions_mut().insert(key.clone());
            let mut res = next.run(req).await;
            let headers = res.headers_mut();
            headers.insert("x-agg-id", key.agg_id.to_string().parse().unwrap());
            headers.insert("x-com-id", key.com_id.to_string().parse().unwrap());
            res
        }
        None => i18n::response(UniResponse::KeyError, &lang).into_response(),
    }
}

/// Json 格式
pub struct JsonFormat;
/// Rkyv 格式
pub struct RkyvFormat;

/// 解析命令请求体
pub struct UniCommand<T, F>(pub T, pub String, pub PhantomData<F>);

impl<T, S> FromRequest<S> for UniCommand<T, RkyvFormat>
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

        Ok(UniCommand(com, lang, PhantomData))
    }
}

impl<T, S> FromRequest<S> for UniCommand<T, JsonFormat>
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

        Ok(UniCommand(com, lang, PhantomData))
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
