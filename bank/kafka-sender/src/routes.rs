mod account;
mod transaction;

use axum::Router;
use domain::{account::AccountCommand, transaction::TransactionCommand};
use std::sync::Arc;
use unis_kafka::sender::Sender;

pub fn account_routes() -> Router<Arc<Sender<AccountCommand>>> {
    Router::new()
        .nest("/api/v1/rkyv", account::rkyv_routes())
        .nest("/api/v1/json", account::json_routes())
}

pub fn transaction_routes() -> Router<Arc<Sender<TransactionCommand>>> {
    Router::new()
        .nest("/api/v1/rkyv", transaction::rkyv_routes())
        .nest("/api/v1/json", transaction::json_routes())
}
