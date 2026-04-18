//! # **unis** 应用上下文
#![allow(unused_imports)]

use crate::domain::CommandEnum;
use ahash::{AHashSet, RandomState};
use dashmap::DashMap;
use rkyv::{
    Archive, Deserialize,
    de::Pool,
    rancor::{Error, Strategy},
};
use std::{
    any::TypeId,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::OnceCell;
use tokio::{
    sync::{Mutex, Notify},
    task::JoinSet,
};
use tracing::{error, info};
use uuid::Uuid;

/// 应用上下文
pub async fn context() -> &'static Context {
    static CONTEXT: OnceCell<Arc<Context>> = OnceCell::const_new();
    CONTEXT
        .get_or_init(|| async {
            let ctx = Arc::new(Context::new());
            let ctx_clone = Arc::clone(&ctx);
            tokio::spawn(async move {
                shutdown_signal().await;
                ctx_clone.shutdown().await;
            });
            ctx
        })
        .await
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-utils"))]
pub fn test_context() -> &'static Context {
    use std::sync::LazyLock;

    static CONTEXT: LazyLock<Context> = LazyLock::new(|| Context::new());
    &CONTEXT
}

/// 应用上下文结构
pub struct Context {
    initiated: AtomicBool,
    tasks: Mutex<JoinSet<()>>,
    notify: Arc<Notify>,
    #[cfg(feature = "subscriber")]
    subscriber_types: Mutex<AHashSet<TypeId>>,
    #[cfg(feature = "sender")]
    sender_types: Mutex<AHashSet<TypeId>>,
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-utils"))]
    agg_ids: Arc<DashMap<Uuid, Uuid, RandomState>>,
}

impl Context {
    fn new() -> Self {
        Self {
            initiated: AtomicBool::new(false),
            tasks: Mutex::new(JoinSet::new()),
            notify: Arc::new(Notify::new()),
            #[cfg(feature = "subscriber")]
            subscriber_types: Mutex::new(AHashSet::new()),
            #[cfg(feature = "sender")]
            sender_types: Mutex::new(AHashSet::new()),
            #[cfg(any(test, feature = "test-utils"))]
            agg_ids: Arc::new(DashMap::with_hasher(RandomState::new())),
        }
    }

    /// 启动特定聚合类型的订阅者
    #[cfg(feature = "subscriber")]
    pub async fn launch<C, S>(&'static self)
    where
        C: CommandEnum + Sync,
        <C as Archive>::Archived: Deserialize<C, Strategy<Pool, Error>>,
        <C::E as Archive>::Archived: rkyv::Deserialize<C::E, Strategy<Pool, Error>>,
        S: crate::subscriber::Subscriber<C::A, C, C::E>,
    {
        let mut types = self.subscriber_types.lock().await;
        if types.insert(TypeId::of::<C>()) {
            if let Err(e) = S::launch(self).await {
                error!(e);
                self.shutdown().await;
                self.all_done().await;
                panic!("异常退出订阅者初始设置")
            }
        } else {
            let type_name = std::any::type_name::<C>();
            error!("重复启动 {type_name} 的订阅者");
            self.shutdown().await;
            self.all_done().await;
            panic!("特定聚合类型的订阅者只能启动一次");
        }
    }

    /// 初始设置特定聚合类型的发送者
    #[cfg(feature = "sender")]
    pub async fn setup<C, S>(&'static self) -> S
    where
        C: CommandEnum + Sync,
        <C as Archive>::Archived: Deserialize<C, Strategy<Pool, Error>>,
        <C::E as Archive>::Archived: rkyv::Deserialize<C::E, Strategy<Pool, Error>>,
        S: crate::sender::Sender<C::A, C, C::E>,
    {
        let mut types = self.sender_types.lock().await;
        if types.insert(TypeId::of::<C>()) {
            match S::new(self).await {
                Ok(sender) => sender,
                Err(e) => {
                    error!(e);
                    self.shutdown().await;
                    self.all_done().await;
                    panic!("异常退出发送者初始设置")
                }
            }
        } else {
            let type_name = std::any::type_name::<C>();
            error!("重复初始设置 {type_name} 的发送者");
            self.shutdown().await;
            self.all_done().await;
            panic!("特定聚合类型的发送者只能初始设置一次");
        }
    }

    /// 启用无需通知优雅退出的异步任务
    pub async fn spawn<F, Fut>(&self, task: F)
    where
        F: FnOnce(Arc<Notify>) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let mut tasks = self.tasks.lock().await;
        let ready = Arc::new(Notify::new());
        let waiter = Arc::clone(&ready);
        tasks.spawn(task(ready));
        waiter.notified().await;
    }

    /// 启用需要通知优雅退出的异步任务
    pub async fn spawn_notify<F, Fut>(&self, task: F)
    where
        F: FnOnce(Arc<Notify>, Arc<Notify>) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let mut tasks = self.tasks.lock().await;
        let ready = Arc::new(Notify::new());
        let notify = Arc::clone(&self.notify);
        let waiter = Arc::clone(&ready);
        tasks.spawn(task(ready, notify));
        waiter.notified().await;
    }

    async fn shutdown(&self) {
        if self
            .initiated
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
        {
            info!("开始优雅退出");
            self.notify.notify_waiters();
        }
    }

    /// 等待所有后台任务优雅退出
    pub async fn all_done(&self) {
        let mut tasks = self.tasks.lock().await;
        while let Some(result) = tasks.join_next().await {
            if let Err(e) = result {
                error!("后台任务发生错误：{e}");
            }
        }
        info!("优雅退出所有后台任务");
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-utils"))]
    pub async fn teardown(&self) {
        self.shutdown().await;
        self.all_done().await;
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub(crate) fn insert(&self, com_id: Uuid, agg_id: Uuid) -> Option<Uuid> {
        self.agg_ids.insert(com_id, agg_id)
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-utils"))]
    pub fn remove(&self, com_id: &Uuid) -> Option<(Uuid, Uuid)> {
        self.agg_ids.remove(com_id)
    }
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut sigint = match signal(SignalKind::interrupt()) {
            Ok(sigint) => sigint,
            Err(e) => {
                panic!("监听 SIGINT 信号失败：{e}");
            }
        };
        let mut sigterm = match signal(SignalKind::terminate()) {
            Ok(sigterm) => sigterm,
            Err(e) => {
                panic!("监听 SIGTERM 信号失败：{e}");
            }
        };
        tokio::select! {
            _ = sigint.recv() => info!("收到 SIGINT 信号"),
            _ = sigterm.recv() => info!("收到 SIGTERM 信号"),
        }
    }
    #[cfg(not(unix))]
    {
        use tracing::{error, info};

        if let Err(e) = tokio::signal::ctrl_c().await {
            error!("监听 Ctrl-C 信号失败: {e}");
            return;
        }
        info!("收到 Ctrl-C 信号");
    }
}
