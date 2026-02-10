use super::{SUBSCRIBER_CONFIG, TopicTask};
use crate::{Context, config::load_bootstrap, subscriber::core::Subscriber};
use rdkafka::{
    ClientConfig,
    admin::{AdminClient, AdminOptions, NewTopic, TopicReplication},
    client::DefaultClientContext,
};
use rkyv::{
    Archive, Deserialize,
    de::Pool,
    rancor::{Error, Strategy},
};
use std::{
    ops::Deref,
    sync::{Arc, LazyLock},
};
use tokio::{
    sync::{Notify, OnceCell, mpsc},
    time::{Duration, Instant},
};
use tracing::{debug, debug_span, error, info};
use unis::{config::build_config, domain::CommandEnum};

static ADMIN: LazyLock<AdminClient<DefaultClientContext>> = LazyLock::new(|| {
    let config = build_config();
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

static CONTEXT: OnceCell<Arc<App>> = OnceCell::const_new();
/// 订阅者上下文
pub async fn context() -> Arc<App> {
    Arc::clone(
        CONTEXT
            .get_or_init(|| async {
                LazyLock::force(&ADMIN);
                LazyLock::force(&OPTS);
                let (topic_tx, topic_rx) = mpsc::unbounded_channel::<TopicTask>();
                let app = App::new(topic_tx);
                app.spawn_notify(move |ready, notify| topic_creator(topic_rx, ready, notify))
                    .await;
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
    LazyLock::force(&ADMIN);
    LazyLock::force(&OPTS);
    let (topic_tx, topic_rx) = mpsc::unbounded_channel::<TopicTask>();
    let app = App::new(topic_tx);
    app.spawn_notify(move |ready, notify| topic_creator(topic_rx, ready, notify))
        .await;
    app
}

/// 订阅者上下文结构
pub struct App {
    topic_tx: mpsc::UnboundedSender<TopicTask>,
    context: Context,
}

impl App {
    fn new(topic_tx: mpsc::UnboundedSender<TopicTask>) -> Arc<Self> {
        Arc::new(Self {
            topic_tx,
            context: Context::new(),
        })
    }

    /// 设置特定聚合类型的订阅者
    pub async fn setup<C>(self: &Arc<Self>)
    where
        C: CommandEnum,
        <C as Archive>::Archived: Sync + Deserialize<C, Strategy<Pool, Error>>,
    {
        if let Err(e) = Subscriber::<C::A, C, C::E>::launch(Arc::clone(self)).await {
            error!("{e}");
            self.shutdown().await;
            self.all_done().await;
            panic!("异常退出订阅者初始设置")
        }
    }

    pub(super) fn topic_tx(&self) -> mpsc::UnboundedSender<TopicTask> {
        self.topic_tx.clone()
    }
}

impl Deref for App {
    type Target = Context;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

async fn topic_creator(
    mut topic_rx: mpsc::UnboundedReceiver<TopicTask>,
    ready: Arc<Notify>,
    notify: Arc<Notify>,
) {
    info!("启动聚合主题创建者");
    let capacity = 1000;
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
                    info!("优雅退出，提交聚合主题批量创建");
                    topic_batch(&mut batch).await;
                }
                break;
            }
            _ = interval.tick() => {
                if !batch.is_empty() && last_flush.elapsed() > Duration::from_millis(5) {
                    debug!("触及提交间隔阈值，提交聚合主题批量创建");
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
                            debug!("触及提交计数阈值，提交聚合主题批量创建");
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
                            info!("优雅退出，提交聚合主题批量创建");
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
