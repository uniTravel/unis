use super::*;
use domain::transaction::*;

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

/// 初始化交易期
///
/// * 账户设立时执行一次，后续都是打开交易期。
#[utoipa::path(post, path = "/init", request_body = InitPeriod)]
pub async fn init<F>(
    Extension(UniKey { agg_id, com_id }): Extension<UniKey>,
    State(svc): State<Arc<KafkaSender<TransactionCommand>>>,
    AxumCommand(com, lang, _): AxumCommand<InitPeriod, F>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let res = svc
        .apply(agg_id, com_id, TransactionCommand::InitPeriod(com))
        .await;
    unis::into(res, &lang)
}

/// 打开交易期
///
/// * 每个月末打开新的交易期，待结转交易限额后方可交易。
#[utoipa::path(post, path = "/open", request_body = OpenPeriod)]
pub async fn open<F>(
    Extension(UniKey { agg_id, com_id }): Extension<UniKey>,
    State(svc): State<Arc<KafkaSender<TransactionCommand>>>,
    AxumCommand(com, lang, _): AxumCommand<OpenPeriod, F>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let res = svc
        .apply(agg_id, com_id, TransactionCommand::OpenPeriod(com))
        .await;
    unis::into(res, &lang)
}

/// 结转交易限额
///
/// > **⚠️ 注意**
/// * 交易限额不得大于账户限额。
#[utoipa::path(post, path = "/set_limit", request_body = SetLimit)]
pub async fn set_limit<F>(
    Extension(UniKey { agg_id, com_id }): Extension<UniKey>,
    State(svc): State<Arc<KafkaSender<TransactionCommand>>>,
    AxumCommand(com, lang, _): AxumCommand<SetLimit, F>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let res = svc
        .apply(agg_id, com_id, TransactionCommand::SetLimit(com))
        .await;
    unis::into(res, &lang)
}

/// 修改限额
#[utoipa::path(post, path = "/change_limit", request_body = ChangeLimit)]
pub async fn change_limit<F>(
    Extension(UniKey { agg_id, com_id }): Extension<UniKey>,
    State(svc): State<Arc<KafkaSender<TransactionCommand>>>,
    AxumCommand(com, lang, _): AxumCommand<ChangeLimit, F>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let res = svc
        .apply(agg_id, com_id, TransactionCommand::ChangeLimit(com))
        .await;
    unis::into(res, &lang)
}

/// 修改交易限额
#[utoipa::path(post, path = "/set_trans_limit", request_body = SetTransLimit)]
pub async fn set_trans_limit<F>(
    Extension(UniKey { agg_id, com_id }): Extension<UniKey>,
    State(svc): State<Arc<KafkaSender<TransactionCommand>>>,
    AxumCommand(com, lang, _): AxumCommand<SetTransLimit, F>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let res = svc
        .apply(agg_id, com_id, TransactionCommand::SetTransLimit(com))
        .await;
    unis::into(res, &lang)
}

/// 存款
#[utoipa::path(post, path = "/deposit", request_body = Deposit)]
pub async fn deposit<F>(
    Extension(UniKey { agg_id, com_id }): Extension<UniKey>,
    State(svc): State<Arc<KafkaSender<TransactionCommand>>>,
    AxumCommand(com, lang, _): AxumCommand<Deposit, F>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let res = svc
        .apply(agg_id, com_id, TransactionCommand::Deposit(com))
        .await;
    unis::into(res, &lang)
}

/// 取款
#[utoipa::path(post, path = "/withdraw", request_body = Withdraw)]
pub async fn withdraw<F>(
    Extension(UniKey { agg_id, com_id }): Extension<UniKey>,
    State(svc): State<Arc<KafkaSender<TransactionCommand>>>,
    AxumCommand(com, lang, _): AxumCommand<Withdraw, F>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let res = svc
        .apply(agg_id, com_id, TransactionCommand::Withdraw(com))
        .await;
    unis::into(res, &lang)
}

/// 转入
#[utoipa::path(post, path = "/transfer_in", request_body = TransferIn)]
pub async fn transfer_in<F>(
    Extension(UniKey { agg_id, com_id }): Extension<UniKey>,
    State(svc): State<Arc<KafkaSender<TransactionCommand>>>,
    AxumCommand(com, lang, _): AxumCommand<TransferIn, F>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let res = svc
        .apply(agg_id, com_id, TransactionCommand::TransferIn(com))
        .await;
    unis::into(res, &lang)
}

/// 转出
#[utoipa::path(post, path = "/transfer_out", request_body = TransferOut)]
pub async fn transfer_out<F>(
    Extension(UniKey { agg_id, com_id }): Extension<UniKey>,
    State(svc): State<Arc<KafkaSender<TransactionCommand>>>,
    AxumCommand(com, lang, _): AxumCommand<TransferOut, F>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let res = svc
        .apply(agg_id, com_id, TransactionCommand::TransferOut(com))
        .await;
    unis::into(res, &lang)
}
