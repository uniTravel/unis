use super::*;
use domain::account::*;

#[derive(OpenApi)]
#[openapi(paths(create, verify, limit, approve))]
pub struct AccountApi;

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
