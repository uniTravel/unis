use std::sync::Arc;

use crate::{AppState, account};
use axum::{Router, routing::post};

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .nest("/account", account_routes())
        .nest("/transaction", transaction_routes())
}

fn account_routes() -> Router<Arc<AppState>> {
    Router::new().route("/create/:agg_id/:com_id", post(account::create))
}

fn transaction_routes() -> Router<Arc<AppState>> {
    Router::new()
}
