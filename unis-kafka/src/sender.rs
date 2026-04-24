//! # Kafka 发送者

use crate::config::SenderConfig;
use ahash::RandomState;
use dashmap::DashMap;
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
    any::TypeId,
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
    domain::Config,
    sender::{Sender, Todo},
};
use unis::{
    config::SendConfig,
    domain::{Aggregate, CommandEnum, EventEnum},
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

trait Topic: 'static {
    fn topic() -> &'static str;
    fn topic_com() -> &'static str;
}

impl<A> Topic for A
where
    A: Aggregate + 'static,
{
    fn topic() -> &'static str {
        static CACHE: LazyLock<DashMap<TypeId, &'static str, RandomState>> =
            LazyLock::new(|| DashMap::with_hasher(RandomState::new()));
        let type_id = TypeId::of::<A>();

        if let Some(entry) = CACHE.get(&type_id) {
            return &entry;
        }

        &CACHE.entry(type_id).or_insert_with(|| {
            let agg_type = A::type_name();
            let cfg_name = agg_type.rsplit(".").next().unwrap();
            let cfg = SENDER_CONFIG.sender.get(cfg_name);
            let topic = format!("{}.{}", cfg.key, agg_type);
            Box::leak(Box::new(topic))
        })
    }

    fn topic_com() -> &'static str {
        static CACHE: LazyLock<DashMap<TypeId, &'static str, RandomState>> =
            LazyLock::new(|| DashMap::with_hasher(RandomState::new()));
        let type_id = TypeId::of::<A>();

        if let Some(entry) = CACHE.get(&type_id) {
            return &entry;
        }

        &CACHE.entry(type_id).or_insert_with(|| {
            let topic_com = format!("{}-command", Self::topic());
            Box::leak(Box::new(topic_com))
        })
    }
}

/// Kafka 发送者结构
pub struct KafkaSender<C>
where
    C::A: Aggregate,
    C: CommandEnum,
    <C as Archive>::Archived: Deserialize<C, Strategy<Pool, Error>>,
    C::E: EventEnum<A = C::A>,
    <C::E as Archive>::Archived: Deserialize<C::E, Strategy<Pool, Error>>,
{
    tx: mpsc::UnboundedSender<Todo<C::A, C, C::E>>,
    _marker: PhantomData<C>,
}

impl<A, C, E> Sender<A, C, E> for KafkaSender<C>
where
    A: Aggregate,
    C: CommandEnum<A = A, E = E>,
    <C as Archive>::Archived: Deserialize<C, Strategy<Pool, Error>>,
    E: EventEnum<A = A>,
    <E as Archive>::Archived: Deserialize<E, Strategy<Pool, Error>>,
{
    #[inline(always)]
    fn topic(&self) -> &'static str {
        A::topic()
    }

    #[inline(always)]
    fn send(&self, todo: Todo<A, C, E>) -> Result<(), mpsc::error::SendError<Todo<A, C, E>>> {
        self.tx.send(todo)
    }

    #[instrument(name = "build_sender", skip_all, fields(topic))]
    async fn new(ctx: &'static unis::app::Context) -> Result<Self, String> {
        let agg_type = A::type_name();
        let topic = A::topic();
        Span::current().record("topic", topic);
        let cfg_name = agg_type.rsplit(".").next().ok_or("获取聚合名称失败")?;
        let settings = SENDER_CONFIG
            .tc
            .get(cfg_name)
            .ok_or("获取发送者消费配置失败")?;
        let cfg = SENDER_CONFIG.sender.get(cfg_name);
        let topic_com = A::topic_com();
        let producer = match cfg.hotspot {
            true => Arc::new(
                CP_CONFIG
                    .create()
                    .map_err(|e| format!("聚合命令生产者创建失败：{e}"))?,
            ),
            false => SHARED_CP.clone(),
        };
        info!("成功创建 {topic_com} 聚合命令生产者");

        let mut config = ClientConfig::new();
        for (key, value) in settings {
            config.set(key, value);
        }
        config.set("bootstrap.servers", &SENDER_CONFIG.bootstrap);
        config.set("group.id", format!("{topic}-{}", SENDER_CONFIG.hostname));
        let tc: Arc<StreamConsumer> = Arc::new(
            config
                .create()
                .map_err(|e| format!("发送者消费创建失败：{e}"))?,
        );
        tc.subscribe(&[topic])
            .map_err(|e| format!("订阅聚合类型事件流失败：{e}"))?;
        info!("成功订阅聚合类型事件流");

        let (tx, rx) = mpsc::unbounded_channel::<Todo<A, C, E>>();
        ctx.spawn_notify(move |ready, notify| {
            Self::respond(topic, producer, topic_com, cfg, rx, ready, notify)
        })
        .await;
        let tx_clone = tx.clone();
        ctx.spawn_notify(move |ready, notify| Self::consume(topic, tc, tx_clone, ready, notify))
            .await;

        Ok(Self {
            tx,
            _marker: PhantomData,
        })
    }
}

