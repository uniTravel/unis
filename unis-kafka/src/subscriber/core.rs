//! Kafka 订阅者内核

use crate::{
    commit::{Commit, commit_coordinator},
    subscriber::{App, SUBSCRIBER_CONFIG, reader::restore, stream::Writer},
};
use futures::StreamExt;
use rdkafka::{
    ClientConfig, Message,
    consumer::{Consumer, StreamConsumer},
    message::{BorrowedMessage, Headers},
};
use std::{marker::PhantomData, sync::Arc};
use tokio::sync::mpsc;
use tracing::{Span, debug, error, info, info_span, instrument};
use unis::{
    Com,
    aggregator::Aggregator,
    domain::{Aggregate, Dispatch, EventEnum, Load},
    errors::UniError,
};
use uuid::Uuid;

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
    #[instrument(name = "launch_subscriber", skip_all, fields(agg_type))]
    pub async fn launch(app: Arc<App>, dispatcher: D, loader: L) {
        let agg_type = A::topic();
        Span::current().record("agg_type", agg_type);
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
        let stream = Arc::new(Writer::new(&cfg, app.topic_tx()));
        tokio::spawn(commit_coordinator(topic, cc.clone(), commit_rx));
        tokio::spawn(consumer(agg_type, cc, tx, commit_tx));
        tokio::spawn(Aggregator::launch(
            cfg, dispatcher, loader, stream, restore, rx,
        ));
    }
}

#[instrument(name = "receive_command", skip(cc, tx, commit_tx))]
async fn consumer(
    agg_type: &'static str,
    cc: Arc<StreamConsumer>,
    tx: mpsc::UnboundedSender<Com>,
    commit_tx: mpsc::UnboundedSender<Commit>,
) {
    let message_stream = cc.stream();
    // let shutdown_rx = shutdown();
    // let shutdown = shutdown_rx.notified();
    tokio::pin!(message_stream);
    // tokio::pin!(shutdown);
    loop {
        tokio::select! {
            biased;
            // _ = &mut shutdown => {
            //     info!("收到关闭信号，开始优雅退出");
            //     break;
            // }
            Some(msg) = message_stream.next() => match msg {
                Ok(msg) => {
                    match process_message(&msg).await {
                        Ok((agg_id, com_id, com_data)) => {
                            let span = info_span!(parent: None, "handle_command", agg_type, %agg_id, %com_id);
                            span.clone().in_scope(|| {
                                match tx.send(Com{agg_id, com_id, com_data, span}) {
                                    Ok(()) => info!("提交聚合命令"),
                                    Err(e) => error!("提交聚合命令错误：{e}"),
                                }
                            });
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
