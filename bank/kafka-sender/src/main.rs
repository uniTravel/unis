mod handlers;
mod routes;
#[cfg(test)]
mod tests;

use axum::Router;
use domain::{account::AccountCommand, transaction::TransactionCommand};
use handlers::{account, transaction};
use std::sync::Arc;
use tracing_appender::non_blocking;
use tracing_subscriber::fmt;
use unis::app;
use unis_kafka::sender::KafkaSender;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};
// use utoipa_redoc::{Redoc, Servable};

#[tokio::main]
async fn main() {
    let (non_blocking, _guard) = non_blocking(std::io::stdout());
    fmt()
        .with_writer(non_blocking)
        .with_target(false)
        .pretty()
        .init();

    let ctx = app::context().await;
    let svc_account = Arc::new(ctx.setup::<_, KafkaSender<AccountCommand>>().await);
    let svc_transaction = Arc::new(ctx.setup::<_, KafkaSender<TransactionCommand>>().await);
    let app = Router::new()
        .merge(Scalar::with_url("/", ApiDoc::openapi()))
        // .merge(Redoc::with_url("/", ApiDoc::openapi()))
        .merge(routes::account_routes().with_state(svc_account))
        .merge(routes::transaction_routes().with_state(svc_transaction));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    let _ = axum::serve(listener, app)
        .with_graceful_shutdown(ctx.all_done())
        .await;
}

#[derive(OpenApi)]
#[openapi(
    info(title = "账户管理"),
    nest(
        (path = "/v1/account", api = account::AccountApi),
        (path = "/v1/transaction", api = transaction::TransactionApi)
    )
)]
struct ApiDoc;
