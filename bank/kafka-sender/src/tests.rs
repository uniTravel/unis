mod account;
mod transaction;

use crate::routes;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use rkyv::rancor::Error;
use rstest::{fixture, rstest};
use std::sync::Arc;
use tokio::sync::OnceCell;
use tower::ServiceExt;
use tracing::Level;
use tracing_appender::non_blocking;
use tracing_subscriber::fmt;
use unis_kafka::sender::{App, test_context};
use uuid::Uuid;

static SETUP: OnceCell<()> = OnceCell::const_new();
#[fixture]
async fn setup() {
    SETUP
        .get_or_init(|| async {
            let (non_blocking, _guard) = non_blocking(std::io::stdout());
            fmt()
                .with_max_level(Level::DEBUG)
                .with_writer(non_blocking)
                .with_target(false)
                .pretty()
                .with_test_writer()
                .init();
        })
        .await;
}

#[fixture]
async fn ctx() -> &'static App {
    test_context().await
}
