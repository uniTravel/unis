use std::fmt::Debug;

use crate::domain::Command;
use axum::{
    body::Bytes,
    extract::FromRequest,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use rkyv::{
    Archive, Deserialize,
    de::Pool,
    rancor::{Error, Strategy},
};
use uuid::Uuid;

/// 命令请求的路径参数
#[derive(serde::Deserialize)]
pub struct UniKey {
    /// 聚合 Id
    pub agg_id: Uuid,
    /// 聚合命令 Id
    pub com_id: Uuid,
}

/// 解析命令请求体
pub struct UniCommand<T>(pub T);

impl<T, S> FromRequest<S> for UniCommand<T>
where
    T: Command + Debug,
    <T as Archive>::Archived: Deserialize<T, Strategy<Pool, Error>>,
    Bytes: FromRequest<S>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, Self::Rejection> {
        let bytes = Bytes::from_request(req, state)
            .await
            .map_err(|e| e.into_response())?;

        let value = T::from_bytes(&bytes)
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()).into_response())?;

        Ok(UniCommand(value))
    }
}
