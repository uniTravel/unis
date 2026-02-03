use super::ProjectError;
use crate::config::{load_bootstrap, load_hostname};
use ahash::AHashMap;
use rdkafka::{
    ClientConfig, Message, TopicPartitionList,
    consumer::{BaseConsumer, Consumer, ConsumerGroupMetadata},
    error::KafkaError,
    message::{BorrowedMessage, Headers},
    producer::{BaseRecord, DefaultProducerContext, Producer, ThreadedProducer},
};
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use tracing::{debug, info};
use unis::{UniResponse, domain::Aggregate, errors::UniError};
use uuid::Uuid;

pub(super) struct Projector {
    bootstrap: String,
    transaction_id: String,
    capacity: usize,
    partitions: usize,
    interval: u64,
    ap: Option<ThreadedProducer<DefaultProducerContext>>,
    tc: BaseConsumer,
}

fn create_producer(
    bootstrap: &str,
    transaction_id: &str,
) -> Result<ThreadedProducer<DefaultProducerContext>, KafkaError> {
    let ap: ThreadedProducer<DefaultProducerContext> = ClientConfig::new()
        .set("bootstrap.servers", bootstrap)
        .set("transactional.id", transaction_id)
        .create()?;
    ap.init_transactions(Duration::from_secs(30))?;
    Ok(ap)
}

fn create_consumer(bootstrap: &str, group_id: &str) -> Result<BaseConsumer, KafkaError> {
    ClientConfig::new()
        .set("bootstrap.servers", bootstrap)
        .set("group.id", group_id)
        .set("enable.auto.commit", "false")
        .set("auto.offset.reset", "earliest")
        .set("isolation.level", "read_committed")
        .create()
}

impl Projector {
    pub(super) fn new(group_id: String) -> Self {
        let bootstrap = load_bootstrap(&super::CONFIG);
        let hostname = load_hostname(&super::CONFIG);
        let transaction_id = format!("{group_id}-{hostname}");
        let ap = create_producer(&bootstrap, &transaction_id).expect("初创投影生产者失败");
        let tc = create_consumer(&bootstrap, &group_id).expect("初创投影消费者失败");
        Self {
            bootstrap,
            transaction_id,
            capacity: 100,
            partitions: 10,
            interval: 50,
            ap: Some(ap),
            tc,
        }
    }

    pub fn rebuild(&mut self) -> Result<(), ProjectError> {
        let ap = create_producer(&self.bootstrap, &self.transaction_id)?;
        self.ap = Some(ap);
        Ok(())
    }

    pub fn subscribe<A>(&self)
    where
        A: Aggregate,
    {
        self.tc
            .subscribe(&[A::topic()])
            .expect("订阅聚合类型事件流失败");
    }

