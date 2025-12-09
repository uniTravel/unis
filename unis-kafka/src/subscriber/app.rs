//! Kafka 订阅者上下文

use crate::{
    config::load_bootstrap,
    subscriber::{SUBSCRIBER_CONFIG, TopicTask},
};
use rdkafka::{
    ClientConfig,
    admin::{AdminClient, AdminOptions, NewTopic, TopicReplication},
    client::DefaultClientContext,
};
use std::{
    path::PathBuf,
    sync::{
        Arc, LazyLock,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::{
    sync::{Mutex, Notify, OnceCell, mpsc},
    task::JoinSet,
    time::{Duration, Instant},
};
use tracing::{debug, debug_span, error, info};
use unis::config::build_config;

static ADMIN: LazyLock<AdminClient<DefaultClientContext>> = LazyLock::new(|| {
    let config = build_config(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    let bootstrap = load_bootstrap(&config);
    ClientConfig::new()
        .set("bootstrap.servers", bootstrap)
        .create()
        .expect("管理客户端创建失败")
});

static OPTS: LazyLock<AdminOptions> = LazyLock::new(|| {
    AdminOptions::new()
        .operation_timeout(Some(Duration::from_secs(3)))
        .request_timeout(Some(Duration::from_secs(5)))
});

static APP: OnceCell<Arc<App>> = OnceCell::const_new();
/// 订阅者上下文
pub async fn app() -> Arc<App> {
    Arc::clone(
        APP.get_or_init(|| async {
            LazyLock::force(&ADMIN);
            LazyLock::force(&OPTS);
            let (topic_tx, topic_rx) = mpsc::unbounded_channel::<TopicTask>();
            let app = Arc::new(App::new(topic_tx));
            app.spawn(|ready, notify| topic_creator(topic_rx, ready, notify))
                .await;
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

/// 测试专用订阅者上下文
#[cfg(any(test, feature = "test-utils"))]
pub async fn test_context() -> Arc<App> {
    LazyLock::force(&ADMIN);
    LazyLock::force(&OPTS);
    let (topic_tx, topic_rx) = mpsc::unbounded_channel::<TopicTask>();
    let app = Arc::new(App::new(topic_tx));
    app.spawn(|ready, notify| topic_creator(topic_rx, ready, notify))
        .await;
    app
}

/// 订阅者上下文结构
pub struct App {
    initiated: AtomicBool,
    topic_tx: mpsc::UnboundedSender<TopicTask>,
    tasks: Mutex<JoinSet<()>>,
    notify: Arc<Notify>,
}

impl App {
    fn new(topic_tx: mpsc::UnboundedSender<TopicTask>) -> Self {
        Self {
            initiated: AtomicBool::new(false),
            topic_tx,
            tasks: Mutex::new(JoinSet::new()),
            notify: Arc::new(Notify::new()),
        }
    }

    /// 启用后台任务
    pub async fn spawn<F, Fut>(&self, task: F)
    where
        F: FnOnce(Arc<Notify>, Arc<Notify>) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let mut tasks = self.tasks.lock().await;
        let notify = Arc::clone(&self.notify);
        let ready = Arc::new(Notify::new());
        let waiter = Arc::clone(&ready);
        tasks.spawn(task(ready, notify));
        waiter.notified().await;
    }

    pub(crate) fn topic_tx(&self) -> mpsc::UnboundedSender<TopicTask> {
        self.topic_tx.clone()
    }

    /// 优雅关闭
    pub async fn shutdown(&self) {
        if self
            .initiated
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
        {
            info!("开始优雅退出");
            self.notify.notify_waiters();
            let mut tasks = self.tasks.lock().await;
            while let Some(result) = tasks.join_next().await {
                if let Err(e) = result {
                    error!("后台任务发生错误：{e}");
                }
            }
            info!("优雅退出所有后台任务");
        }
    }
}

async fn topic_creator(
    mut topic_rx: mpsc::UnboundedReceiver<TopicTask>,
    ready: Arc<Notify>,
    notify: Arc<Notify>,
) {
    info!("启动聚合主题创建者");
    let capacity = 20;
    let mut batch = Vec::with_capacity(capacity);
    let mut last_flush = Instant::now();
    let mut interval = tokio::time::interval(Duration::from_millis(1));
    let mut count: usize = 0;
    let threshold = capacity - 1;
    let notified = notify.notified();

    tokio::pin!(notified);
    ready.notify_one();
    loop {
        tokio::select! {
            biased;
            _ = &mut notified => {
                info!("收到关闭信号，开始优雅退出");
                if !batch.is_empty() {
                    info!("优雅退出，提交聚合主题创建");
                    topic_batch(&mut batch).await;
                }
                break;
            }
            _ = interval.tick() => {
                if !batch.is_empty() && last_flush.elapsed() > Duration::from_millis(5) {
                    debug!("触及提交间隔阈值，提交聚合主题创建");
                    topic_batch(&mut batch).await;
                    debug!("结束聚合主题批量创建");
                    last_flush = Instant::now();
                    count = 0;
                }
            }
            data = topic_rx.recv() => {
                match data {
                    Some(TopicTask { agg_type, agg_id }) => {
                        let mut topic = String::with_capacity(agg_type.len() + 37);
                        let span = debug_span!("create_topic", agg_type, %agg_id);
                        span.in_scope(|| {
                            topic.push_str(agg_type);
                            topic.push_str("-");
                            topic.push_str(&agg_id.to_string());
                            debug!("收到聚合主题创建任务");
                            batch.push(topic);
                        });

                        if count == threshold {
                            debug!("触及提交计数阈值，提交聚合主题创建");
                            topic_batch(&mut batch).await;
                            last_flush = Instant::now();
                            count = 0;
                        } else {
                            count += 1;
                        }
                    }
                    None => {
                        info!("发送端均已关闭，开始优雅退出");
                        if !batch.is_empty() {
                            info!("优雅退出，提交聚合主题创建");
                            topic_batch(&mut batch).await;
                        }
                        break;
                    }
                }
            }
        }
    }
}

async fn topic_batch(batch: &mut Vec<String>) {
    let mut topics = Vec::new();
    let tns: Vec<String> = batch.drain(..).collect();

    for topic in &tns {
        let topic = NewTopic::new(
            &topic,
            1,
            TopicReplication::Fixed(SUBSCRIBER_CONFIG.replicas),
        )
        .set("retention.ms", "-1");
        topics.push(topic);
    }

    debug!("开始聚合主题批量创建");
    if let Err(e) = ADMIN.create_topics(&topics, &OPTS).await {
        error!("批量创建聚合主题失败: {e}");
    }
}
