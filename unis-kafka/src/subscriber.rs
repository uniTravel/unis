//! Kafka 订阅者

use crate::{
    commit::{Commit, commit_coordinator},
    config::SubscriberConfig,
    reader::restore,
    stream::Writer,
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
use tracing::{debug, error, info};
use unis::{
    Com,
    aggregator::Aggregator,
    domain::{Aggregate, Config, Dispatch, EventEnum, Load},
    errors::UniError,
};
use uuid::Uuid;

pub(crate) static SUBSCRIBER_CONFIG: LazyLock<SubscriberConfig> =
    LazyLock::new(|| SubscriberConfig::get());

static SHUTDOWN_RX: LazyLock<watch::Receiver<bool>> = LazyLock::new(|| {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("无法监听 Ctrl+C 信号");
        shutdown_tx.send(true).expect("发送 Ctrl+C 信号失败");
    });
    shutdown_rx
});

/// 订阅者结构
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
    A: Aggregate + Clone,
    D: Dispatch<A, E, L>,
    E: EventEnum<A = A>,
    L: Load,
{
    /// 启动订阅者
    pub async fn launch(dispatcher: D, loader: L) {
        let agg_type = A::topic();
        let cfg_name = agg_type.rsplit(".").next().expect("获取聚合名称失败");
        let settings = SUBSCRIBER_CONFIG
            .cc
            .get(cfg_name)
            .expect("获取订阅者消费配置失败");
        let cfg = SUBSCRIBER_CONFIG.subscriber.get(cfg_name);
        let topic = A::topic_com();
        let mut config = ClientConfig::new();
        for (key, value) in settings {
            config.set(key, value);
        }
        config.set("bootstrap.servers", &SUBSCRIBER_CONFIG.bootstrap);
        config.set("group.id", topic);
        let cc: Arc<StreamConsumer> = Arc::new(config.create().expect("订阅者消费创建失败"));
        cc.subscribe(&[topic]).expect("订阅命令流失败");
        info!("成功订阅 {topic} 命令流");

        let (tx, rx) = mpsc::unbounded_channel::<Com>();
        let (commit_tx, commit_rx) = mpsc::unbounded_channel::<Commit>();
        tokio::spawn(commit_coordinator(cc.clone(), commit_rx));
        tokio::spawn(consumer(cc, tx, commit_tx));
        info!("成功启用类型 {agg_type} 订阅者");

        Aggregator::launch(
            &cfg,
            dispatcher,
            loader,
            Arc::new(Writer::new(&cfg)),
            restore,
            rx,
        )
        .await;
        info!("成功启用类型 {agg_type} 聚合器");
    }
}

async fn consumer(
    cc: Arc<StreamConsumer>,
    tx: mpsc::UnboundedSender<Com>,
    commit_tx: mpsc::UnboundedSender<Commit>,
) {
    let mut shutdown_rx = SHUTDOWN_RX.clone();
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
                            debug!("发送聚合 {agg_id} 命令 {com_id}");
                            if let Err(e) = tx.send(Com{agg_id, com_id, com_data}) {
                                error!("发送聚合 {agg_id} 命令 {com_id} 错误：{e}");
                            }
                        }
                        Err(e) => error!("{e}"),
                    }
                    if let Err(e) = commit_tx.send(Commit::from(&msg)) {
                        error!("发送消费偏移量错误：{e}");
                    }
                }
                Err(e) => error!("消息错误：{e}"),
            }
        }
    }
}

async fn process_message(msg: &BorrowedMessage<'_>) -> Result<(Uuid, Uuid, Vec<u8>), UniError> {
    let key = msg.key().ok_or("消息键不存在")?;
    let agg_id = Uuid::from_slice(key).map_err(|e| UniError::MsgError(e.to_string()))?;
    debug!("提取聚合Id：{agg_id}");

    let id = msg
        .headers()
        .ok_or("消息头不存在")?
        .iter()
        .find(|h| h.key == "com_id")
        .ok_or("键为'com_id'的消息头不存在")?
        .value
        .ok_or("键'com_id'对应的值为空")?;
    let com_id = Uuid::from_slice(id).map_err(|e| UniError::MsgError(e.to_string()))?;
    debug!("提取命令Id：{com_id}");

    let com_data = msg.payload().ok_or("空消息体")?;
    Ok((agg_id, com_id, com_data.to_vec()))
}
