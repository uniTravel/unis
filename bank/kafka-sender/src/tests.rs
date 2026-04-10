mod account;
mod transaction;

use crate::routes;
use axum::{Router, body::to_bytes, http::StatusCode};
use proptest::{char, collection::vec, prelude::*, strategy::ValueTree, test_runner::TestRunner};
use rstest::{fixture, rstest};
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{Level, error};
use tracing_appender::non_blocking;
use tracing_subscriber::fmt;
use unis::{
    app::{self, Context},
    sender::{change, create},
};
use unis_kafka::sender::KafkaSender;
use uuid::Uuid;

#[fixture]
#[once]
fn setup() {
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
            error!("获取环境变量 'NEXTEST_TEST_NAME' 失败：{e}");
            panic!("需用 cargo nextest 执行测试！");
        }
    }
}

#[fixture]
fn ctx() -> &'static Context {
    app::test_context()
}

fn digit_string(lenth: usize) -> impl Strategy<Value = String> {
    vec(b'0'..=b'9', lenth).prop_map(|bytes| String::from_utf8(bytes).unwrap())
}

fn long_string(ge: usize) -> impl Strategy<Value = String> {
    vec(char::any(), ge..=50).prop_map(|chars| chars.into_iter().collect())
}