    pub fn launch(&mut self) -> Result<(), ProjectError> {
        let mut msgs = AHashMap::with_capacity(self.partitions);
        let mut offsets = AHashMap::with_capacity(self.capacity);
        let mut last_flush = Instant::now();
        let mut count: usize = 0;
        let cgm = self
            .tc
            .group_metadata()
            .ok_or(ProjectError::MetadataError)?;
        let ap = self.ap.take().ok_or(ProjectError::ProducerNotFound)?;
        info!("成功初始化投影者事务");

        loop {
            if super::EXIT.load(Ordering::SeqCst) {
                if !msgs.is_empty() {
                    process_batch(&ap, &cgm, &mut msgs, &mut offsets, "优雅退出")?;
                }
                break Ok(());
            }

            if !msgs.is_empty() && last_flush.elapsed() > Duration::from_millis(self.interval) {
                process_batch(&ap, &cgm, &mut msgs, &mut offsets, "触及提交间隔阈值")?;
                last_flush = Instant::now();
                count = 0;
            }

            match self.tc.poll(Duration::from_millis(100)) {
                Some(Ok(msg)) => match process_message(&msg) {
                    Ok((agg_id, payload, res)) if res == UniResponse::Success => {
                        let agg_type = msg.topic().to_string();
                        let mut topic = String::with_capacity(agg_type.len() + 37);
                        topic.push_str(&agg_type);
                        topic.push_str("-");
                        topic.push_str(&agg_id.to_string());
                        let partition = msg.partition();
                        let offset = msg.offset();

                        match msgs.get_mut(&topic) {
                            Some(payloads) => payloads.push(payload),
                            None => {
                                if msgs.len() == self.partitions {
                                    process_batch(
                                        &ap,
                                        &cgm,
                                        &mut msgs,
                                        &mut offsets,
                                        "触及分区数阈值",
                                    )?;
                                    last_flush = Instant::now();
                                    count = 0;
                                }
                                msgs.insert(topic, vec![payload]);
                            }
                        }

                        let key = (agg_type, partition);
                        match offsets.get_mut(&key) {
                            Some(max_offset) => *max_offset = offset,
                            None => {
                                offsets.insert(key, offset);
                            }
                        }

                        count += 1;
                        if count == self.capacity {
                            process_batch(&ap, &cgm, &mut msgs, &mut offsets, "触及提交计数阈值")?;
                            last_flush = Instant::now();
                            count = 0;
                        }
                    }
                    Ok(_) => continue,
                    Err(e) => break Err(ProjectError::UniError(e)),
                },
                Some(Err(e)) => break Err(ProjectError::KafkaError(e)),
                None => continue,
            }
        }
    }
}

fn process_batch(
    ap: &ThreadedProducer<DefaultProducerContext>,
    cgm: &ConsumerGroupMetadata,
    msgs: &mut AHashMap<String, Vec<Vec<u8>>>,
    offsets: &mut AHashMap<(String, i32), i64>,
    reason: &str,
) -> Result<(), KafkaError> {
    info!("{reason}，提交批量投影");
    let msg_vec: Vec<(String, Vec<Vec<u8>>)> = msgs.drain().collect();
    let offset_vec: Vec<((String, i32), i64)> = offsets.drain().collect();

    ap.begin_transaction()?;

    for (topic, payloads) in msg_vec {
        for payload in payloads {
            let record: BaseRecord<'_, (), Vec<u8>> = BaseRecord::to(&topic).payload(&payload);
            if let Err(e) = ap.send(record).map_err(|(e, _)| e) {
                ap.abort_transaction(Duration::from_secs(30))?;
                return Err(e);
            }
        }
    }

    let mut offsets = TopicPartitionList::new();
    for ((topic, partition), offset) in offset_vec {
        offsets.add_partition_offset(&topic, partition, rdkafka::Offset::Offset(offset + 1))?;
    }
    if let Err(e) = ap.send_offsets_to_transaction(&offsets, cgm, Duration::from_secs(30)) {
        ap.abort_transaction(Duration::from_secs(30))?;
        return Err(e);
    }

    if let Err(e) = ap.commit_transaction(Duration::from_secs(30)) {
        ap.abort_transaction(Duration::from_secs(30))?;
        return Err(e);
    }

    info!("完成批量投影");
    Ok(())
}

fn process_message(msg: &BorrowedMessage<'_>) -> Result<(Uuid, Vec<u8>, UniResponse), UniError> {
    let key = msg.key().ok_or("消息键不存在")?;
    let agg_id = Uuid::from_slice(key).map_err(|e| UniError::MsgError(e.to_string()))?;
    debug!("提取聚合Id：{agg_id}");

    let payload = msg.payload().ok_or("err")?.to_vec();
    let headers = msg.headers().ok_or("消息头不存在")?;

    let res_data = headers
        .iter()
        .find(|h| h.key == "response")
        .ok_or("键为'response'的消息头不存在")?
        .value
        .ok_or("键'response'对应的值为空")?;
    let res = UniResponse::from_bytes(res_data);
    debug!("提取命令处理结果：{res}");

    Ok((agg_id, payload, res))
}
