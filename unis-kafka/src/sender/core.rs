use crate::{config::SenderConfig, sender::app::App};
use ahash::AHashMap;
use rdkafka::{
    ClientConfig, Message,
    consumer::{Consumer, StreamConsumer},
    message::{BorrowedMessage, Header, Headers, OwnedHeaders},
    producer::{FutureProducer, FutureRecord, future_producer::Delivery},
};
use rkyv::{
    Archive, Deserialize,
    de::Pool,
    rancor::{Error, Strategy},
    ser::allocator::Arena,
};
use std::{
    marker::PhantomData,
    sync::{Arc, LazyLock},
};
use tokio::{
    sync::{Notify, mpsc, oneshot},
    time::{Duration, Instant, MissedTickBehavior, interval_at},
};
use tracing::{Span, debug, error, info, info_span, instrument};
use unis::{
    UniResponse,
    config::SendConfig,
    domain::{Aggregate, CommandEnum, Config, EventEnum, Request},
    errors::UniError,
};
use uuid::Uuid;

static SENDER_CONFIG: LazyLock<SenderConfig> = LazyLock::new(|| SenderConfig::get());

static SHARED_CP: LazyLock<Arc<FutureProducer>> = LazyLock::new(|| {
    let mut config = ClientConfig::new();
    config.set("bootstrap.servers", &SENDER_CONFIG.bootstrap);
    Arc::new(config.create().expect("共享的聚合命令生产者创建失败"))
});

static CP_CONFIG: LazyLock<ClientConfig> = LazyLock::new(|| {
    let mut config = ClientConfig::new();
    for (key, value) in &SENDER_CONFIG.cp {
        config.set(key, value);
    }
    config.set("bootstrap.servers", &SENDER_CONFIG.bootstrap);
    config
});

enum Todo<A, C, E>
where
    A: Aggregate,
    C: CommandEnum<A = A, E = E>,
    <C as Archive>::Archived: Deserialize<C, Strategy<Pool, Error>>,
    E: EventEnum<A = A>,
    <E as Archive>::Archived: Deserialize<E, Strategy<Pool, Error>>,
{
    Reply {
        agg_id: Uuid,
        com_id: Uuid,
        com: C,
        res_tx: oneshot::Sender<UniResponse>,
    },
    Response {
        com_id: Uuid,
        res: UniResponse,
    },
}

/// 发送者结构
pub struct Sender<C>
where
    C::A: Aggregate,
    C: CommandEnum,
    <C as Archive>::Archived: Deserialize<C, Strategy<Pool, Error>>,
    C::E: EventEnum<A = C::A>,
    <C::E as Archive>::Archived: Deserialize<C::E, Strategy<Pool, Error>>,
{
    agg_type: &'static str,
    tx: mpsc::UnboundedSender<Todo<C::A, C, C::E>>,
    _marker: PhantomData<C>,
}

impl<A, C, E> Request<A, C, E> for Sender<C>
where
    A: Aggregate,
    C: CommandEnum<A = A, E = E>,
    <C as Archive>::Archived: Deserialize<C, Strategy<Pool, Error>>,
    E: EventEnum<A = A>,
    <E as Archive>::Archived: Deserialize<E, Strategy<Pool, Error>>,
{
    #[instrument(name = "send_command", skip_all, fields(agg_type = self.agg_type, %agg_id, %com_id))]
    async fn send(&self, agg_id: Uuid, com_id: Uuid, com: C) -> UniResponse {
        let (res_tx, res_rx) = oneshot::channel::<UniResponse>();
        if let Err(e) = self.tx.send(Todo::Reply {
            agg_id,
            com_id,
            com,
            res_tx,
        }) {
            error!("聚合命令请求反馈错误：{e}");
            panic!("响应处理器停止工作");
        }

        info!("发送聚合命令");
        match res_rx.await {
            Ok(res) => {
                info!("聚合命令收到反馈：{res}");
                res
            }
            Err(e) => {
                error!("聚合命令接收反馈错误：{e}");
                UniResponse::Timeout
            }
        }
    }
}

