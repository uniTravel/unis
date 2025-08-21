use crate::{
    commit::{CommitCoordinator, CommitTask},
    errors::SubscriberError,
};
use bytes::Bytes;
use futures::StreamExt;
use rdkafka::{
    ClientConfig, Message,
    consumer::{Consumer, StreamConsumer},
    message::{BorrowedMessage, Headers},
};
use std::{collections::HashMap, marker::PhantomData, sync::Arc};
use tokio::sync::{Semaphore, mpsc, oneshot, watch};
use tracing::{debug, info, warn};
use unis::{
    aggregator::{AggTask, Aggregator, Res},
    config::AggConfig,
    domain::{Aggregate, Dispatch, EventEnum, Load, Replay, Stream},
    errors::DomainError,
};
use uuid::Uuid;

pub struct Subscriber<A, D, E, L, R, S>
where
    A: Aggregate,
    D: Dispatch<A, E, L, R, S>,
    E: EventEnum<A = A>,
    L: Load<A, R, S>,
    R: Replay<A = A>,
    S: Stream<A = A>,
{
    _marker_a: PhantomData<A>,
    _marker_d: PhantomData<D>,
    _marker_e: PhantomData<E>,
    _marker_r: PhantomData<R>,
    _marker_s: PhantomData<S>,
    _marker_l: PhantomData<L>,
}

impl<A, D, E, L, R, S> Subscriber<A, D, E, L, R, S>
where
    A: Aggregate + Send + 'static,
    D: Dispatch<A, E, L, R, S> + Send + 'static,
    E: EventEnum<A = A> + bincode::Encode + Send + 'static,
    L: Load<A, R, S> + Send + 'static,
    R: Replay<A = A> + Send + 'static,
    S: Stream<A = A> + Send + 'static,
{
    async fn run_consumer(
        semaphore: Arc<Semaphore>,
        consumer: Arc<StreamConsumer>,
        agg_tx: mpsc::Sender<AggTask>,
        commit_tx: mpsc::Sender<CommitTask>,
        mut shutdown_rx: watch::Receiver<bool>,
    ) {
        let message_stream = consumer.stream();
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
                        let _permit = semaphore.acquire().await.unwrap();
                        match process_message(&msg).await {
                            Ok((agg_id, com_id, com_data)) => {
                                let (reply_tx, reply_rx) = oneshot::channel();
                                debug!("开始处理聚合{agg_id}命令{com_id}");
                                match agg_tx.send(AggTask::Com {agg_id, com_id, com_data, reply_tx}).await {
                                    Ok(()) => {
                                        match reply_rx.await {
                                            Ok(reply) => match reply {
                                                Ok(()) => (),
                                                Err(e @ DomainError::Duplicate) => {
                                                    warn!("聚合{agg_id}命令{com_id}处理错误：{e}");
                                                    let res = Res::Duplicate;
                                                    let evt_data = Bytes::new();
                                                    if let Err(e) = agg_tx.send(AggTask::Evt { agg_id, com_id, res, evt_data }).await {
                                                        warn!("发送聚合{agg_id}命令{com_id}的重复提交事件错误：{e}");
                                                    }
                                                }
                                                Err(e) => {
                                                    warn!("聚合{agg_id}命令{com_id}处理错误：{e}");
                                                    let res = Res::Fail;
                                                    let evt_data = Bytes::from(e.to_string());
                                                    if let Err(e) = agg_tx.send(AggTask::Evt { agg_id, com_id, res, evt_data }).await {
                                                        warn!("发送聚合{agg_id}命令{com_id}的处理异常事件错误：{e}");
                                                    }
                                                }
                                            }
                                            Err(e) => warn!("接收聚合{agg_id}命令{com_id}处理结果错误：{e}")
                                        }
                                    }
                                    Err(e) => warn!("发送聚合{agg_id}命令{com_id}错误：{e}")
                                }

                                if let Err(e) = commit_tx.send(CommitTask::from(&msg)).await {
                                    warn!("推送聚合{agg_id}命令{com_id}的消费偏移量错误：{e}");
                                }
                            }
                            Err(e) => warn!("{e}")
                        }
                    }
                    Err(e) => {
                        warn!("消息错误：{e}");
                    }
                }
            }
        }
    }

    pub async fn new(
        cfg: AggConfig,
        settings: HashMap<String, String>,
        dispatcher: D,
        loader: L,
        replayer: R,
        stream: S,
    ) -> Self {
        let mut config = ClientConfig::new();
        for (key, value) in settings {
            config.set(key, value);
        }
        let consumer: Arc<StreamConsumer> = Arc::new(config.create().expect("消费者创建失败"));
        let name = std::any::type_name::<A>();
        let mut topic = String::with_capacity(name.len() + 8);
        topic.push_str(name);
        topic.push_str("-command");
        consumer.subscribe(&[&topic]).unwrap();

        let (agg_tx, agg_rx) = mpsc::channel::<AggTask>(cfg.capacity);
        let (commit_tx, commit_rx) = mpsc::channel::<CommitTask>(cfg.capacity);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let semaphore = Arc::new(Semaphore::new(cfg.capacity));
        let _ = Aggregator::new(cfg, agg_rx, dispatcher, loader, replayer, stream).await;
        let _ = CommitCoordinator::new(consumer.clone(), commit_rx, shutdown_rx.clone());

        let ctrl_c = tokio::spawn(async move {
            tokio::signal::ctrl_c().await.unwrap();
            shutdown_tx.send(true).unwrap();
        });

        let consumer_task = tokio::spawn(Self::run_consumer(
            semaphore,
            consumer,
            agg_tx,
            commit_tx,
            shutdown_rx,
        ));

        tokio::select! {
            _ = ctrl_c => {},
            res = consumer_task => {
                if let Err(e) = res {
                    warn!("消费者任务错误: {e}")
                }
            }
        }

        Self {
            _marker_a: PhantomData,
            _marker_d: PhantomData,
            _marker_e: PhantomData,
            _marker_l: PhantomData,
            _marker_r: PhantomData,
            _marker_s: PhantomData,
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
