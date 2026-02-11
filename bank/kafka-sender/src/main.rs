mod handlers;
mod routes;

use axum::Router;
use domain::{account::AccountCommand, transaction::TransactionCommand};
use std::sync::Arc;
use tracing_appender::non_blocking;
use tracing_subscriber::fmt;
use unis_kafka::sender;

#[tokio::main]
async fn main() {
    let (non_blocking, _guard) = non_blocking(std::io::stdout());
    fmt()
        .with_writer(non_blocking)
        .with_target(false)
        .pretty()
        .init();

    let ctx = sender::context().await;
    let svc_account = Arc::new(ctx.setup::<AccountCommand>().await);
    let svc_transaction = Arc::new(ctx.setup::<TransactionCommand>().await);
    let app = Router::new()
        .merge(routes::account_routes().with_state(svc_account))
        .merge(routes::transaction_routes().with_state(svc_transaction));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    let _ = axum::serve(listener, app)
        .with_graceful_shutdown(ctx.all_done())
        .await;
}
