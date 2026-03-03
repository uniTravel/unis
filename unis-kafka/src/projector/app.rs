//! Kafka 投影者上下文

use super::{CONFIG, ProjectError, core::Projector};
use crate::{
    Context,
    config::{load_bootstrap, load_hostname, load_name},
};
use rdkafka::{
    ClientConfig,
    consumer::{Consumer, StreamConsumer},
    error::KafkaError,
    producer::{FutureProducer, Producer},
};
use std::{
    ops::Deref,
    sync::{Arc, LazyLock},
    thread::sleep,
    time::Duration,
};
use tokio::sync::Notify;
use tracing::error;

static CONTEXT: LazyLock<App> = LazyLock::new(|| App::new());
fn app() -> &'static App {
    &CONTEXT
}

/// 投影者上下文
pub async fn context() -> &'static App {
    tokio::spawn(async move {
        crate::shutdown_signal().await;
        app().shutdown().await;
    });
    app()
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-utils"))]
pub async fn test_context() -> &'static App {
    app()
}

fn create_producer(bootstrap: &str, transaction_id: &str) -> Result<FutureProducer, KafkaError> {
    let ap: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", bootstrap)
        .set("transactional.id", transaction_id)
        .create()?;
    ap.init_transactions(Duration::from_secs(30))?;
    Ok(ap)
}

fn create_consumer(bootstrap: &str, group_id: &str) -> Result<StreamConsumer, KafkaError> {
    ClientConfig::new()
        .set("bootstrap.servers", bootstrap)
        .set("group.id", group_id)
        .set("enable.auto.commit", "false")
        .set("auto.offset.reset", "earliest")
        .set("isolation.level", "read_committed")
        .set("group.protocol", "consumer")
        .create()
}

/// 投影者上下文结构
pub struct App {
    projector: Projector,
    context: Context,
}

impl App {
    fn new() -> Self {
        Self {
            projector: Projector::new(),
            context: Context::new(),
        }
    }

    /// 启动投影
    pub async fn launch(&'static self, topics: Vec<&'static str>) {
        self.spawn_notify(move |ready, notify| self.run(topics, ready, notify))
            .await;
    }

    async fn run(&self, topics: Vec<&'static str>, ready: Arc<Notify>, notify: Arc<Notify>) {
        let secs = 45;
        let mut count = 0;
        let bootstrap = load_bootstrap(&super::CONFIG);
        let group_id = load_name(&CONFIG);
        let hostname = load_hostname(&super::CONFIG);
        let transaction = format!("{group_id}-{hostname}");
        let mut tc = create_consumer(&bootstrap, &group_id).expect("初创投影消费者失败");
        tc.subscribe(&topics).expect("订阅聚合类型事件流失败");
        let mut ap = create_producer(&bootstrap, &transaction).expect("初创投影生产者失败");
        loop {
            match self
                .projector
                .process(&ap, &tc, Arc::clone(&ready), Arc::clone(&notify))
                .await
            {
                Ok(()) => break,
                Err(ProjectError::UniError(e)) => {
                    error!("投影处理错误：{:?}", e);
                    break;
                }
                Err(ProjectError::MetadataError) => {
                    count += 1;
                    error!("获取消费组元数据失败");
                    if count == 15 {
                        error!("重试 {count} 次仍然失败，退出应用！");
                        break;
                    } else {
                        sleep(Duration::from_secs(secs));
                        continue;
                    }
                }
                Err(ProjectError::KafkaError(e)) => {
                    error!("投影处理错误：{:?}", e);
                    tc = create_consumer(&bootstrap, &group_id).expect("初创投影消费者失败");
                    ap = create_producer(&bootstrap, &transaction).expect("初创投影生产者失败");
                    sleep(Duration::from_secs(secs));
                }
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
