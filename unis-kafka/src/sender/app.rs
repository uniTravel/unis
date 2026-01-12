//! Kafka 发送者上下文

use crate::{Context, sender::core::Sender};
use std::{ops::Deref, sync::Arc};
use tokio::sync::OnceCell;
use tracing::error;
use unis::domain::{Aggregate, CommandEnum};

static CONTEXT: OnceCell<Arc<App>> = OnceCell::const_new();
/// 发送者上下文
pub async fn context() -> Arc<App> {
    Arc::clone(
        CONTEXT
            .get_or_init(|| async {
                let app = App::new();
                let app_clone = Arc::clone(&app);
                tokio::spawn(async move {
                    crate::shutdown_signal().await;
                    app_clone.shutdown().await;
                });
                app
            })
            .await,
    )
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-utils"))]
pub async fn test_context() -> Arc<App> {
    App::new()
}

/// 发送者上下文结构
pub struct App {
    context: Context,
}

impl App {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            context: Context::new(),
        })
    }

    /// 设置特定聚合类型的发送者
    pub async fn setup<A, C>(self: &Arc<Self>) -> Sender<A, C>
    where
        A: Aggregate + Sync,
        C: CommandEnum<A = A> + Sync + 'static,
    {
        match Sender::<A, C>::new(Arc::clone(self)).await {
            Ok(sender) => sender,
            Err(e) => {
                error!(e);
                self.shutdown().await;
                self.all_done().await;
                panic!("异常退出发送者初始设置")
            }
        }
    }
}

impl Deref for App {
    type Target = Context;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}
