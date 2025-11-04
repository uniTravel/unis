//! Kafka 数据读取

use crate::{pool::ConsumerPool, subscriber::SUBSCRIBER_CONFIG};
use ahash::{AHashMap, AHashSet};
use rdkafka::{Message, TopicPartitionList, consumer::Consumer, message::Headers};
use std::{sync::LazyLock, time::SystemTime};
use tracing::debug;
use unis::errors::UniError;
use uuid::Uuid;

static POOL: LazyLock<ConsumerPool> = LazyLock::new(|| ConsumerPool::new());

/// 加载事件流
pub async fn load(agg_type: &'static str, agg_id: Uuid) -> Result<Vec<Vec<u8>>, UniError> {
    let mut topic = String::with_capacity(agg_type.len() + 37);
    topic.push_str(agg_type);
    topic.push_str("-");
    topic.push_str(&agg_id.to_string());

    let mut tpl = TopicPartitionList::new();
    tpl.add_partition_offset(&topic, 0, rdkafka::Offset::Beginning)
        .map_err(|e| UniError::ReadError(e.to_string()))?;
    debug!("从消费者池获取消费者");
    let guard = POOL.get()?;
    let consumer = guard.into_inner();
    debug!("开始将消费者指派到给{topic}主题");
    consumer
        .assign(&tpl)
        .map_err(|e| UniError::ReadError(e.to_string()))?;

    let (low, high) = consumer
        .fetch_watermarks(&topic, 0, SUBSCRIBER_CONFIG.timeout)
        .map_err(|e| UniError::ReadError(e.to_string()))?;

    if low == -1 || high == -1 {
        return Err(UniError::ReadError("数据为空".to_owned()));
    }

    debug!("从聚合事件流{topic}读取数据");
    let mut msgs = Vec::new();
    loop {
        match consumer.poll(SUBSCRIBER_CONFIG.timeout) {
            Some(Ok(msg)) => {
                if let Some(payload) = msg.payload() {
                    msgs.push(payload.to_vec());
                }
                if msg.offset() == high {
                    break;
                }
            }
            Some(Err(e)) => {
                debug!("聚合事件流{topic}数据错误：{e}");
                return Err(UniError::ReadError(e.to_string()));
            }
            None => {
                debug!("聚合事件流{topic}未取到数据");
                return Err(UniError::ReadError("聚合事件流未取到数据".to_string()));
            }
        }
    }

    Ok(msgs)
}

pub(crate) async fn restore(
    agg_type: &'static str,
    latest: i64,
) -> Result<AHashMap<Uuid, AHashSet<Uuid>>, UniError> {
    let mut agg_coms: AHashMap<Uuid, AHashSet<Uuid>> = AHashMap::new();
    let mut tpl = TopicPartitionList::new();
    let mut watermarks = AHashMap::new();

    debug!("从消费者池获取消费者");
    let guard = POOL.get()?;
    let consumer = guard.into_inner();

    let metadata = consumer
        .fetch_metadata(Some(agg_type), SUBSCRIBER_CONFIG.timeout)
        .map_err(|e| UniError::ReadError(e.to_string()))?;
    for partition in metadata.topics()[0].partitions() {
        let pid = partition.id();
        tpl.add_partition(agg_type, pid);
        let (_, high) = consumer
            .fetch_watermarks(agg_type, pid, SUBSCRIBER_CONFIG.timeout)
            .map_err(|e| UniError::ReadError(e.to_string()))?;
        watermarks.insert(pid, high);
    }
    debug!("开始将消费者指派到给{agg_type}主题");
    consumer
        .assign(&tpl)
        .map_err(|e| UniError::ReadError(e.to_string()))?;

    let start_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| UniError::ReadError(e.to_string()))?
        .as_millis() as i64
        - (latest * 60 * 1000);

    consumer
        .seek_partitions(
            consumer
                .offsets_for_timestamp(start_time, SUBSCRIBER_CONFIG.timeout)
                .map_err(|e| UniError::ReadError(e.to_string()))?,
            SUBSCRIBER_CONFIG.timeout,
        )
        .map_err(|e| UniError::ReadError(e.to_string()))?;

    debug!("从聚合类型事件流读取数据");
    loop {
        match consumer.poll(SUBSCRIBER_CONFIG.timeout) {
            Some(Ok(msg)) => {
                let key = msg
                    .key()
                    .ok_or("消息键不存在")
                    .map_err(|e| UniError::ReadError(e.to_owned()))?;
                let agg_id =
                    Uuid::from_slice(key).map_err(|e| UniError::ReadError(e.to_string()))?;
                let id = msg
                    .headers()
                    .ok_or(UniError::ReadError("消息头不存在".to_owned()))?
                    .iter()
                    .find(|h| h.key == "com_id")
                    .ok_or(UniError::ReadError("'com_id'消息头不存在".to_owned()))?
                    .value
                    .ok_or(UniError::ReadError("'com_id'消息头值为空".to_owned()))?;
                let com_id =
                    Uuid::from_slice(id).map_err(|e| UniError::ReadError(e.to_string()))?;
                if let Some(coms) = agg_coms.get_mut(&agg_id) {
                    coms.insert(com_id);
                } else {
                    let mut coms = AHashSet::new();
                    coms.insert(com_id);
                    agg_coms.insert(agg_id, coms);
                }
                let pid = msg.partition();
                if msg.offset() == watermarks[&pid] {
                    watermarks.remove(&pid);
                }
                if watermarks.len() == 0 {
                    break;
                }
            }
            Some(Err(e)) => {
                debug!("聚合类型事件流{agg_type}数据错误：{e}");
                return Err(UniError::ReadError(e.to_string()));
            }
            None => {
                debug!("聚合类型事件流{agg_type}未取到数据");
                break;
            }
        }
    }

    Ok(agg_coms)
}
