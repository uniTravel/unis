//! # **unis** 后台任务

use std::{
    ops::Not,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::{
    sync::{Mutex, Notify, OnceCell, RwLock},
    task::JoinSet,
};
use tracing::{error, info};

static INITIATED: AtomicBool = AtomicBool::new(false);

static WORKER: OnceCell<Worker> = OnceCell::const_new();
/// 获取静态的后台任务管理器
// pub async fn worker() -> &'static Worker {
//     WORKER
//         .get_or_init(|| async {
//             let worker = Arc::new(Worker::new());
//             let worker_clone = Arc::clone(&worker);
//             tokio::spawn(async move {
//                 match tokio::signal::ctrl_c().await {
//                     Ok(_) => info!("收到 Ctrl-C 信号"),
//                     Err(e) => {
//                         error!("监听 Ctrl-C 信号失败: {e}");
//                         info!("启用备用关闭机制");
//                     }
//                 }
//                 worker_clone.shutdown().await;
//             });
//             Arc::clone(&worker)
//         })
//         .await
// }

tokio::task_local! {
    static TASKS: Arc<RwLock<JoinSet<()>>>;
    static NOTIFY: Arc<Notify>;
}

// fn init() {
//         CURRENT_RUNTIME_JOINSET
//             .scope(Arc::new(RwLock::new(JoinSet::new())), || {})
//             .unwrap();
//     }



// /// 后台任务管理器
// pub struct Worker;

// impl Worker {

//     /// 启动后台任务
//     pub async fn spawn<F>(&self, task: F) -> Arc<Notify>
//     where
//         F: Future<Output = ()> + Send + 'static,
//     {
//         let mut tasks = self.tasks.lock().await;
//         tasks.spawn(task);
//         Arc::clone(&self.notify)
//     }

//     pub(crate) async fn shutdown(&self) {
//         if INITIATED
//             .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
//             .is_ok()
//         {
//             info!("开始优雅退出");
//             let mut tasks = self.tasks.lock().await;
//             while let Some(result) = tasks.join_next().await {
//                 if let Err(e) = result {
//                     error!("后台任务发生错误：{e}");
//                 }
//             }
//             info!("优雅退出所有后台任务");
//         }
//     }
// }
