//! Kafka发送者

use crate::{
    domain::{Aggregate, Config},
    kafka::{
        commit::{CommitCoordinator, CommitTask},
        config::SenderConfig,
    },
};
use futures::StreamExt;
use rdkafka::{
    ClientConfig, Message,
    consumer::{Consumer, StreamConsumer},
    message::{BorrowedMessage, Headers},
    producer::FutureProducer,
};
use std::{
    marker::PhantomData,
    sync::{Arc, LazyLock},
};
use tokio::sync::{mpsc, watch};
use tracing::warn;

static SENDER_CONFIG: LazyLock<SenderConfig> = LazyLock::new(|| SenderConfig::get());

static CP: LazyLock<FutureProducer> = LazyLock::new(|| {
    let mut config = ClientConfig::new();
    config.set("bootstrap.servers", &SENDER_CONFIG.bootstrap);
    for (key, value) in &SENDER_CONFIG.cp {
        config.set(key, value);
    }
    config.create().expect("聚合命令生产者创建失败")
});

/// 发送者
pub struct Sender<A>
where
    A: Aggregate,
{
    _marker_a: PhantomData<A>,
}

impl<A> Sender<A>
where
    A: Aggregate + 'static,
{
    async fn run_consumer(
        tc: Arc<StreamConsumer>,
        commit_tx: mpsc::Sender<CommitTask>,
        mut shutdown_rx: watch::Receiver<bool>,
    ) {
    }

    /// 构造函数
    pub async fn new() -> Self {
        let agg_type = std::any::type_name::<A>();
        let cfg_name = agg_type.rsplit("::").next().expect("获取聚合名称失败");

        let mut config = ClientConfig::new();
        config.set("bootstrap.servers", &SENDER_CONFIG.bootstrap);
        config.set("group.id", format!("{agg_type}-{}", SENDER_CONFIG.hostname));
        let tc: Arc<StreamConsumer> = Arc::new(config.create().expect("消费者创建失败"));
        let (commit_tx, commit_rx) = mpsc::channel::<CommitTask>(100);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let _ = CommitCoordinator::new(tc.clone(), commit_rx, shutdown_rx.clone());

        let ctrl_c = tokio::spawn(async move {
            tokio::signal::ctrl_c().await.unwrap();
            shutdown_tx.send(true).unwrap();
        });

        let consumer_task = tokio::spawn(Self::run_consumer(tc, commit_tx, shutdown_rx));

        tokio::select! {
            _ = ctrl_c => (),
            res = consumer_task => {
                if let Err(e) = res {
                    warn!("消费者任务错误: {e}")
                }
            }
        }

        Self {
            _marker_a: PhantomData,
        }
    }
}
