//! Kafka 发送者内核

use crate::{BINCODE_HEADER, Context, config::SenderConfig};
use ahash::AHashMap;
use bincode::error::EncodeError;
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
    sync::{Notify, mpsc, oneshot},
    time::{Duration, Instant, MissedTickBehavior, interval_at},
};
use tracing::{Instrument, Span, debug, error, info, info_span, instrument, warn};
use unis::{
    BINCODE_CONFIG, Response,
    config::SendConfig,
    domain::{Aggregate, CommandEnum, Config, Request},
    errors::UniError,
    pool::BufferPool,
};
use uuid::Uuid;

static SENDER_CONFIG: LazyLock<SenderConfig> = LazyLock::new(|| SenderConfig::get());

static SHARED_CP: LazyLock<Arc<FutureProducer>> = LazyLock::new(|| {
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

enum Todo<A, C>
where
    A: Aggregate + Sync,
    C: CommandEnum<A = A> + Sync + 'static,
{
    Reply {
        agg_id: Uuid,
        com_id: Uuid,
        com: C,
        span: tracing::Span,
        res_tx: oneshot::Sender<Response>,
    },
    Response {
        com_id: Uuid,
        span: tracing::Span,
        res: Response,
    },
}

/// 发送者结构
#[derive(Debug)]
pub struct Sender<A, C>
where
    A: Aggregate + Sync,
    C: CommandEnum<A = A> + Sync + 'static,
{
    agg_type: &'static str,
    tx: mpsc::UnboundedSender<Todo<A, C>>,
    _marker_a: PhantomData<A>,
    _marker_c: PhantomData<C>,
}

impl<A, C> Request<A, C> for Sender<A, C>
where
    A: Aggregate + Sync,
    C: CommandEnum<A = A> + Sync + 'static,
{
    #[instrument(name = "send_command", skip_all, fields(agg_type = self.agg_type, %agg_id, %com_id))]
    async fn send(&self, agg_id: Uuid, com_id: Uuid, com: C) -> Response {
        let (res_tx, res_rx) = oneshot::channel::<Response>();
        if let Err(e) = self.tx.send(Todo::Reply {
            agg_id,
            com_id,
            com,
            span: Span::current(),
            res_tx,
        }) {
            error!("聚合命令请求反馈错误：{e}");
            panic!("响应处理器停止工作");
        }

        match res_rx.await {
            Ok(res) => {
                info!("聚合命令收到反馈");
                res
            }
            Err(e) => {
                error!("聚合命令接收反馈错误：{e}");
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
    /// 发送者构造函数
    #[instrument(name = "build_sender", skip_all, fields(agg_type))]
    pub async fn new(context: Arc<Context>) -> Self {
        let agg_type = A::topic();
        Span::current().record("agg_type", agg_type);
        let cfg_name = agg_type.rsplit(".").next().expect("获取聚合名称失败");
        let settings = SENDER_CONFIG
            .tc
            .get(cfg_name)
            .expect("获取发送者消费配置失败");
        let cfg = SENDER_CONFIG.sender.get(cfg_name);
        let topic = A::topic_com();
        let producer = match cfg.hotspot {
            true => Arc::new(CP_CONFIG.create().expect("命令生产者创建失败")),
            false => SHARED_CP.clone(),
        };
        info!("成功创建 {topic} 命令生产者");

        let mut config = ClientConfig::new();
        for (key, value) in settings {
            config.set(key, value);
        }
        config.set("bootstrap.servers", &SENDER_CONFIG.bootstrap);
        config.set("group.id", format!("{agg_type}-{}", SENDER_CONFIG.hostname));
        let tc: Arc<StreamConsumer> = Arc::new(config.create().expect("发送者消费创建失败"));
        tc.subscribe(&[agg_type]).expect("订阅类型事件流失败");
        info!("成功订阅事件流");

        let (tx, rx) = mpsc::unbounded_channel::<Todo<A, C>>();
        let pool = Arc::new(BufferPool::new(cfg.bufs, cfg.sems));
        context
            .spawn_notify(move |ready, notify| {
                Self::respond(agg_type, producer, topic, pool, cfg, rx, ready, notify)
            })
            .await;
        let tx_clone = tx.clone();
        context
            .spawn_notify(move |ready, notify| Self::consume(agg_type, tc, tx_clone, ready, notify))
            .await;

        Self {
            agg_type,
            tx,
            _marker_a: PhantomData,
            _marker_c: PhantomData,
        }
    }

    #[instrument(
        name = "aggregate_respond",
        skip(producer, topic, pool, cfg, rx, ready, notify)
    )]
    async fn respond(
        agg_type: &'static str,
        producer: Arc<FutureProducer>,
        topic: &'static str,
        pool: Arc<BufferPool>,
        cfg: SendConfig,
        mut rx: mpsc::UnboundedReceiver<Todo<A, C>>,
        ready: Arc<Notify>,
        notify: Arc<Notify>,
    ) {
        let mut rs: AHashMap<Uuid, (Option<oneshot::Sender<Response>>, Option<Response>, Instant)> =
            AHashMap::new();
        let start = Instant::now();
        let mut interval = interval_at(start, Duration::from_secs(cfg.interval));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let notified = notify.notified();
        tokio::pin!(notified);
        ready.notify_one();
        loop {
            tokio::select! {
                biased;
                _ = &mut notified => {
                    info!("收到关闭信号，开始优雅退出");
                    break;
                }
                _ = interval.tick() => {
                    let _ = rs.extract_if(|_, (_, _, t)| t.elapsed() > Duration::from_secs(cfg.retain));
                }
                data = rx.recv() => {
                    match data {
                        Some(Todo::Reply {agg_id, com_id, com, span, res_tx }) => {
                            async {
                                match rs.get_mut(&com_id) {
                                    Some((Some(_), None, _)) => {
                                        warn!("聚合命令不得重复请求反馈");
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
                                                    buf.reserve(buf.capacity() * 2);
                                                    unsafe {
                                                        buf.set_len(buf.capacity());
                                                    }
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
                                                            info!("聚合命令写入分区 {partition} 偏移 {offset}");
                                                        },
                                                    ) {
                                                    Ok(()) => {
                                                        rs.insert(com_id, (Some(res_tx), None, Instant::now()));
                                                    }
                                                    Err(e) => {
                                                        error!("聚合命令发送失败：{e}");
                                                        let _ = res_tx.send(e.response());
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                error!("聚合命令序列化错误：{e}");
                                                let _ = res_tx.send(e.response());
                                            }
                                        }
                                        pool.put(buf);
                                    }
                                }
                            }.instrument(span).await;
                        }
                        Some(Todo::Response { com_id, span, res }) => {
                            async {
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
                            }.instrument(span).await;
                        }
                        None => {
                            info!("发送端已关闭，响应处理器稍后将停止工作");
                            break;
                        }
                    }
                }
            }
        }
    }

    #[instrument(name = "aggregate_consume", skip(tc, tx, ready, notify))]
    async fn consume(
        agg_type: &'static str,
        tc: Arc<StreamConsumer>,
        tx: mpsc::UnboundedSender<Todo<A, C>>,
        ready: Arc<Notify>,
        notify: Arc<Notify>,
    ) {
        let notified = notify.notified();
        tokio::pin!(notified);
        ready.notify_one();
        loop {
            tokio::select! {
                biased;
                _ = &mut notified => {
                    info!("收到关闭信号，开始优雅退出");
                    break;
                }
                data = tc.recv() => match data {
                    Ok(msg) => {
                        match process_message(&msg).await {
                            Ok((agg_id, com_id, res)) => {
                                let span = info_span!(parent: None, "respond_command", agg_type, %agg_id, %com_id);
                                span.clone().in_scope(|| {
                                    if let Err(e) = tx.send(Todo::Response { com_id, span, res }) {
                                        error!("发送聚合命令反馈错误：{e}");
                                    }
                                });
                            }
                            Err(e) => error!("{e}"),
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
