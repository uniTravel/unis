//! Kafka 发送者上下文

use crate::{Context, sender::core::Sender};
use std::{ops::Deref, sync::Arc};
use tokio::sync::OnceCell;
use tracing::{error, info};
use unis::domain::{Aggregate, CommandEnum};

static CONTEXT: OnceCell<Arc<App>> = OnceCell::const_new();
/// 发送者上下文
pub async fn context() -> Arc<App> {
    Arc::clone(
        CONTEXT
            .get_or_init(|| async {
                let app = Arc::new(App::new());
                let app_clone = Arc::clone(&app);
                tokio::spawn(async move {
                    match tokio::signal::ctrl_c().await {
                        Ok(_) => info!("收到 Ctrl-C 信号"),
                        Err(e) => {
                            error!("监听 Ctrl-C 信号失败: {e}");
                            info!("启用备用关闭机制");
                        }
                    }
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
    Arc::new(App::new())
}

/// 发送者上下文结构
pub struct App {
    context: Context,
}

impl App {
    fn new() -> Self {
        Self {
            context: Context::new(),
        }
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
                panic!("异常退出订阅者初始设置")
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
