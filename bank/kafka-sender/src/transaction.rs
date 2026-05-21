use axum::{Extension, Router, extract::State, http::StatusCode};
use domain::transaction::*;
use std::sync::Arc;
use unis::{AxumCommand, UniKey, sender::Sender};
use unis_kafka::sender::KafkaSender;
use utoipa::OpenApi;

/// 初始化交易期
#[utoipa::path(post, path = "/init", request_body = InitPeriod)]
pub async fn init<F>(
    Extension(key): Extension<UniKey>,
    State(svc): State<Arc<KafkaSender<TransactionCommand>>>,
    AxumCommand(com, lang, _): AxumCommand<InitPeriod, F>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let res = svc.apply(key, TransactionCommand::InitPeriod(com)).await;
    unis::into(res, &lang)
}

/// 打开交易期
#[utoipa::path(post, path = "/open", request_body = OpenPeriod)]
pub async fn open<F>(
    Extension(key): Extension<UniKey>,
    State(svc): State<Arc<KafkaSender<TransactionCommand>>>,
    AxumCommand(com, lang, _): AxumCommand<OpenPeriod, F>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let res = svc.apply(key, TransactionCommand::OpenPeriod(com)).await;
    unis::into(res, &lang)
}

/// 结转交易限额
#[utoipa::path(post, path = "/set_limit", request_body = SetLimit)]
pub async fn set_limit<F>(
    Extension(key): Extension<UniKey>,
    State(svc): State<Arc<KafkaSender<TransactionCommand>>>,
    AxumCommand(com, lang, _): AxumCommand<SetLimit, F>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let res = svc.apply(key, TransactionCommand::SetLimit(com)).await;
    unis::into(res, &lang)
}

/// 修改限额
#[utoipa::path(post, path = "/change_limit", request_body = ChangeLimit)]
pub async fn change_limit<F>(
    Extension(key): Extension<UniKey>,
    State(svc): State<Arc<KafkaSender<TransactionCommand>>>,
    AxumCommand(com, lang, _): AxumCommand<ChangeLimit, F>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let res = svc.apply(key, TransactionCommand::ChangeLimit(com)).await;
    unis::into(res, &lang)
}

/// 修改交易限额
#[utoipa::path(post, path = "/set_trans_limit", request_body = SetTransLimit)]
pub async fn set_trans_limit<F>(
    Extension(key): Extension<UniKey>,
    State(svc): State<Arc<KafkaSender<TransactionCommand>>>,
    AxumCommand(com, lang, _): AxumCommand<SetTransLimit, F>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let res = svc.apply(key, TransactionCommand::SetTransLimit(com)).await;
    unis::into(res, &lang)
}

/// 存款
#[utoipa::path(post, path = "/deposit", request_body = Deposit)]
pub async fn deposit<F>(
    Extension(key): Extension<UniKey>,
    State(svc): State<Arc<KafkaSender<TransactionCommand>>>,
    AxumCommand(com, lang, _): AxumCommand<Deposit, F>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let res = svc.apply(key, TransactionCommand::Deposit(com)).await;
    unis::into(res, &lang)
}

/// 取款
#[utoipa::path(post, path = "/withdraw", request_body = Withdraw)]
pub async fn withdraw<F>(
    Extension(key): Extension<UniKey>,
    State(svc): State<Arc<KafkaSender<TransactionCommand>>>,
    AxumCommand(com, lang, _): AxumCommand<Withdraw, F>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let res = svc.apply(key, TransactionCommand::Withdraw(com)).await;
    unis::into(res, &lang)
}

/// 转入
#[utoipa::path(post, path = "/transfer_in", request_body = TransferIn)]
pub async fn transfer_in<F>(
    Extension(key): Extension<UniKey>,
    State(svc): State<Arc<KafkaSender<TransactionCommand>>>,
    AxumCommand(com, lang, _): AxumCommand<TransferIn, F>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let res = svc.apply(key, TransactionCommand::TransferIn(com)).await;
    unis::into(res, &lang)
}

/// 转出
#[utoipa::path(post, path = "/transfer_out", request_body = TransferOut)]
pub async fn transfer_out<F>(
    Extension(key): Extension<UniKey>,
    State(svc): State<Arc<KafkaSender<TransactionCommand>>>,
    AxumCommand(com, lang, _): AxumCommand<TransferOut, F>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let res = svc.apply(key, TransactionCommand::TransferOut(com)).await;
    unis::into(res, &lang)
}

#[derive(OpenApi)]
#[openapi(paths(
    init,
    open,
    set_limit,
    change_limit,
    set_trans_limit,
    deposit,
    withdraw,
    transfer_in,
    transfer_out
))]
pub struct TransactionApi;

unis::route_builder!(
    transaction,
    KafkaSender<TransactionCommand>,
    [
        init,
        open,
        set_limit,
        change_limit,
        set_trans_limit,
        deposit,
        withdraw,
        transfer_out,
        transfer_in
    ]
);

pub async fn routes() -> Router {
    let ctx = unis::app::context().await;
    let svc = Arc::new(ctx.setup::<_, KafkaSender<TransactionCommand>>().await);
    Router::new()
        .nest("/rkyv/v1", rkyv_routes())
        .nest("/v1", json_routes())
        .with_state(svc)
}
