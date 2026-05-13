#[cfg(test)]
mod tests;

mod account;
mod transaction;

use axum::{Router, http::StatusCode, routing::get};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::LogExporter;
use opentelemetry_sdk::{Resource, logs::SdkLoggerProvider};
use std::sync::OnceLock;
use tracing_appender::non_blocking;
use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt, util::SubscriberInitExt};
use unis::app;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

fn get_resource() -> Resource {
    static RESOURCE: OnceLock<Resource> = OnceLock::new();
    RESOURCE
        .get_or_init(|| Resource::builder().with_service_name("bank-sender").build())
        .clone()
}

fn init_logs() -> SdkLoggerProvider {
    let exporter = LogExporter::builder().build().expect("创建日志导出器失败");
    SdkLoggerProvider::builder()
        .with_resource(get_resource())
        .with_batch_exporter(exporter)
        .build()
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

#[tokio::main]
async fn main() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let (non_blocking, _guard) = non_blocking(std::io::stdout());
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_target(false)
        .pretty();
    let logger_provider = init_logs();
    let otel_layer = OpenTelemetryTracingBridge::new(&logger_provider);
    Registry::default()
        .with(env_filter)
        .with(fmt_layer)
        .with(otel_layer)
        .init();

    let app = Router::new()
        .route("/health", get(|| async { StatusCode::OK }))
        .merge(account::routes().await)
        .merge(transaction::routes().await)
        .merge(Scalar::with_url("/", ApiDoc::openapi()));

    let ctx = app::context().await;
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    let _ = axum::serve(listener, app)
        .with_graceful_shutdown(ctx.all_done())
        .await;
}
