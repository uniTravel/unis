use axum::{Router, http::StatusCode, routing::get};
use opentelemetry::trace::TracerProvider;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{LogExporter, SpanExporter};
use opentelemetry_sdk::{
    Resource,
    logs::SdkLoggerProvider,
    trace::{Sampler, SdkTracerProvider},
};
use std::sync::OnceLock;
use tracing_appender::non_blocking;
use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt};
use unis_kafka::projector::{self, Topic};

fn get_resource() -> Resource {
    static RESOURCE: OnceLock<Resource> = OnceLock::new();
    RESOURCE
        .get_or_init(|| Resource::builder().with_service_name("bank").build())
        .clone()
}

fn init_logger() -> SdkLoggerProvider {
    let exporter = LogExporter::builder().build().expect("创建日志导出器失败");
    SdkLoggerProvider::builder()
        .with_resource(get_resource())
        .with_batch_exporter(exporter)
        .build()
}
// TODO：优化采样策略
fn init_tracer() -> SdkTracerProvider {
    let exporter = SpanExporter::builder().build().expect("创建追踪导出器失败");
    SdkTracerProvider::builder()
        .with_resource(get_resource())
        .with_sampler(Sampler::AlwaysOn)
        .with_batch_exporter(exporter)
        .build()
}

#[tokio::main]
async fn main() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let (non_blocking, _guard) = non_blocking(std::io::stdout());
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_target(false)
        .pretty();
    let logger_provider = init_logger();
    let logger_layer = OpenTelemetryTracingBridge::new(&logger_provider);
    let tracer_provider = init_tracer();
    let tracer_layer = tracing_opentelemetry::layer::<Registry>()
        .with_tracer(tracer_provider.tracer("bank-projector"));
    let subscriber = Registry::default()
        .with(tracer_layer)
        .with(logger_layer)
        .with(env_filter)
        .with(fmt_layer);
    tracing::subscriber::set_global_default(subscriber).expect("设置全局追踪订阅者失败");

    let app = Router::new().route("/health", get(|| async { StatusCode::OK }));

    let ctx = projector::context().await;
    projector::launch(
        ctx,
        vec![domain::Account::topic(), domain::Transaction::topic()],
    )
    .await;
    let listener = tokio::net::TcpListener::bind("0.0.0.0:7002").await.unwrap();
    let _ = axum::serve(listener, app)
        .with_graceful_shutdown(ctx.all_done())
        .await;
    let _ = logger_provider.shutdown();
    let _ = tracer_provider.shutdown();
}
