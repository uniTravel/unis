use super::pool::ConsumerPool;
use crate::config::SubscriberConfig;
use ahash::{AHashMap, AHashSet};
use rdkafka::{Message, Offset, TopicPartitionList, consumer::Consumer};
use rkyv::{
    Archive, Deserialize,
    de::Pool,
    rancor::{Error, Strategy},
};
use std::{
    sync::LazyLock,
    time::{Duration, SystemTime},
};
use tracing::{debug, error, info};
use unis::{
    UniResponse,
    domain::{Config, EventEnum},
    errors::UniError,
};
use uuid::Uuid;

static POOL: LazyLock<ConsumerPool> = LazyLock::new(|| ConsumerPool::new());

pub async fn load<E>(
    topic: &'static str,
    agg_id: Uuid,
    checkpoint: [u8; 8],
) -> Result<Vec<([u8; 16], E)>, UniError>
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

    info!(topic_agg, "开始加载事件流数据");
    let mut msgs = Vec::new();
    if checkpoint == [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF] {
        loop {
            match consumer.poll(Duration::from_secs(2)) {
                Some(Ok(msg)) => {
                    let payload = msg.payload().ok_or("消息体不存在")?;
                    let com_id = crate::get_com_key(&msg)?;
                    msgs.push((com_id, E::from_bytes(payload)?));
                }
                Some(Err(e)) => {
                    debug!(topic_agg, error = ?e, "事件流数据错误");
                    return Err(UniError::ReadError(e.to_string()));
                }
                None => {
                    if msgs.len() == 0 {
                        debug!(topic_agg, "聚合事件流数据不存在");
                        return Err(UniError::ReadError("聚合事件流数据不存在".to_owned()));
                    }
                    info!(topic_agg, "读取 {} 条事件流数据", msgs.len());
                    break;
                }
            }
        }
    } else {
        loop {
            match consumer.poll(SubscriberConfig::get().timeout) {
                Some(Ok(msg)) => {
                    let payload = msg.payload().ok_or("消息体不存在")?;
                    let com_id = crate::get_com_key(&msg)?;
                    let headers = msg.headers().ok_or("消息头不存在")?;
                    let revision = crate::get_revision(headers)?;
                    msgs.push((com_id, E::from_bytes(payload)?));
                    if revision == checkpoint {
                        info!(topic_agg, "完整读取 {} 条事件流数据", msgs.len());
                        break;
                    }
                }
                Some(Err(e)) => {
                    debug!(topic_agg, error = ?e, "事件流数据错误");
                    return Err(UniError::ReadError(e.to_string()));
                }
                None => {
                    if msgs.len() == 0 {
                        debug!(topic_agg, "聚合事件流数据不存在");
                        return Err(UniError::ReadError("聚合事件流数据不存在".to_owned()));
                    }
                    debug!(topic_agg, "未能完整读取聚合事件流数据");
                    return Err(UniError::ReadError("未能完整读取聚合事件流数据".to_owned()));
                }
            }
        }
    }

    Ok(msgs)
}

pub(crate) async fn restore(
    topic: &'static str,
    latest: i64,
) -> Result<AHashMap<Uuid, ([u8; 8], AHashSet<[u8; 16]>)>, UniError> {
    debug!(topic, "开始恢复最近 {latest} 小时的命令操作记录");
    let cfg_subscriber = SubscriberConfig::get();
    let mut agg_coms: AHashMap<Uuid, ([u8; 8], AHashSet<[u8; 16]>)> = AHashMap::new();
    let mut tpl = TopicPartitionList::new();
    let mut watermarks = AHashMap::new();

    let guard = POOL.get()?;
    let consumer = guard.into_inner();

    let metadata = consumer
        .fetch_metadata(Some(topic), cfg_subscriber.timeout)
        .map_err(|e| UniError::ReadError(e.to_string()))?;
    let start_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| UniError::ReadError(e.to_string()))?
        .as_millis() as i64
        - (latest * 60 * 60 * 1000);

    for partition in metadata.topics()[0].partitions() {
        let pid = partition.id();
        let mut seek_tpl = TopicPartitionList::new();
        seek_tpl
            .add_partition_offset(topic, pid, Offset::Offset(start_time))
            .map_err(|e| UniError::ReadError(e.to_string()))?;
        let offset = if let Some(tp) = consumer
            .offsets_for_times(seek_tpl, cfg_subscriber.timeout)
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
            .fetch_watermarks(topic, pid, cfg_subscriber.timeout)
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
        match consumer.poll(cfg_subscriber.timeout) {
            Some(Ok(msg)) => {
                let agg_id = crate::get_agg_key(&msg)?;
                let headers = msg.headers().ok_or("消息头不存在")?;
                let res = crate::get_response(headers)?;

                if res == UniResponse::Success {
                    let com_id = crate::get_com_id(headers)?;
                    let revision = crate::get_revision(headers)?;
                    if let Some((cp, coms)) = agg_coms.get_mut(&agg_id) {
                        *cp = revision;
                        coms.insert(com_id);
                    } else {
                        let mut coms = AHashSet::new();
                        coms.insert(com_id);
                        agg_coms.insert(agg_id, (revision, coms));
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
