use crate::AppState;
use axum::extract::{Path, State};
use domain::account::{self, AccountCommand};
use std::sync::Arc;
use unis::{UniCommand, UniKey, UniResponse, domain::Request};

pub async fn create(
    Path(UniKey { agg_id, com_id }): Path<UniKey>,
    State(svc): State<Arc<AppState>>,
    UniCommand(com): UniCommand<account::CreateAccount>,
) -> UniResponse {
    svc.account
        .send(agg_id, com_id, AccountCommand::Create(com))
        .await
    // UniResponse::Success
}
