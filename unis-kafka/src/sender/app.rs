//! Kafka 发送者上下文

use crate::Context;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tracing::{error, info};

static CONTEXT: OnceCell<Arc<Context>> = OnceCell::const_new();
/// 发送者上下文
pub async fn context() -> Arc<Context> {
    Arc::clone(
        CONTEXT
            .get_or_init(|| async {
                let app = Arc::new(Context::new());
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
pub async fn test_context() -> Arc<Context> {
    Arc::new(Context::new())
}
