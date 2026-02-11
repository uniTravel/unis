use crate::domain::Command;
use axum::{
    body::Bytes,
    extract::{FromRequest, Json},
    http::StatusCode,
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
#[derive(serde::Deserialize)]
pub struct UniKey {
    /// 聚合 Id
    pub agg_id: Uuid,
    /// 聚合命令 Id
    pub com_id: Uuid,
}

/// Json 格式
pub struct JsonFormat;
/// Rkyv 格式
pub struct RkyvFormat;

/// 解析命令请求体
pub struct UniCommand<T, F>(pub T, pub PhantomData<F>);

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

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, Self::Rejection> {
        let bytes = Bytes::from_request(req, state)
            .await
            .map_err(|e| e.into_response())?;

        let mut aligned = AlignedVec::<4096>::new();
        aligned.extend_from_slice(&bytes);
        let com = rkyv::from_bytes::<T, Error>(&aligned)
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()).into_response())?;

        com.validate()
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()).into_response())?;

        Ok(UniCommand(com, PhantomData))
    }
}

impl<T, S> FromRequest<S> for UniCommand<T, JsonFormat>
where
    T: Command + Validate + DeserializeOwned,
    Bytes: FromRequest<S>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, Self::Rejection> {
        let bytes = Bytes::from_request(req, state)
            .await
            .map_err(|e| e.into_response())?;

        let Json(com) = Json::<T>::from_bytes(&bytes).map_err(|e| e.into_response())?;

        com.validate()
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()).into_response())?;

        Ok(UniCommand(com, PhantomData))
    }
}
