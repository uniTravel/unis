use super::{SUBSCRIBER_CONFIG, pool::ConsumerPool};
use ahash::{AHashMap, AHashSet};
use rdkafka::{Message, Offset, TopicPartitionList, consumer::Consumer};
use rkyv::{
    Archive, Deserialize,
    de::Pool,
    rancor::{Error, Strategy},
};
use std::{sync::LazyLock, time::SystemTime};
use tracing::{debug, error};
use unis::{UniResponse, domain::EventEnum, errors::UniError};
use uuid::Uuid;

static POOL: LazyLock<ConsumerPool> = LazyLock::new(|| ConsumerPool::new());

pub async fn load<E>(topic: &'static str, agg_id: Uuid) -> Result<Vec<([u8; 16], E)>, UniError>
where
    E: EventEnum,
    <E as Archive>::Archived: Deserialize<E, Strategy<Pool, Error>>,
{
    let topic_agg = super::topic_agg(topic, agg_id);
    let mut tpl = TopicPartitionList::new();
    tpl.add_partition_offset(&topic_agg, 0, rdkafka::Offset::Beginning)
        .map_err(|e| UniError::ReadError(e.to_string()))?;
    let guard = POOL.get()?;
    let consumer = guard.into_inner();
    consumer
        .assign(&tpl)
        .map_err(|e| UniError::ReadError(e.to_string()))?;

    let (low, high) = consumer
        .fetch_watermarks(&topic_agg, 0, SUBSCRIBER_CONFIG.timeout)
        .map_err(|e| UniError::ReadError(e.to_string()))?;

    if low == -1 || high == -1 {
        return Err(UniError::ReadError("未能获取聚合事件流水位数据".to_owned()));
    }

    debug!(topic_agg, "开始读取事件流数据");
    let mut msgs = Vec::new();
    loop {
        match consumer.poll(SUBSCRIBER_CONFIG.timeout) {
            Some(Ok(msg)) => {
                let payload = msg.payload().ok_or("消息体不存在")?;
                let com_id = crate::get_com_key(&msg)?;
                msgs.push((com_id, E::from_bytes(payload)?));

                if msg.offset() + 1 == high {
                    debug!(topic_agg, "读到 {} 条事件流数据", msgs.len());
                    break;
                }
            }
            Some(Err(e)) => {
                debug!(topic_agg, error = ?e, "事件流数据错误");
                return Err(UniError::ReadError(e.to_string()));
            }
            None => {
                debug!(topic_agg, "结束事件流数据读取");
                return Err(UniError::ReadError("未能读取聚合事件流数据".to_owned()));
            }
        }
    }

    Ok(msgs)
}

pub(crate) async fn restore(
    topic: &'static str,
    latest: i64,
) -> Result<AHashMap<Uuid, AHashSet<[u8; 16]>>, UniError> {
    debug!(topic, "开始恢复最近 {latest} 分钟的命令操作记录");
    let mut agg_coms: AHashMap<Uuid, AHashSet<[u8; 16]>> = AHashMap::new();
    let mut tpl = TopicPartitionList::new();
    let mut watermarks = AHashMap::new();

    let guard = POOL.get()?;
    let consumer = guard.into_inner();

    let metadata = consumer
        .fetch_metadata(Some(topic), SUBSCRIBER_CONFIG.timeout)
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
            .add_partition_offset(topic, pid, Offset::Offset(start_time))
            .map_err(|e| UniError::ReadError(e.to_string()))?;
        let offset = if let Some(tp) = consumer
            .offsets_for_times(seek_tpl, SUBSCRIBER_CONFIG.timeout)
            .map_err(|e| UniError::ReadError(e.to_string()))?
            .elements()
            .first()
        {
            match tp.offset() {
                Offset::Offset(o) => {
                    debug!(topic, "分区 {pid}：起始消费偏移 {o}");
                    Offset::Offset(o)
                }
                Offset::End => {
                    debug!(topic, "分区 {pid}: 时间戳在最晚消息之后，从最新位置消费");
                    Offset::End
                }
                Offset::Beginning => {
                    debug!(topic, "分区 {pid}: 时间戳在最早消息之前，从开始消费");
                    Offset::Beginning
                }
                Offset::Stored => {
                    debug!(topic, "分区 {pid}: 使用存储的偏移");
                    Offset::Stored
                }
                Offset::Invalid => {
                    debug!(topic, "分区 {pid}: 无效偏移，从开始消费");
                    Offset::Beginning
                }
                Offset::OffsetTail(c) => {
                    debug!(topic, "分区 {pid}: 回溯 {c} 条消息");
                    Offset::OffsetTail(c)
                }
            }
        } else {
            debug!(topic, "分区 {pid}: 未取得偏移，从最新位置消费");
            Offset::End
        };

        tpl.add_partition_offset(topic, pid, offset)
            .map_err(|e| UniError::ReadError(e.to_string()))?;
        let (low, high) = consumer
            .fetch_watermarks(topic, pid, SUBSCRIBER_CONFIG.timeout)
            .map_err(|e| UniError::ReadError(e.to_string()))?;
        debug!(topic, "分区 {pid} 水位：{low} ~ {high}");
        if offset != Offset::End {
            watermarks.insert(pid, high);
        }
    }

    consumer
        .assign(&tpl)
        .map_err(|e| UniError::ReadError(e.to_string()))?;

    debug!(topic, "开始读取事件流数据");
    while watermarks.len() > 0 {
        match consumer.poll(SUBSCRIBER_CONFIG.timeout) {
            Some(Ok(msg)) => {
                let agg_id = crate::get_agg_key(&msg)?;
                let headers = msg.headers().ok_or("消息头不存在")?;
                let com_id = crate::get_com_id(headers)?;
                let res = crate::get_response(headers)?;

                if res == UniResponse::Success {
                    if let Some(coms) = agg_coms.get_mut(&agg_id) {
                        coms.insert(com_id);
                    } else {
                        let mut coms = AHashSet::new();
                        coms.insert(com_id);
                        agg_coms.insert(agg_id, coms);
                    }
                }

                let pid = msg.partition();
                debug!(topic, %agg_id, "分区 {pid}：偏移 {} 读到聚合", msg.offset());
                if msg.offset() + 1 == watermarks[&pid] {
                    watermarks.remove(&pid);
                    debug!(topic, "消费到高水位，移除分区 {pid}");
                }
            }
            Some(Err(e)) => {
                error!(topic, "事件流数据错误：{e}");
                return Err(UniError::ReadError(e.to_string()));
            }
            None => {
                break;
            }
        }
    }
    debug!(topic, "结束事件流数据读取");
    Ok(agg_coms)
}
