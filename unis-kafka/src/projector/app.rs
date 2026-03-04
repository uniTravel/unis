//! Kafka 投影者上下文

use super::core::launch;
use crate::Context;
use std::{ops::Deref, sync::LazyLock};

static CONTEXT: LazyLock<App> = LazyLock::new(|| App::new());
fn app() -> &'static App {
    &CONTEXT
}

/// 投影者上下文
pub async fn context() -> &'static App {
    tokio::spawn(async move {
        crate::shutdown_signal().await;
        app().shutdown().await;
    });
    app()
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-utils"))]
pub async fn test_context() -> &'static App {
    app()
}

/// 投影者上下文结构
pub struct App {
    context: Context,
}

impl App {
    fn new() -> Self {
        Self {
            context: Context::new(),
        }
    }

    /// 启动投影
    pub async fn launch(&self, topics: Vec<&'static str>) {
        self.spawn_notify(move |ready, notify| launch(topics, ready, notify))
            .await;
    }
}

impl Deref for App {
    type Target = Context;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}
