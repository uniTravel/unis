//! Kafka订阅者

use crate::{
    aggregator::Aggregator,
    domain::{Aggregate, Config, Dispatch, EventEnum, Load},
    kafka::{
        commit::{CommitCoordinator, CommitTask},
        config::SubscriberConfig,
        errors::SubscriberError,
        reader::restore,
        writer::write,
    },
};
use futures::StreamExt;
use rdkafka::{
    ClientConfig, Message,
    consumer::{Consumer, StreamConsumer},
    message::{BorrowedMessage, Headers},
};
use std::{
    marker::PhantomData,
    sync::{Arc, LazyLock},
};
use tokio::sync::{mpsc, watch};
use tracing::{debug, info, warn};
use uuid::Uuid;

pub(crate) static SUBSCRIBER_CONFIG: LazyLock<SubscriberConfig> =
    LazyLock::new(|| SubscriberConfig::get());

/// 订阅者
pub struct Subscriber<A, D, E, L>
where
    A: Aggregate,
    D: Dispatch<A, E, L>,
    E: EventEnum<A = A>,
    L: Load,
{
    _marker_a: PhantomData<A>,
    _marker_d: PhantomData<D>,
    _marker_e: PhantomData<E>,
    _marker_l: PhantomData<L>,
}

impl<A, D, E, L> Subscriber<A, D, E, L>
where
    A: Aggregate + Send + 'static,
    D: Dispatch<A, E, L>,
    E: EventEnum<A = A> + bincode::Encode + Send + 'static,
    L: Load + Copy,
{
    async fn run_consumer(
        mut aggregator: Aggregator<A, D, E, L>,
        cc: Arc<StreamConsumer>,
        commit_tx: mpsc::Sender<CommitTask>,
        mut shutdown_rx: watch::Receiver<bool>,
    ) {
        let message_stream = cc.stream();
        tokio::pin!(message_stream);

        loop {
            tokio::select! {
                biased;
                _ = shutdown_rx.changed(), if *shutdown_rx.borrow() => {
                    info!("收到关闭信号，开始优雅退出");
                    break;
                }
                Some(msg) = message_stream.next() => match msg {
                    Ok(msg) => {
                        match process_message(&msg).await {
                            Ok((agg_id, com_id, com_data)) => {
                                debug!("发送聚合{agg_id}命令{com_id}");
                                if let Err(e) = aggregator.commit(agg_id, com_id, com_data).await {
                                    warn!("发送聚合{agg_id}命令{com_id}错误：{e}");
                                }
                            }
                            Err(e) => warn!("{e}"),
                        }
                        if let Err(e) = commit_tx.send(CommitTask::from(&msg)).await {
                            warn!("发送消费偏移量错误：{e}");
                        }
                    }
                    Err(e) => {
                        warn!("消息错误：{e}");
                    }
                }
            }
        }
    }

    /// 构造函数
    pub async fn new(dispatcher: D, loader: L) -> Self {
        let agg_type = std::any::type_name::<A>();
        let cfg_name = agg_type.rsplit("::").next().expect("获取聚合名称失败");
        let settings = SUBSCRIBER_CONFIG
            .cc
            .get(cfg_name)
            .expect("获取聚合命令消费者配置失败");
        let cfg = SUBSCRIBER_CONFIG.aggregates.get(cfg_name);
        let mut topic = String::with_capacity(agg_type.len() + 8);
        topic.push_str(agg_type);
        topic.push_str("-command");
        let mut config = ClientConfig::new();
        config.set("bootstrap.servers", &SUBSCRIBER_CONFIG.bootstrap);
        config.set("group.id", &topic);
        for (key, value) in settings {
            config.set(key, value);
        }
        let cc: Arc<StreamConsumer> = Arc::new(config.create().expect("消费者创建失败"));
        cc.subscribe(&[&topic]).expect("订阅聚合命令失败");

        let (commit_tx, commit_rx) = mpsc::channel::<CommitTask>(cfg.capacity);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let aggregator = Aggregator::new(cfg, dispatcher, loader, restore, write).await;
        info!("成功启用{agg_type}聚合器");
        let _ = CommitCoordinator::new(cc.clone(), commit_rx, shutdown_rx.clone());

        let ctrl_c = tokio::spawn(async move {
            tokio::signal::ctrl_c().await.unwrap();
            shutdown_tx.send(true).unwrap();
        });

        let consumer_task =
            tokio::spawn(Self::run_consumer(aggregator, cc, commit_tx, shutdown_rx));

        tokio::select! {
            _ = ctrl_c => (),
            res = consumer_task => {
                if let Err(e) = res {
                    warn!("消费者任务错误: {e}")
                }
            }
        }

        info!("成功启用{agg_type}订阅者");

        Self {
            _marker_a: PhantomData,
            _marker_d: PhantomData,
            _marker_e: PhantomData,
            _marker_l: PhantomData,
        }
    }
}

async fn process_message(
    msg: &BorrowedMessage<'_>,
) -> Result<(Uuid, Uuid, Vec<u8>), SubscriberError> {
    let key = msg.key().ok_or("消息键不存在")?;
    let agg_id = Uuid::from_slice(key).map_err(|e| SubscriberError::Processing(e.to_string()))?;
    debug!("提取聚合Id：{agg_id}");

    let id = msg
        .headers()
        .ok_or("消息头不存在")?
        .iter()
        .find(|h| h.key == "com_id")
        .ok_or("键为'com_id'的消息头不存在")?
        .value
        .ok_or("键'com_id'对应的值为空")?;
    let com_id = Uuid::from_slice(id).map_err(|e| SubscriberError::Processing(e.to_string()))?;
    debug!("提取命令Id：{com_id}");

    let com_data = msg.payload().ok_or("空消息体")?;
    Ok((agg_id, com_id, com_data.to_vec()))
}
