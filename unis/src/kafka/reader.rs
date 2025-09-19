//! 从Kafka读取数据

use crate::{
    errors::DomainError,
    kafka::{pool::ConsumerPool, subscriber::SUBSCRIBER_CONFIG},
};
use ahash::{AHashMap, AHashSet};
use futures::StreamExt;
use rdkafka::{Message, TopicPartitionList, consumer::Consumer, message::Headers};
use std::{collections::VecDeque, sync::LazyLock, time::SystemTime};
use uuid::Uuid;

static POOL: LazyLock<ConsumerPool> = LazyLock::new(|| ConsumerPool::new());

///
pub async fn load(agg_type: &'static str, agg_id: Uuid) -> Result<Vec<Vec<u8>>, DomainError> {
    let mut topic = String::with_capacity(agg_type.len() + 37);
    topic.push_str(agg_type);
    topic.push_str("-");
    topic.push_str(&agg_id.to_string());

    let mut tpl = TopicPartitionList::new();
    tpl.add_partition_offset(&topic, 0, rdkafka::Offset::Beginning)
        .map_err(|e| DomainError::ReadError(e.to_string()))?;
    let guard = POOL.get()?;
    let consumer = guard.into_inner();
    consumer
        .assign(&tpl)
        .map_err(|e| DomainError::ReadError(e.to_string()))?;

    let (low, high) = consumer
        .fetch_watermarks(&topic, 0, SUBSCRIBER_CONFIG.timeout)
        .map_err(|e| DomainError::ReadError(e.to_string()))?;

    if low == -1 || high == -1 {
        return Err(DomainError::ReadError("数据为空".to_string()));
    }

    let mut msgs = Vec::new();
    let message_stream = consumer.stream();
    tokio::pin!(message_stream);

    while let Some(msg) = message_stream.next().await {
        match msg {
            Ok(msg) => {
                if let Some(payload) = msg.payload() {
                    msgs.push(payload.to_vec());
                }
                if msg.offset() == high {
                    break;
                }
            }
            Err(e) => return Err(DomainError::ReadError(e.to_string())),
        }
    }

    Ok(msgs)
}

///
pub async fn restore(
    agg_type: &'static str,
    latest: i64,
) -> Result<(AHashSet<Uuid>, VecDeque<Uuid>), DomainError> {
    let mut com_set: AHashSet<Uuid> = AHashSet::new();
    let mut com_vec: VecDeque<Uuid> = VecDeque::new();
    let mut tpl = TopicPartitionList::new();
    let mut watermarks = AHashMap::new();
    let guard = POOL.get()?;
    let consumer = guard.into_inner();
    let metadata = consumer
        .fetch_metadata(Some(agg_type), SUBSCRIBER_CONFIG.timeout)
        .map_err(|e| DomainError::ReadError(e.to_string()))?;
    for partition in metadata.topics()[0].partitions() {
        let pid = partition.id();
        tpl.add_partition(agg_type, pid);
        let (_, high) = consumer
            .fetch_watermarks(agg_type, pid, SUBSCRIBER_CONFIG.timeout)
            .map_err(|e| DomainError::ReadError(e.to_string()))?;
        watermarks.insert(pid, high);
    }
    consumer
        .assign(&tpl)
        .map_err(|e| DomainError::ReadError(e.to_string()))?;

    let start_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| DomainError::ReadError(e.to_string()))?
        .as_millis() as i64
        - (latest * 60 * 1000);

    consumer
        .seek_partitions(
            consumer
                .offsets_for_timestamp(start_time, SUBSCRIBER_CONFIG.timeout)
                .map_err(|e| DomainError::ReadError(e.to_string()))?,
            SUBSCRIBER_CONFIG.timeout,
        )
        .map_err(|e| DomainError::ReadError(e.to_string()))?;

    let message_stream = consumer.stream();
    tokio::pin!(message_stream);

    while let Some(msg) = message_stream.next().await {
        let msg = msg.map_err(|e| DomainError::ReadError(e.to_string()))?;
        let id = msg
            .headers()
            .ok_or(DomainError::ReadError("消息头不存在".to_string()))?
            .iter()
            .find(|h| h.key == "com_id")
            .ok_or(DomainError::ReadError("'com_id'消息头不存在".to_string()))?
            .value
            .ok_or(DomainError::ReadError("'com_id'消息头值为空".to_string()))?;
        let com_id = Uuid::from_slice(id).map_err(|e| DomainError::ReadError(e.to_string()))?;
        com_set.insert(com_id);
        com_vec.push_back(com_id);
        let pid = msg.partition();
        if msg.offset() == watermarks[&pid] {
            watermarks.remove(&pid);
        }
        if watermarks.len() == 0 {
            break;
        }
    }

    Ok((com_set, com_vec))
}
