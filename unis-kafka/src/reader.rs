//! Kafka 数据读取

use crate::{pool::ConsumerPool, subscriber::SUBSCRIBER_CONFIG};
use ahash::{AHashMap, AHashSet};
use rdkafka::{Message, Offset, TopicPartitionList, consumer::Consumer, message::Headers};
use std::{sync::LazyLock, time::SystemTime};
use tracing::{debug, error};
use unis::errors::UniError;
use uuid::Uuid;

static POOL: LazyLock<ConsumerPool> = LazyLock::new(|| ConsumerPool::new());

/// 加载聚合事件流
pub async fn load(agg_type: &'static str, agg_id: Uuid) -> Result<Vec<Vec<u8>>, UniError> {
    let mut topic = String::with_capacity(agg_type.len() + 37);
    topic.push_str(agg_type);
    topic.push_str("-");
    topic.push_str(&agg_id.to_string());

    let mut tpl = TopicPartitionList::new();
    tpl.add_partition_offset(&topic, 0, rdkafka::Offset::Beginning)
        .map_err(|e| UniError::ReadError(e.to_string()))?;
    let guard = POOL.get()?;
    let consumer = guard.into_inner();
    consumer
        .assign(&tpl)
        .map_err(|e| UniError::ReadError(e.to_string()))?;

    let (low, high) = consumer
        .fetch_watermarks(&topic, 0, SUBSCRIBER_CONFIG.timeout)
        .map_err(|e| UniError::ReadError(e.to_string()))?;

    if low == -1 || high == -1 {
        return Err(UniError::ReadError("数据为空".to_owned()));
    }

    debug!("开始从聚合 {topic} 读取事件流数据");
    let mut msgs = Vec::new();
    loop {
        match consumer.poll(SUBSCRIBER_CONFIG.timeout) {
            Some(Ok(msg)) => {
                if let Some(payload) = msg.payload() {
                    msgs.push(payload.to_vec());
                }
                if msg.offset() == high {
                    debug!("读到聚合 {topic} {} 条事件流数据", msgs.len());
                    break;
                }
            }
            Some(Err(e)) => {
                debug!("聚合 {topic} 事件流数据错误：{e}");
                return Err(UniError::ReadError(e.to_string()));
            }
            None => {
                debug!("结束聚合 {topic} 事件流数据读取");
                return Err(UniError::ReadError("聚合事件流未读到数据".to_string()));
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

    let guard = POOL.get()?;
    let consumer = guard.into_inner();

    let metadata = consumer
        .fetch_metadata(Some(agg_type), SUBSCRIBER_CONFIG.timeout)
        .map_err(|e| UniError::ReadError(e.to_string()))?;
    let start_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| UniError::ReadError(e.to_string()))?
        .as_millis() as i64
        - (latest * 60 * 1000);

    for partition in metadata.topics()[0].partitions() {
        let pid = partition.id();
        let mut seek_tpl = TopicPartitionList::new();
        seek_tpl
            .add_partition_offset(agg_type, pid, Offset::Offset(start_time))
            .map_err(|e| UniError::ReadError(e.to_string()))?;
        let offset = if let Some(tp) = consumer
            .offsets_for_times(seek_tpl, SUBSCRIBER_CONFIG.timeout)
            .map_err(|e| UniError::ReadError(e.to_string()))?
            .elements()
            .first()
        {
            match tp.offset() {
                Offset::Offset(o) => {
                    debug!("类型 {agg_type} 分区 {pid}: 起始消费偏移 {o}");
                    Offset::Offset(o)
                }
                Offset::End => {
                    debug!("类型 {agg_type} 分区 {pid}: 时间戳在最晚消息之后，从最新位置消费");
                    Offset::End
                }
                Offset::Beginning => {
                    debug!("类型 {agg_type} 分区 {pid}: 时间戳在最早消息之前，从开始消费");
                    Offset::Beginning
                }
                Offset::Stored => {
                    debug!("类型 {agg_type} 分区 {pid}: 使用存储的偏移");
                    Offset::Stored
                }
                Offset::Invalid => {
                    debug!("类型 {agg_type} 分区 {pid}: 无效偏移，从开始消费");
                    Offset::Beginning
                }
                Offset::OffsetTail(c) => {
                    debug!("类型 {agg_type} 分区 {pid}: 回溯 {c} 条消息");
                    Offset::OffsetTail(c)
                }
            }
        } else {
            debug!("类型 {agg_type} 分区 {pid}: 未取得偏移，从最新位置消费");
            Offset::End
        };

        tpl.add_partition_offset(agg_type, pid, offset)
            .map_err(|e| UniError::ReadError(e.to_string()))?;
        let (low, high) = consumer
            .fetch_watermarks(agg_type, pid, SUBSCRIBER_CONFIG.timeout)
            .map_err(|e| UniError::ReadError(e.to_string()))?;
        debug!("类型 {agg_type} 分区 {pid} 水位：{low} ~ {high}");
        if offset != Offset::End {
            watermarks.insert(pid, high);
        }
    }

    consumer
        .assign(&tpl)
        .map_err(|e| UniError::ReadError(e.to_string()))?;

    debug!("开始从类型 {agg_type} 读取事件流数据");
    while watermarks.len() > 0 {
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
                debug!(
                    "类型 {agg_type} 分区 {pid}：偏移 {} 读到聚合 {agg_id}",
                    msg.offset()
                );
                if msg.offset() + 1 == watermarks[&pid] {
                    watermarks.remove(&pid);
                    debug!("消费到高水位，移除类型 {agg_type} 分区 {pid}");
                }
            }
            Some(Err(e)) => {
                error!("类型 {agg_type} 事件流数据错误：{e}");
                return Err(UniError::ReadError(e.to_string()));
            }
            None => {
                break;
            }
        }
    }
    debug!("结束类型 {agg_type} 事件流数据读取");
    Ok(agg_coms)
}
