//! Kafka 发送者

use crate::{
    BINCODE_HEADER,
    commit::{Commit, commit_coordinator},
    config::SenderConfig,
};
use ahash::AHashMap;
use bincode::error::EncodeError;
use futures::StreamExt;
use rdkafka::{
    ClientConfig, Message,
    consumer::{Consumer, StreamConsumer},
    message::{BorrowedMessage, Header, Headers, OwnedHeaders},
    producer::{FutureProducer, FutureRecord, future_producer::Delivery},
};
use std::{
    marker::PhantomData,
    sync::{Arc, LazyLock},
};
use tokio::{
    sync::{mpsc, oneshot, watch},
    time::{Duration, Instant, MissedTickBehavior, interval_at},
};
use tracing::{debug, error, info, warn};
use unis::{
    BINCODE_CONFIG, Response,
    config::SendConfig,
    domain::{Aggregate, CommandEnum, Config, Request},
    errors::UniError,
    pool::BufferPool,
};
use uuid::Uuid;

static SENDER_CONFIG: LazyLock<SenderConfig> = LazyLock::new(|| SenderConfig::get());

static SHARED: LazyLock<Arc<FutureProducer>> = LazyLock::new(|| {
    let mut config = ClientConfig::new();
    config.set("bootstrap.servers", &SENDER_CONFIG.bootstrap);
    Arc::new(config.create().expect("共享的聚合类型生产者创建失败"))
});

static CP_CONFIG: LazyLock<ClientConfig> = LazyLock::new(|| {
    let mut config = ClientConfig::new();
    for (key, value) in &SENDER_CONFIG.cp {
        config.set(key, value);
    }
    config.set("bootstrap.servers", &SENDER_CONFIG.bootstrap);
    config
});

static SHUTDOWN_RX: LazyLock<watch::Receiver<bool>> = LazyLock::new(|| {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("无法监听 Ctrl+C 信号");
        shutdown_tx.send(true).expect("发送 Ctrl+C 信号失败");
    });
    shutdown_rx
});

enum Todo<A, C>
where
    A: Aggregate + Sync,
    C: CommandEnum<A = A> + Sync + 'static,
{
    Reply {
        agg_id: Uuid,
        com_id: Uuid,
        com: C,
        res_tx: oneshot::Sender<Response>,
    },
    Response {
        com_id: Uuid,
        res: Response,
    },
}

/// 发送者结构
pub struct Sender<A, C>
where
    A: Aggregate + Sync,
    C: CommandEnum<A = A> + Sync + 'static,
{
    tx: mpsc::UnboundedSender<Todo<A, C>>,
    _marker_a: PhantomData<A>,
    _marker_c: PhantomData<C>,
}

impl<A, C> Request<A, C> for Sender<A, C>
where
    A: Aggregate + Sync,
    C: CommandEnum<A = A> + Sync + 'static,
{
    /// 构造函数
    async fn new() -> Self {
        let agg_type = A::topic();
        let cfg_name = agg_type.rsplit(".").next().expect("获取聚合名称失败");
        let cfg = SENDER_CONFIG.sender.get(cfg_name);
        let topic = A::topic_com();
        let producer = match cfg.hotspot {
            true => Arc::new(CP_CONFIG.create().expect("命令生产者创建失败")),
            false => SHARED.clone(),
        };
        info!("成功创建 {topic} 命令生产者");

        let mut config = ClientConfig::new();
        config.set("bootstrap.servers", &SENDER_CONFIG.bootstrap);
        config.set("group.id", format!("{agg_type}-{}", SENDER_CONFIG.hostname));
        config.set("enable.auto.commit", "false");
        let tc: Arc<StreamConsumer> = Arc::new(config.create().expect("发送者消费创建失败"));
        tc.subscribe(&[agg_type]).expect("订阅类型事件流失败");
        info!("成功订阅类型 {agg_type} 事件流");

        let (tx, rx) = mpsc::unbounded_channel::<Todo<A, C>>();
        let (commit_tx, commit_rx) = mpsc::unbounded_channel::<Commit>();
        let pool = Arc::new(BufferPool::new(cfg.bufs, cfg.sems));
        tokio::spawn(commit_coordinator(tc.clone(), commit_rx));
        tokio::spawn(Self::responsor(producer, topic, pool, agg_type, cfg, rx));
        info!("成功启用类型 {agg_type} 响应处理器");
        tokio::spawn(Self::consumer(tc, tx.clone(), commit_tx));
        info!("成功启用类型 {agg_type} 发送者");

        Self {
            tx,
            _marker_a: PhantomData,
            _marker_c: PhantomData,
        }
    }

    async fn send(&self, agg_id: Uuid, com_id: Uuid, com: C) -> Response {
        let (res_tx, res_rx) = oneshot::channel::<Response>();
        if let Err(e) = self.tx.send(Todo::Reply {
            agg_id,
            com_id,
            com,
            res_tx,
        }) {
            error!("聚合 {agg_id} 命令 {com_id} 请求反馈错误：{e}");
            panic!("响应处理器停止工作");
        }

        match res_rx.await {
            Ok(res) => {
                info!("聚合 {agg_id} 命令 {com_id} 收到反馈");
                res
            }
            Err(e) => {
                error!("聚合 {agg_id} 命令 {com_id} 接收反馈错误：{e}");
                Response::Timeout
            }
        }
    }
}

