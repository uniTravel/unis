//! Stream的Kafka实现

use crate::{
    BINCODE_HEADER,
    aggregator::Res,
    domain::{Config, Stream},
    errors::DomainError,
    kafka::config::SubscriberConfig,
};
use ahash::AHashSet;
use bincode::encode_into_slice;
use rdkafka::{
    ClientConfig, Message, TopicPartitionList,
    admin::AdminClient,
    client::DefaultClientContext,
    consumer::{BaseConsumer, Consumer, StreamConsumer},
    message::{Header, OwnedHeaders},
    producer::{FutureProducer, FutureRecord, future_producer::Delivery},
    util::Timeout,
};
use std::{sync::LazyLock, time::Duration};
use tracing::debug;
use uuid::Uuid;

static TP: LazyLock<FutureProducer> = LazyLock::new(|| {
    let cfg_root = SubscriberConfig::get().expect("获取订阅者配置失败");
    let cfg = &cfg_root.tp;
    let mut config = ClientConfig::new();
    config.set("bootstrap.servers", &cfg_root.bootstrap);
    for (key, value) in cfg {
        config.set(key, value);
    }
    config.create().expect("聚合类型生产者创建失败")
});

static AC: LazyLock<BaseConsumer> = LazyLock::new(|| {
    let cfg_root = SubscriberConfig::get().expect("获取订阅者配置失败");
    ClientConfig::new()
        .set("bootstrap.servers", &cfg_root.bootstrap)
        .set("auto.offset.reset", "earliest")
        .set("enable.auto.commit", "false")
        .set("isolation.level", "read_committed")
        .create()
        .expect("聚合消费者创建失败")
});

static RC: LazyLock<StreamConsumer> = LazyLock::new(|| {
    let cfg_root = SubscriberConfig::get().expect("获取订阅者配置失败");
    ClientConfig::new()
        .set("bootstrap.servers", &cfg_root.bootstrap)
        .create()
        .expect("恢复命令操作记录消费者创建失败")
});

static ADMIN: LazyLock<AdminClient<DefaultClientContext>> = LazyLock::new(|| {
    let cfg_root = SubscriberConfig::get().expect("获取订阅者配置失败");
    ClientConfig::new()
        .set("bootstrap.servers", &cfg_root.bootstrap)
        .create()
        .expect("恢复命令操作记录消费者创建失败")
});

/// Stream的Kafka实现
pub struct KafkaStream;

impl KafkaStream {
    /// 初始化
    pub fn init() {
        LazyLock::force(&TP);
        LazyLock::force(&AC);
    }
}

impl Stream for KafkaStream {
    async fn write(
        agg_type: &'static str,
        agg_id: Uuid,
        com_id: Uuid,
        _revision: u64,
        evt_data: &[u8],
    ) -> Result<(), DomainError> {
        let mut buf = [0u8; 1];
        encode_into_slice(Res::Success, &mut buf, BINCODE_HEADER)?;
        let record = FutureRecord::to(agg_type)
            .payload(evt_data)
            .key(agg_id.as_bytes())
            .headers(
                OwnedHeaders::new_with_capacity(2)
                    .insert(Header {
                        key: "com_id",
                        value: Some(com_id.as_bytes()),
                    })
                    .insert(Header {
                        key: "res",
                        value: Some(&buf),
                    }),
            );

        TP.send(record, Timeout::Never)
            .await
            .map_err(|(e, _)| DomainError::WriteError(e.to_string()))
            .map(
                |Delivery {
                     partition,
                     offset,
                     timestamp: _,
                 }| {
                    debug!("聚合{agg_id}命令{com_id}的事件写入分区{partition}偏移{offset}")
                },
            )
    }

    async fn respond(
        agg_type: &'static str,
        agg_id: Uuid,
        com_id: Uuid,
        res: Res,
        evt_data: &[u8],
    ) -> Result<(), DomainError> {
        let mut buf = [0u8; 1];
        encode_into_slice(res, &mut buf, BINCODE_HEADER)?;
        let record = FutureRecord::to(agg_type)
            .payload(evt_data)
            .key(agg_id.as_bytes())
            .headers(
                OwnedHeaders::new_with_capacity(2)
                    .insert(Header {
                        key: "com_id",
                        value: Some(com_id.as_bytes()),
                    })
                    .insert(Header {
                        key: "res",
                        value: Some(&buf),
                    }),
            );

        TP.send(record, Timeout::Never)
            .await
            .map_err(|(e, _)| DomainError::WriteError(e.to_string()))
            .map(
                |Delivery {
                     partition,
                     offset,
                     timestamp: _,
                 }| {
                    debug!("聚合{agg_id}命令{com_id}的执行错误写入分区{partition}偏移{offset}")
                },
            )
    }

    fn read(agg_type: &'static str, agg_id: Uuid) -> Result<Vec<Vec<u8>>, DomainError> {
        let mut topic = String::with_capacity(agg_type.len() + 37);
        topic.push_str(agg_type);
        topic.push_str("-");
        topic.push_str(&agg_id.to_string());

        let mut tpl = TopicPartitionList::new();
        tpl.add_partition_offset(&topic, 0, rdkafka::Offset::Beginning)
            .map_err(|e| DomainError::ReadError(e.to_string()))?;
        AC.assign(&tpl)
            .map_err(|e| DomainError::ReadError(e.to_string()))?;

        let (low, high) = AC
            .fetch_watermarks(&topic, 0, Duration::from_millis(100))
            .map_err(|e| DomainError::ReadError(e.to_string()))?;

        if low == -1 || high == -1 {
            return Err(DomainError::ReadError("数据为空".to_string()));
        }

        let mut msgs = Vec::new();
        let mut current_offset = low;
        while current_offset < high {
            match AC.poll(Duration::from_millis(100)) {
                Some(Ok(msg)) => {
                    if let Some(payload) = msg.payload() {
                        msgs.push(payload.to_vec());
                        current_offset = msg.offset();
                    }
                }
                Some(Err(e)) => return Err(DomainError::ReadError(e.to_string())),
                None => (),
            }
        }
        Ok(msgs)
    }

    fn restore(
        agg_type: &'static str,
        com_set: &mut AHashSet<Uuid>,
        com_vec: &mut std::collections::VecDeque<Uuid>,
        count: usize,
    ) {
        let partitions: Vec<i32> = ADMIN
            .inner()
            .fetch_metadata(Some(agg_type), Duration::from_millis(100))
            .unwrap()
            .topics()
            .iter()
            .find(|t| t.name() == agg_type)
            .unwrap()
            .partitions()
            .iter()
            .map(|p| p.id())
            .collect();

        todo!()
    }
}
