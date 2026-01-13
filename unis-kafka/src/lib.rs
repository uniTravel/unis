//! # **unis** 的 Kafka 实现
//!
//!
#![warn(missing_docs)]

mod config;

#[cfg(feature = "projector")]
pub mod projector;
#[cfg(feature = "sender")]
pub mod sender;
#[cfg(feature = "subscriber")]
pub mod subscriber;

use bincode::config::{Configuration, Fixint, Limit, LittleEndian};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::{
    sync::{Mutex, Notify},
    task::JoinSet,
};
use tracing::{error, info};

#[allow(dead_code)]
const BINCODE_HEADER: Configuration<LittleEndian, Fixint, Limit<4>> = bincode::config::standard()
    .with_fixed_int_encoding()
    .with_limit::<4>();

/// 应用上下文结构
pub struct Context {
    initiated: AtomicBool,
    tasks: Mutex<JoinSet<()>>,
    notify: Arc<Notify>,
}

impl Context {
    #[allow(dead_code)]
    fn new() -> Self {
        Self {
            initiated: AtomicBool::new(false),
            tasks: Mutex::new(JoinSet::new()),
            notify: Arc::new(Notify::new()),
        }
    }

    #[allow(dead_code)]
    async fn spawn<F, Fut>(&self, task: F)
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

    #[allow(dead_code)]
    async fn spawn_notify<F, Fut>(&self, task: F)
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

    #[allow(dead_code)]
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
}

#[allow(dead_code)]
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut sigint = match signal(SignalKind::interrupt()) {
            Ok(sigint) => sigint,
            Err(e) => {
                error!("监听 SIGINT 信号失败：{e}");
                panic!("监听 SIGINT 信号失败");
            }
        };
        let mut sigterm = match signal(SignalKind::terminate()) {
            Ok(sigterm) => sigterm,
            Err(e) => {
                error!("监听 SIGTERM 信号失败：{e}");
                panic!("监听 SIGTERM 信号失败");
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