impl<A, C> Sender<A, C>
where
    A: Aggregate + Sync,
    C: CommandEnum<A = A> + Sync + 'static,
{
    async fn responsor(
        producer: Arc<FutureProducer>,
        topic: &'static str,
        pool: Arc<BufferPool>,
        agg_type: &str,
        cfg: SendConfig,
        mut rx: mpsc::UnboundedReceiver<Todo<A, C>>,
    ) {
        let mut rs: AHashMap<Uuid, (Option<oneshot::Sender<Response>>, Option<Response>, Instant)> =
            AHashMap::new();
        let start = Instant::now();
        let mut interval = interval_at(start, Duration::from_secs(cfg.interval));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                biased;
                _ = interval.tick() => {
                    let _ = rs.extract_if(|_, (_, _, t)| t.elapsed() > Duration::from_secs(cfg.retain));
                }
                data = rx.recv() => {
                    match data {
                        Some(Todo::Reply {agg_id, com_id, com, res_tx }) => {
                            match rs.get_mut(&com_id) {
                                Some((Some(_), None, _)) => {
                                    warn!("命令 {com_id} 不得重复请求反馈");
                                    let _ = res_tx.send(Response::Duplicate);
                                }
                                Some((None, Some(_), _)) => {
                                    if let Some((_, Some(res), _)) = rs.remove(&com_id) {
                                        let _ = res_tx.send(res);
                                    }
                                }
                                Some(_) => error!("请求反馈进入非法处理分支"),
                                None => {
                                    let mut buf = pool.get();
                                    match loop {
                                        match bincode::encode_into_slice(&com, buf.as_mut(), BINCODE_CONFIG) {
                                            Ok(len) => break Ok(&buf[..len]),
                                            Err(EncodeError::UnexpectedEnd) => {
                                                let new_size = buf.capacity() * 2;
                                                buf.reserve(new_size);
                                            }
                                            Err(e) => break Err(UniError::EncodeError(e)),
                                        }
                                    } {
                                        Ok(com_data) => {
                                            let record = FutureRecord::to(topic)
                                                .payload(com_data)
                                                .key(agg_id.as_bytes())
                                                .headers(OwnedHeaders::new_with_capacity(1).insert(Header {
                                                    key: "com_id",
                                                    value: Some(com_id.as_bytes()),
                                                }));
                                            match producer
                                                .send(record, SENDER_CONFIG.timeout)
                                                .await
                                                .map_err(|(e, _)| UniError::SendError(e.to_string()))
                                                .map(
                                                    |Delivery {
                                                        partition,
                                                        offset,
                                                        timestamp: _timestamp,
                                                    }| {
                                                        info!("聚合 {agg_id} 命令 {com_id} 写入分区 {partition} 偏移 {offset}");
                                                    },
                                                ) {
                                                Ok(()) => {
                                                    rs.insert(com_id, (Some(res_tx), None, Instant::now()));
                                                }
                                                Err(e) => {
                                                    error!("聚合 {agg_id} 命令 {com_id} 发送失败：{e}");
                                                    let _ = res_tx.send(e.response());
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            error!("聚合 {agg_id} 命令 {com_id} 序列化错误：{e}");
                                            let _ = res_tx.send(e.response());
                                        }
                                    }
                                    pool.put(buf);
                                }
                            }
                        }
                        Some(Todo::Response { com_id, res }) => {
                            match rs.get_mut(&com_id) {
                                Some((Some(_), None, _)) => {
                                    if let Some((Some(res_tx), _, _)) = rs.remove(&com_id) {
                                        let _ = res_tx.send(res);
                                    }
                                }
                                Some(_) => error!("发送反馈进入非法处理分支"),
                                None => {
                                    rs.insert(com_id, (None, Some(res), Instant::now()));
                                }
                            }
                        }
                        None => {
                            info!("发送端已关闭，类型 {agg_type} 响应处理器稍后将停止工作");
                            break;
                        }
                    }
                }
            }
        }
    }

    async fn consumer(
        tc: Arc<StreamConsumer>,
        tx: mpsc::UnboundedSender<Todo<A, C>>,
        commit_tx: mpsc::UnboundedSender<Commit>,
    ) {
        let mut shutdown_rx = SHUTDOWN_RX.clone();
        let message_stream = tc.stream();
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
                            Ok((agg_id, com_id, res)) => {
                                if let Err(e) = tx.send(Todo::Response { com_id, res }) {
                                    error!("聚合 {agg_id} 命令 {com_id} 发送反馈错误：{e}");
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
}

async fn process_message(msg: &BorrowedMessage<'_>) -> Result<(Uuid, Uuid, Response), UniError> {
    let key = msg.key().ok_or("消息键不存在")?;
    let agg_id = Uuid::from_slice(key).map_err(|e| UniError::MsgError(e.to_string()))?;
    debug!("提取聚合Id：{agg_id}");

    let headers = msg.headers().ok_or("消息头不存在")?;

    let id = headers
        .iter()
        .find(|h| h.key == "com_id")
        .ok_or("键为'com_id'的消息头不存在")?
        .value
        .ok_or("键'com_id'对应的值为空")?;
    let com_id = Uuid::from_slice(id).map_err(|e| UniError::MsgError(e.to_string()))?;
    debug!("提取命令Id：{com_id}");

    let res_data = headers
        .iter()
        .find(|h| h.key == "response")
        .ok_or("键为'response'的消息头不存在")?
        .value
        .ok_or("键'response'对应的值为空")?;
    let (res, _): (Response, _) = bincode::decode_from_slice(&res_data, BINCODE_HEADER)?;
    debug!("提取命令处理结果：{:?}", res);

    Ok((agg_id, com_id, res))
}