impl<A, C, E> KafkaSender<C>
where
    A: Aggregate,
    C: CommandEnum<A = A, E = E>,
    <C as Archive>::Archived: Deserialize<C, Strategy<Pool, Error>>,
    E: EventEnum<A = A>,
    <E as Archive>::Archived: Deserialize<E, Strategy<Pool, Error>>,
{
    #[instrument(
        name = "aggregate_respond",
        skip(producer, topic_com, cfg, rx, ready, notify)
    )]
    async fn respond(
        topic: &'static str,
        producer: Arc<FutureProducer>,
        topic_com: &'static str,
        cfg: SendConfig,
        mut rx: mpsc::UnboundedReceiver<Todo<A, C, E>>,
        ready: Arc<Notify>,
        notify: Arc<Notify>,
    ) {
        let mut arena = Arena::new();
        let rs: Arc<
            DashMap<
                Uuid,
                (
                    Option<oneshot::Sender<Result<Vec<u8>, UniResponse>>>,
                    Option<Result<Vec<u8>, UniResponse>>,
                    Instant,
                ),
                RandomState,
            >,
        > = Arc::new(DashMap::with_hasher(RandomState::new()));
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
                    rs.retain(|_, (_, _, t)| t.elapsed() < Duration::from_secs(cfg.retain));
                }
                data = rx.recv() => match data {
                    Some(Todo::Reply { agg_id, com_id, com, res_tx }) => match rs.remove(&com_id) {
                        Some((_, (Some(rep), None, _))) => {
                            let _ = rep.send(Err(UniResponse::Conflict));
                            rs.insert(com_id, (Some(res_tx), None, Instant::now()));
                        }
                        Some((_, (None, Some(res), _))) => {
                            let _ = res_tx.send(res);
                        }
                        Some(_) => error!("请求反馈进入非法处理分支"),
                        None => match com.to_bytes(&mut arena) {
                            Ok(bytes) => {
                                let producer = Arc::clone(&producer);
                                let rs = Arc::clone(&rs);
                                tokio::spawn(async move {
                                    let record = FutureRecord::to(topic_com)
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
                                            let _ = res_tx.send(Err(e.response()));
                                        }
                                    }
                                });
                            }
                            Err(e) => {
                                let _ = res_tx.send(Err(e.response()));
                            }
                        }
                    }
                    Some(Todo::Response { com_id, res }) => match rs.remove(&com_id) {
                        Some((_, (Some(res_tx), None, _))) => {
                            let _ = res_tx.send(res);
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
        topic: &'static str,
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
                            let span = info_span!(parent: None, "respond_command", topic, %agg_id, %com_id);
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

fn process_message(
    msg: &BorrowedMessage<'_>,
) -> Result<(Uuid, Uuid, Result<Vec<u8>, UniResponse>), UniError> {
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

    match res {
        UniResponse::Success => Ok((
            agg_id,
            com_id,
            Ok(msg.payload().ok_or("消息体不存在")?.to_vec()),
        )),
        res => Ok((agg_id, com_id, Err(res))),
    }
}

/// 为聚合命令构造处理器
#[macro_export]
macro_rules! com_handler {
    ($func_name:ident, $c:ty, $com:ty, $variant:ident) => {
        pub async fn $func_name<F>(
            axum::Extension(key): axum::Extension<UniKey>,
            State(svc): State<Arc<crate::KafkaSender<$c>>>,
            UniCommand(com, lang, _): UniCommand<$com, F>,
        ) -> Result<Vec<u8>, (axum::http::StatusCode, String)> {
            svc.apply(key.agg_id, key.com_id, <$c>::$variant(com), &lang)
                .await
        }
    };
}
