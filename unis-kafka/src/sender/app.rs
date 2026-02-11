use crate::{Context, sender::core::Sender};
use rkyv::{
    Archive, Deserialize,
    de::Pool,
    rancor::{Error, Strategy},
};
use std::ops::Deref;
use tokio::sync::OnceCell;
use tracing::error;
use unis::domain::CommandEnum;

static CONTEXT: OnceCell<App> = OnceCell::const_new();
pub(super) async fn app() -> &'static App {
    CONTEXT.get_or_init(App::new).await
}

/// 发送者上下文
pub async fn context() -> &'static App {
    tokio::spawn(async move {
        crate::shutdown_signal().await;
        app().await.shutdown().await;
    });
    app().await
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-utils"))]
pub async fn test_context() -> &'static App {
    app().await
}

/// 发送者上下文结构
pub struct App {
    context: Context,
}

impl App {
    async fn new() -> Self {
        Self {
            context: Context::new(),
        }
    }

    /// 设置特定聚合类型的发送者
    pub async fn setup<C>(&self) -> Sender<C>
    where
        C: CommandEnum + Sync,
        <C as Archive>::Archived: Deserialize<C, Strategy<Pool, Error>>,
        <C::E as Archive>::Archived: rkyv::Deserialize<C::E, Strategy<Pool, Error>>,
    {
        match Sender::new().await {
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
