use axum::{Extension, Router, extract::State, http::StatusCode};
use domain::account::*;
use std::sync::Arc;
use unis::{AxumCommand, UniKey, sender::Sender};
use unis_kafka::sender::KafkaSender;
use utoipa::OpenApi;

/// 申请账户
#[utoipa::path(post, path = "/create", request_body = CreateAccount)]
pub async fn create<F>(
    Extension(UniKey { agg_id, com_id }): Extension<UniKey>,
    State(svc): State<Arc<KafkaSender<AccountCommand>>>,
    AxumCommand(com, lang, _): AxumCommand<CreateAccount, F>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let res = svc.apply(agg_id, com_id, AccountCommand::Create(com)).await;
    unis::into(res, &lang)
}

/// 审核账户申请
#[utoipa::path(post, path = "/verify", request_body = VerifyAccount)]
pub async fn verify<F>(
    Extension(UniKey { agg_id, com_id }): Extension<UniKey>,
    State(svc): State<Arc<KafkaSender<AccountCommand>>>,
    AxumCommand(com, lang, _): AxumCommand<VerifyAccount, F>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let res = svc.apply(agg_id, com_id, AccountCommand::Verify(com)).await;
    unis::into(res, &lang)
}

/// 审批账户申请
#[utoipa::path(post, path = "/approve", request_body = ApproveAccount)]
pub async fn approve<F>(
    Extension(UniKey { agg_id, com_id }): Extension<UniKey>,
    State(svc): State<Arc<KafkaSender<AccountCommand>>>,
    AxumCommand(com, lang, _): AxumCommand<ApproveAccount, F>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let res = svc
        .apply(agg_id, com_id, AccountCommand::Approve(com))
        .await;
    unis::into(res, &lang)
}

/// 调整账户限额
#[utoipa::path(post, path = "/limit", request_body = LimitAccount)]
pub async fn limit<F>(
    Extension(UniKey { agg_id, com_id }): Extension<UniKey>,
    State(svc): State<Arc<KafkaSender<AccountCommand>>>,
    AxumCommand(com, lang, _): AxumCommand<LimitAccount, F>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let res = svc.apply(agg_id, com_id, AccountCommand::Limit(com)).await;
    unis::into(res, &lang)
}

#[derive(OpenApi)]
#[openapi(paths(create, verify, limit, approve))]
pub struct AccountApi;

unis::route_builder!(
    account,
    KafkaSender<AccountCommand>,
    [create, verify, approve, limit]
);

pub async fn routes() -> Router {
    let ctx = unis::app::context().await;
    let svc = Arc::new(ctx.setup::<_, KafkaSender<AccountCommand>>().await);
    Router::new()
        .nest("/rkyv/v1", rkyv_routes())
        .nest("/v1", json_routes())
        .with_state(svc)
}
