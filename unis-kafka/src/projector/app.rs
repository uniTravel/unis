//! Kafka 投影者上下文

use super::{CLOSED, CONFIG, EXIT, ProjectError, core::Projector};
use crate::config::load_name;
use std::{
    sync::{Mutex, atomic::Ordering},
    thread::sleep,
    time::Duration,
};
use tokio::sync::OnceCell;
use tracing::{error, info};
use unis::domain::Aggregate;

static CONTEXT: OnceCell<Mutex<App>> = OnceCell::const_new();
async fn app() -> &'static Mutex<App> {
    CONTEXT.get_or_init(App::new).await
}

/// 投影者上下文
pub async fn context() -> &'static Mutex<App> {
    tokio::spawn(async move {
        crate::shutdown_signal().await;
        let _ = EXIT.compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed);
    });
    app().await
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-utils"))]
pub async fn test_context() -> &'static Mutex<App> {
    app().await
}

/// 投影者上下文结构
pub struct App {
    projector: Projector,
}

impl App {
    async fn new() -> Mutex<Self> {
        Mutex::new(Self {
            projector: Projector::new(load_name(&CONFIG)),
        })
    }

    /// 订阅类型事件流
    pub fn subscribe<A>(&mut self)
    where
        A: Aggregate,
    {
        self.projector.subscribe::<A>();
    }

    /// 启动投影
    pub fn launch(&mut self) {
        let mins = 2;
        loop {
            match self.projector.launch() {
                Ok(()) => break,
                Err(e) => {
                    error!("投影处理错误：{:?}", e);
                    match e {
                        ProjectError::UniError(_) => break,
                        ProjectError::MetadataError => sleep(Duration::from_mins(mins)),
                        ProjectError::KafkaError(_) | ProjectError::ProducerNotFound => loop {
                            match self.projector.rebuild() {
                                Ok(()) => break,
                                Err(e) => {
                                    error!("重建生产者错误：{:?}", e);
                                    sleep(Duration::from_mins(mins));
                                }
                            }
                        },
                    }
                }
            }
        }
        let _ = CLOSED.compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed);
    }

    /// 等待投影任务优雅退出
    pub fn all_done() {
        while !CLOSED.load(Ordering::SeqCst) {
            sleep(Duration::from_millis(5));
        }
        info!("优雅退出投影任务");
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-utils"))]
    pub fn teardown() {
        let _ = EXIT.compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed);
        Self::all_done();
    }
}
