use axum::{Router, http::StatusCode, routing::get};
use tracing_appender::non_blocking;
use tracing_subscriber::fmt;
use unis_kafka::projector::{self, Topic};

#[tokio::main]
async fn main() {
    let (non_blocking, _guard) = non_blocking(std::io::stdout());
    fmt()
        .with_writer(non_blocking)
        .with_target(false)
        .pretty()
        .init();

    let app = Router::new().route("/health", get(|| async { StatusCode::OK }));

    let ctx = projector::context().await;
    projector::launch(
        ctx,
        vec![domain::Account::topic(), domain::Transaction::topic()],
    )
    .await;
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3002").await.unwrap();
    let _ = axum::serve(listener, app)
        .with_graceful_shutdown(ctx.all_done())
        .await;
}
