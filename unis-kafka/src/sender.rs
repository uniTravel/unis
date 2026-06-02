//! # Kafka 发送者

use crate::config::SenderConfig;
use ahash::RandomState;
use dashmap::DashMap;
use opentelemetry::trace::TraceContextExt;
use rdkafka::{
    ClientConfig, Message,
    consumer::{Consumer, StreamConsumer},
    message::{BorrowedMessage, Header, OwnedHeaders},
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
use tracing::{Instrument, debug, error, info, info_span, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt;
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

static SHARED_CP: LazyLock<Arc<FutureProducer>> = LazyLock::new(|| {
    let mut config = ClientConfig::new();
    config.set("bootstrap.servers", &SenderConfig::get().bootstrap);
    Arc::new(config.create().expect("共享的聚合命令生产者创建失败"))
});

static CP_CONFIG: LazyLock<ClientConfig> = LazyLock::new(|| {
    let mut config = ClientConfig::new();
    for (key, value) in SenderConfig::get().cp.clone() {
        config.set(key, value);
    }
    config.set("bootstrap.servers", &SenderConfig::get().bootstrap);
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
            let cfg = SenderConfig::get();
            let topic = format!("{}.{}", cfg.name, agg_type);
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

    async fn new(ctx: &'static unis::app::Context) -> Result<Self, String> {
        let cfg_sender = SenderConfig::get();
        let agg_type = A::type_name();
        let topic = A::topic();
        let cfg_name = agg_type.rsplit(".").next().ok_or("获取聚合名称失败")?;
        let settings = cfg_sender
            .tc
            .get(cfg_name)
            .ok_or("获取发送者消费配置失败")?;
        let cfg = cfg_sender.sender.get(cfg_name);
        let producer = match cfg.hotspot {
            true => Arc::new(
                CP_CONFIG
                    .create()
                    .map_err(|e| format!("聚合命令生产者创建失败：{e}"))?,
            ),
            false => SHARED_CP.clone(),
        };
        info!(topic, "成功创建聚合命令生产者");

        let mut config = ClientConfig::new();
        for (key, value) in settings {
            config.set(key, value);
        }
        config.set("bootstrap.servers", &cfg_sender.bootstrap);
        config.set("group.id", format!("{topic}-{}", cfg_sender.hostname));
        let tc: Arc<StreamConsumer> = Arc::new(
            config
                .create()
                .map_err(|e| format!("发送者消费创建失败：{e}"))?,
        );
        tc.subscribe(&[topic])
            .map_err(|e| format!("订阅聚合类型事件流失败：{e}"))?;
        info!(topic, "成功订阅聚合类型事件流");

        let (tx, rx) = mpsc::unbounded_channel::<Todo<A, C, E>>();
        ctx.spawn_notify(move |ready, notify| Self::respond(producer, cfg, rx, ready, notify))
            .await;
        let tx_clone = tx.clone();
        ctx.spawn_notify(move |ready, notify| Self::consume(tc, tx_clone, ready, notify))
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
    async fn respond(
        producer: Arc<FutureProducer>,
        cfg: &SendConfig,
        mut rx: mpsc::UnboundedReceiver<Todo<A, C, E>>,
        ready: Arc<Notify>,
        notify: Arc<Notify>,
    ) {
        let topic = A::topic();
        let topic_com = A::topic_com();
        let mut arena = Arena::new();
        let rs: Arc<
            DashMap<
                [u8; 16],
                (
                    Option<oneshot::Sender<Result<Vec<u8>, UniResponse>>>,
                    Option<Result<Vec<u8>, UniResponse>>,
                    Instant,
                ),
                RandomState,
            >,
        > = Arc::new(DashMap::with_hasher(RandomState::new()));
        let start = Instant::now();
        let mut interval = interval_at(start, Duration::from_mins(cfg.interval));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let notified = notify.notified();
        tokio::pin!(notified);
        ready.notify_one();
        loop {
            tokio::select! {
                biased;
                _ = &mut notified => {
                    info!(topic, "收到关闭信号，开始优雅退出");
                    break;
                }
                _ = interval.tick() => {
                    rs.retain(|_, (_, _, t)| t.elapsed() < Duration::from_mins(cfg.retain));
                }
                data = rx.recv() => match data {
                    Some(Todo::Reply { agg_id, com_id, cx, com, res_tx }) => {
                        let sp = info_span!("commit");
                        let _ = sp.set_parent(cx.clone());
                        sp.in_scope(|| {
                            info!(topic, %agg_id, "开始提交命令");
                            match rs.remove(&com_id) {
                                Some((_, (Some(rep), None, _))) => {
                                    warn!(topic, %agg_id, "已有相同的积压命令");
                                    if let Err(_) = rep.send(Err(UniResponse::Conflict)) {
                                        error!(topic, %agg_id, "命令结果反馈通道已关闭");
                                    }
                                    rs.insert(com_id, (Some(res_tx), None, Instant::now()));
                                    info!(topic, %agg_id, "以新的命令反馈通道替换原有");
                                }
                                Some((_, (None, Some(res), _))) => {
                                    info!(topic, %agg_id, "已有命令结果");
                                    if let Err(_) = res_tx.send(res) {
                                        error!(topic, %agg_id, "命令结果反馈通道已关闭");
                                    }
                                }
                                Some(_) => error!(topic, %agg_id, "请求反馈进入非法处理分支"),
                                None => match com.to_bytes(&mut arena) {
                                    Ok(bytes) => {
                                        let producer = Arc::clone(&producer);
                                        let rs = Arc::clone(&rs);
                                        let otel_span = cx.span();
                                        let span_id = otel_span.span_context().span_id().to_bytes();
                                        let trace_flags = otel_span.span_context().trace_flags().to_u8();
                                        tokio::spawn(async move {
                                            let sp = info_span!("send");
                                            let _ = sp.set_parent(cx);
                                            async {
                                                let record = FutureRecord::to(topic_com)
                                                    .payload(bytes.as_slice())
                                                    .key(agg_id.as_bytes())
                                                    .headers(OwnedHeaders::new_with_capacity(3)
                                                        .insert(Header {
                                                            key: "com_id",
                                                            value: Some(&com_id),
                                                        })
                                                        .insert(Header {
                                                            key: "span_id",
                                                            value: Some(&span_id),
                                                        })
                                                        .insert(Header {
                                                            key: "trace_flags",
                                                            value: Some(&[trace_flags]),
                                                        }));
                                                match producer
                                                    .send(record, SenderConfig::get().timeout)
                                                    .await
                                                    .map_err(|(e, _)| UniError::SendError(e.to_string()))
                                                    .map(
                                                        |Delivery {
                                                            partition,
                                                            offset,
                                                            timestamp: _timestamp,
                                                        }| {
                                                            debug!(topic, %agg_id, "命令写入分区 {partition} 偏移 {offset}");
                                                        },
                                                    ) {
                                                    Ok(()) => {
                                                        rs.insert(com_id, (Some(res_tx), None, Instant::now()));
                                                        info!(topic, %agg_id, "命令已写入 Kafka");
                                                    }
                                                    Err(e) => {
                                                        error!(topic, %agg_id, error = ?e, "命令写入 Kafka 失败");
                                                        if let Err(_) = res_tx.send(Err(e.response())) {
                                                            error!(topic, %agg_id, "命令结果反馈通道已关闭");
                                                        }
                                                    }
                                                }
                                            }.instrument(sp).await;
                                        });
                                    }
                                    Err(e) => {
                                        error!(topic, %agg_id, error = ?e, "命令序列化失败");
                                        if let Err(_) = res_tx.send(Err(e.response())) {
                                            error!(topic, %agg_id, "命令结果反馈通道已关闭");
                                        }
                                    }
                                }
                            }
                        });
                    }
                    Some(Todo::Response { agg_id, com_id, res }) => {
                        match rs.remove(&com_id) {
                            Some((_, (Some(res_tx), None, _))) => {
                                if let Err(_) = res_tx.send(res) {
                                    error!(topic, %agg_id, "命令结果反馈通道已关闭");
                                }
                            }
                            Some(_) => error!(topic, %agg_id, "发送反馈进入非法处理分支"),
                            None => {
                                rs.insert(com_id, (None, Some(res), Instant::now()));
                            }
                        }
                    }
                    None => {
                        info!(topic, "发送端已关闭，响应处理器稍后将停止工作");
                        break;
                    }
                }
            }
        }
    }

    async fn consume(
        tc: Arc<StreamConsumer>,
        tx: mpsc::UnboundedSender<Todo<A, C, E>>,
        ready: Arc<Notify>,
        notify: Arc<Notify>,
    ) {
        let topic = A::topic();
        let notified = notify.notified();
        tokio::pin!(notified);
        ready.notify_one();
        loop {
            tokio::select! {
                biased;
                _ = &mut notified => {
                    info!(topic, "收到关闭信号，开始优雅退出");
                    break;
                }
                data = tc.recv() => match data {
                    Ok(msg) => match process_message(&msg) {
                        Ok((agg_id, com_id, res)) => {
                            if let Err(e) = tx.send(Todo::Response { agg_id, com_id, res }) {
                                error!(topic, %agg_id, error = ?e, "发送聚合命令反馈错误");
                            }
                        }
                        Err(e) => error!(topic, error = ?e, "处理消息失败"),
                    }
                    Err(e) => error!(topic, error = ?e, "消息错误"),
                }
            }
        }
    }
}

fn process_message(
    msg: &BorrowedMessage<'_>,
) -> Result<(Uuid, [u8; 16], Result<Vec<u8>, UniResponse>), UniError> {
    let agg_id = crate::get_agg_key(msg)?;
    let headers = msg.headers().ok_or("消息头不存在")?;
    let com_id = crate::get_com_id(headers)?;
    let res = crate::get_response(headers)?;

    match res {
        UniResponse::Success => Ok((
            agg_id,
            com_id,
            Ok(msg.payload().ok_or("消息体不存在")?.to_vec()),
        )),
        res => Ok((agg_id, com_id, Err(res))),
    }
}
