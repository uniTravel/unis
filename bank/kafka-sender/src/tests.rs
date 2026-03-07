mod account;
mod transaction;

use crate::routes;
use axum::{
    Router,
    body::{Body, Bytes, to_bytes},
    http::{Request, StatusCode},
};
use rkyv::rancor::Error;
use rstest::{fixture, rstest};
use std::sync::Arc;
use tower::ServiceExt;
use tracing::{Level, error};
use tracing_appender::non_blocking;
use tracing_subscriber::fmt;
use unis::{
    app::{self, Context},
    sender::{change, create},
};
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
