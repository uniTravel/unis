//! # Kafka 订阅者

mod pool;
mod reader;
mod stream;
#[cfg(test)]
mod tests;
mod topic;

use crate::config::SubscriberConfig;
use ahash::RandomState;
use dashmap::DashMap;
use opentelemetry::{
    Context, SpanId, TraceFlags, TraceId,
    trace::{SpanContext, TraceContextExt},
};
use rdkafka::{
    ClientConfig, Message,
    consumer::{Consumer, StreamConsumer},
    message::BorrowedMessage,
};
use rkyv::{
    Archive, Deserialize,
    de::Pool,
    rancor::{Error, Strategy},
};
use std::{
    any::TypeId,
    marker::PhantomData,
    sync::{Arc, LazyLock},
};
use stream::Writer;
use tokio::sync::{Notify, mpsc};
use tracing::{error, info, warn};
use unis::{
    Com,
    aggregator::Aggregator,
    domain::{Aggregate, CommandEnum, Config, EventEnum},
};
use unis::{errors::UniError, subscriber::Subscriber};
use uuid::Uuid;

pub use unis::app::context;

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
            let cfg = SubscriberConfig::get();
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

#[inline(always)]
fn topic_agg(topic: &'static str, agg_id: Uuid) -> String {
    let mut topic_agg = String::with_capacity(topic.len() + 37);
    topic_agg.push_str(topic);
    topic_agg.push_str("-");
    topic_agg.push_str(&agg_id.to_string());
    topic_agg
}

struct TopicTask {
    pub topic: &'static str,
    pub agg_id: Uuid,
}

/// Kafka 订阅者结构
pub struct KafkaSubscriber<C>
where
    C::A: Aggregate,
    C: CommandEnum,
    <C as Archive>::Archived: Deserialize<C, Strategy<Pool, Error>>,
    C::E: EventEnum<A = C::A>,
    <C::E as Archive>::Archived: Deserialize<C::E, Strategy<Pool, Error>>,
{
    _marker: PhantomData<C>,
}

impl<A, C, E> Subscriber<A, C, E> for KafkaSubscriber<C>
where
    A: Aggregate,
    C: CommandEnum<A = A, E = E>,
    <C as Archive>::Archived: Deserialize<C, Strategy<Pool, Error>>,
    E: EventEnum<A = A>,
    <E as Archive>::Archived: Deserialize<E, Strategy<Pool, Error>>,
{
    async fn launch(ctx: &'static unis::app::Context) -> Result<(), String> {
        let cfg_subscriber = SubscriberConfig::get();
        let agg_type = A::type_name();
        let topic = A::topic();
        let cfg_name = agg_type.rsplit(".").next().ok_or("获取聚合名称失败")?;
        let settings = cfg_subscriber
            .cc
            .get(cfg_name)
            .ok_or("获取订阅者消费配置失败")?;
        let cfg = cfg_subscriber.subscriber.get(cfg_name);
        if cfg.retain < 4 {
            warn!(
                topic,
                "缓存保留时长不足 4 小时，请确认投影延迟不会超过该时长"
            );
        }
        let topic_com = A::topic_com();
        let mut config = ClientConfig::new();
        for (key, value) in settings {
            config.set(key, value);
        }
        config.set("bootstrap.servers", &cfg_subscriber.bootstrap);
        config.set("group.id", topic_com);
        let cc: Arc<StreamConsumer> = Arc::new(
            config
                .create()
                .map_err(|e| format!("订阅者消费创建失败：{e}"))?,
        );
        cc.subscribe(&[topic_com])
            .map_err(|e| format!("订阅聚合命令流失败：{e}"))?;
        info!(topic, "成功订阅聚合命令流");

        let (tx, rx) = mpsc::unbounded_channel::<Com<C>>();
        let stream = Arc::new(Writer::new(cfg).await);
        ctx.spawn(move |ready| {
            Aggregator::<A, C, E>::launch(
                topic,
                cfg,
                reader::load,
                stream,
                reader::restore,
                rx,
                ready,
            )
        })
        .await;
        ctx.spawn_notify(move |ready, notify| Self::consume(cc, tx, ready, notify))
            .await;
        Ok(())
    }
}

impl<A, C, E> KafkaSubscriber<C>
where
    A: Aggregate,
    C: CommandEnum<A = A, E = E>,
    <C as Archive>::Archived: Deserialize<C, Strategy<Pool, Error>>,
    E: EventEnum<A = A>,
    <E as Archive>::Archived: Deserialize<E, Strategy<Pool, Error>>,
{
    async fn consume(
        cc: Arc<StreamConsumer>,
        tx: mpsc::UnboundedSender<Com<C>>,
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
                data = cc.recv() => match data {
                    Ok(msg) => {
                        match Self::process_message(&msg) {
                            Ok(com) => {
                                let agg_id = com.agg_id;
                                if let Err(e) = tx.send(com) {
                                    error!(topic, %agg_id, error = ?e, "提交聚合命令失败");
                                }
                            }
                            Err(e) => error!(topic, error = ?e, "处理消息失败"),
                        }
                    }
                    Err(e) => error!(topic, error = ?e, "消息错误"),
                }
            }
        }
    }

    fn process_message(msg: &BorrowedMessage<'_>) -> Result<Com<C>, UniError> {
        let agg_id = crate::get_agg_key(msg)?;
        let headers = msg.headers().ok_or("消息头不存在")?;
        let com_id = crate::get_com_id(headers)?;
        let span_id = crate::get_span_id(headers)?;
        let trace_flags = crate::get_trace_flags(headers)?;
        let span_context = SpanContext::new(
            TraceId::from_bytes(com_id),
            SpanId::from_bytes(span_id),
            TraceFlags::new(trace_flags),
            false,
            Default::default(),
        );
        let cx = Context::new().with_remote_span_context(span_context);
        let com_data = msg.payload().ok_or("空消息体")?;
        Ok(Com {
            agg_id,
            com_id,
            span_id,
            cx,
            com: C::from_bytes(com_data)?,
        })
    }
}
