#[cfg(test)]
mod tests;

mod account;
mod transaction;

use axum::Router;
use tracing_appender::non_blocking;
use tracing_subscriber::fmt;
use unis::app;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

#[tokio::main]
async fn main() {
    let (non_blocking, _guard) = non_blocking(std::io::stdout());
    fmt()
        .with_writer(non_blocking)
        .with_target(false)
        .pretty()
        .init();

    let app = Router::new()
        .merge(account::routes().await)
        .merge(transaction::routes().await)
        .merge(Scalar::with_url("/", ApiDoc::openapi()));

    let ctx = app::context().await;
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
