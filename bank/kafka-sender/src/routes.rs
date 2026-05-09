mod account;
mod transaction;

use axum::Router;
use domain::{account::AccountCommand, transaction::TransactionCommand};
use std::sync::Arc;
use unis_kafka::sender::KafkaSender;

pub fn account_routes() -> Router<Arc<KafkaSender<AccountCommand>>> {
    Router::new()
        .nest("/rkyv/v1", account::rkyv_routes())
        .nest("/v1", account::json_routes())
}

pub fn transaction_routes() -> Router<Arc<KafkaSender<TransactionCommand>>> {
    Router::new()
        .nest("/rkyv/v1", transaction::rkyv_routes())
        .nest("/v1", transaction::json_routes())
}