impl<A, C, E> Sender<C>
where
    A: Aggregate,
    C: CommandEnum<A = A, E = E> + Sync,
    <C as Archive>::Archived: Deserialize<C, Strategy<Pool, Error>>,
    E: EventEnum<A = A>,
    <E as Archive>::Archived: Deserialize<E, Strategy<Pool, Error>>,
{
    /// 构造发送者
    #[instrument(name = "build_sender", skip_all, fields(agg_type))]
    pub async fn new(context: Arc<App>) -> Result<Self, String> {
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
            true => Arc::new(CP_CONFIG.create().expect("聚合命令生产者创建失败")),
            false => SHARED_CP.clone(),
        };
        info!("成功创建 {topic} 聚合命令生产者");

        let mut config = ClientConfig::new();
        for (key, value) in settings {
            config.set(key, value);
        }
        config.set("bootstrap.servers", &SENDER_CONFIG.bootstrap);
        config.set("group.id", format!("{agg_type}-{}", SENDER_CONFIG.hostname));
        let tc: Arc<StreamConsumer> = Arc::new(config.create().expect("发送者消费创建失败"));
        tc.subscribe(&[agg_type]).expect("订阅聚合类型事件流失败");
        info!("成功订阅聚合类型事件流");

        let (tx, rx) = mpsc::unbounded_channel::<Todo<A, C, E>>();
        context
            .spawn_notify(move |ready, notify| {
                Self::respond(agg_type, producer, topic, cfg, rx, ready, notify)
            })
            .await;
        let tx_clone = tx.clone();
        context
            .spawn_notify(move |ready, notify| Self::consume(agg_type, tc, tx_clone, ready, notify))
            .await;

        Ok(Self {
            agg_type,
            tx,
            _marker: PhantomData,
        })
    }

    #[instrument(
        name = "aggregate_respond",
        skip(producer, topic, cfg, rx, ready, notify)
    )]
    async fn respond(
        agg_type: &'static str,
        producer: Arc<FutureProducer>,
        topic: &'static str,
        cfg: SendConfig,
        mut rx: mpsc::UnboundedReceiver<Todo<A, C, E>>,
        ready: Arc<Notify>,
        notify: Arc<Notify>,
    ) {
        let mut arena = Arena::new();
        let mut rs: AHashMap<
            Uuid,
            (
                Option<oneshot::Sender<UniResponse>>,
                Option<UniResponse>,
                Instant,
            ),
        > = AHashMap::new();
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
                data = rx.recv() => match data {
                    Some(Todo::Reply { agg_id, com_id, com, res_tx }) => match rs.get_mut(&com_id) {
                        Some((Some(_), None, _)) => {
                            let _ = res_tx.send(UniResponse::Duplicate);
                        }
                        Some((None, Some(_), _)) => {
                            if let Some((_, Some(res), _)) = rs.remove(&com_id) {
                                let _ = res_tx.send(res);
                            }
                        }
                        Some(_) => error!("请求反馈进入非法处理分支"),
                        None => match com.to_bytes(&mut arena) {
                            Ok(bytes) => {
                                let record = FutureRecord::to(topic)
                                    .payload(bytes.as_slice())
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
                                            debug!("聚合 {agg_id} 命令 {com_id} 写入分区 {partition} 偏移 {offset}");
                                        },
                                    ) {
                                    Ok(()) => {
                                        rs.insert(com_id, (Some(res_tx), None, Instant::now()));
                                    }
                                    Err(e) => {
                                        let _ = res_tx.send(e.response());
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = res_tx.send(e.response());
                            }
                        }
                    }
                    Some(Todo::Response { com_id, res }) => match rs.get_mut(&com_id) {
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
                    None => {
                        info!("发送端已关闭，响应处理器稍后将停止工作");
                        break;
                    }
                }
            }
        }
    }

    #[instrument(name = "aggregate_consume", skip(tc, tx, ready, notify))]
    async fn consume(
        agg_type: &'static str,
        tc: Arc<StreamConsumer>,
        tx: mpsc::UnboundedSender<Todo<A, C, E>>,
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
                    Ok(msg) => match process_message(&msg) {
                        Ok((agg_id, com_id, res)) => {
                            let span = info_span!(parent: None, "respond_command", agg_type, %agg_id, %com_id);
                            span.clone().in_scope(|| {
                                if let Err(e) = tx.send(Todo::Response { com_id, res }) {
                                    error!("发送聚合命令反馈错误：{e}");
                                }
                            });
                        }
                        Err(e) => error!("{e}"),
                    }
                    Err(e) => error!("消息错误：{e}"),
                }
            }
        }
    }
}

fn process_message(msg: &BorrowedMessage<'_>) -> Result<(Uuid, Uuid, UniResponse), UniError> {
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
    let res = UniResponse::from_bytes(res_data);
    debug!("提取命令处理结果：{:?}", res);

    Ok((agg_id, com_id, res))
}
