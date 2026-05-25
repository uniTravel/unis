mod account;
mod transaction;

use crate::{init_logger, init_tracer};
use axum::{Router, body::Bytes, http::StatusCode};
use domain::tests::*;
use opentelemetry::trace::TracerProvider;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use proptest::{prelude::*, strategy::ValueTree, test_runner::TestRunner};
use proptest_state_machine::ReferenceStateMachine;
use rstest::{fixture, rstest};
use std::sync::LazyLock;
use tokio::sync::OnceCell;
use tracing_appender::non_blocking;
use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt};
use unis::{
    app::{self, Context},
    apply,
    domain::{Aggregate, Event, EventEnum},
};
use uuid::Uuid;
// TODO：解决测试不会追踪的问题
static SETUP: LazyLock<()> = LazyLock::new(|| {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"));
    let (non_blocking, _guard) = non_blocking(std::io::stdout());
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_target(false)
        .pretty()
        .with_test_writer();
    let logger_provider = init_logger();
    let logger_layer = OpenTelemetryTracingBridge::new(&logger_provider);
    let tracer_provider = init_tracer();
    let tracer_layer = tracing_opentelemetry::layer::<Registry>()
        .with_tracer(tracer_provider.tracer("bank-sender"));
    let subscriber = Registry::default()
        .with(tracer_layer)
        .with(logger_layer)
        .with(env_filter)
        .with(fmt_layer);
    tracing::subscriber::set_global_default(subscriber).expect("设置全局追踪订阅者失败");

    match std::env::var("NEXTEST_TEST_NAME") {
        Ok(test_name) => {
            let value = test_name.rsplit("::").next().unwrap();
            unsafe {
                std::env::set_var("UNI__HOSTNAME", value);
            }
        }
        Err(e) => {
            tracing::error!("获取环境变量 'NEXTEST_TEST_NAME' 失败：{e}");
            panic!("需用 cargo nextest 执行测试！");
        }
    }
});

#[fixture]
fn ctx() -> &'static Context {
    app::test_context()
}
