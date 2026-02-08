use axum::extract::{Path, State};
use domain::account::{self, AccountCommand};
use std::sync::Arc;
use unis_kafka::sender::{Request, Sender, UniCommand, UniKey, UniResponse};

pub async fn create<F>(
    Path(UniKey { agg_id, com_id }): Path<UniKey>,
    State(svc): State<Arc<Sender<AccountCommand>>>,
    UniCommand(com, _): UniCommand<account::CreateAccount, F>,
) -> UniResponse {
    svc.send(agg_id, com_id, AccountCommand::Create(com)).await
}

pub async fn verify<F>(
    Path(UniKey { agg_id, com_id }): Path<UniKey>,
    State(svc): State<Arc<Sender<AccountCommand>>>,
    UniCommand(com, _): UniCommand<account::VerifyAccount, F>,
) -> UniResponse {
    svc.send(agg_id, com_id, AccountCommand::Verify(com)).await
}
