mod account;
mod transaction;

use crate::routes;
use axum::{Router, body::Bytes, http::StatusCode};
use domain::tests::*;
use proptest::{prelude::*, strategy::ValueTree, test_runner::TestRunner};
use proptest_state_machine::ReferenceStateMachine;
use rstest::{fixture, rstest};
use std::sync::{Arc, LazyLock};
use tokio::sync::OnceCell;
use tracing::Level;
use tracing_appender::non_blocking;
use tracing_subscriber::fmt;
use unis::{
    app::{self, Context},
    domain::{Aggregate, Event, EventEnum},
    sender::apply,
};
use unis_kafka::sender::KafkaSender;
use uuid::Uuid;

static SETUP: LazyLock<()> = LazyLock::new(|| {
    let (non_blocking, _guard) = non_blocking(std::io::stdout());
    fmt()
        .with_max_level(Level::DEBUG)
        .with_writer(non_blocking)
        .with_target(false)
        .pretty()
        .with_test_writer()
        .init();
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
