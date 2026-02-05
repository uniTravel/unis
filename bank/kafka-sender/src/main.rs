mod v1;

mod account;
mod transaction;

use std::sync::Arc;

use axum::Router;
use domain::{
    account::{Account, AccountCommand, AccountEvent},
    transaction::{Transaction, TransactionCommand, TransactionEvent},
};
use tracing_appender::non_blocking;
use tracing_subscriber::fmt;
use unis_kafka::sender::{self, Sender};

#[tokio::main]
async fn main() {
    let (non_blocking, _guard) = non_blocking(std::io::stdout());
    fmt()
        .with_writer(non_blocking)
        .with_target(false)
        .pretty()
        .init();

    let state = Arc::new(AppState {
        account: Arc::new(sender::context().await.setup::<AccountCommand>().await),
        transaction: Arc::new(sender::context().await.setup::<TransactionCommand>().await),
    });

    let app = Router::new().nest("/v1", v1::routes()).with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    let _ = axum::serve(listener, app).await;
}

struct AppState {
    account: Arc<Sender<Account, AccountCommand, AccountEvent>>,
    transaction: Arc<Sender<Transaction, TransactionCommand, TransactionEvent>>,
}
