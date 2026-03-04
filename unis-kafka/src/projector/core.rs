use super::{PROJECTOR_CONFIG, ProjectError};
use ahash::AHashMap;
use rdkafka::{
    ClientConfig, Message, TopicPartitionList,
    consumer::{Consumer, StreamConsumer},
    error::KafkaError,
    message::{BorrowedMessage, Headers},
    producer::{FutureProducer, FutureRecord, Producer, future_producer::Delivery},
};
use std::sync::Arc;
use tokio::{
    sync::Notify,
    time::{Duration, Instant, sleep},
};
use tracing::{debug, error, info};
use unis::{UniResponse, errors::UniError};
use uuid::Uuid;

fn create_producer() -> Result<FutureProducer, KafkaError> {
    let transaction_id = format!("{}-{}", PROJECTOR_CONFIG.name, PROJECTOR_CONFIG.hostname);
    let mut config = ClientConfig::new();
    for (key, value) in &PROJECTOR_CONFIG.pp {
        config.set(key, value);
    }

    let ap: FutureProducer = config
        .set("bootstrap.servers", &PROJECTOR_CONFIG.bootstrap)
        .set("transactional.id", transaction_id)
        .create()?;
    ap.init_transactions(std::time::Duration::from_secs(30))?;
    Ok(ap)
}

fn create_consumer() -> Result<StreamConsumer, KafkaError> {
    let mut config = ClientConfig::new();
    for (key, value) in &PROJECTOR_CONFIG.pc {
        config.set(key, value);
    }
    config
        .set("bootstrap.servers", &PROJECTOR_CONFIG.bootstrap)
        .set("group.id", &PROJECTOR_CONFIG.name)
        .create()
}

pub async fn launch(topics: Vec<&'static str>, ready: Arc<Notify>, notify: Arc<Notify>) {
    let mut count = 0;
    let tc = create_consumer().expect("创建投影消费者失败");
    let mut ap = create_producer().expect("初创投影生产者失败");
    loop {
        match process(&topics, &ap, &tc, Arc::clone(&ready), Arc::clone(&notify)).await {
            Ok(()) => break,
            Err(ProjectError::UniError(e)) => {
                error!("投影处理错误：{:?}", e);
                break;
            }
            Err(ProjectError::MetadataError) => error!("获取消费组元数据失败"),
            Err(ProjectError::KafkaError(e)) => {
                error!("投影处理错误：{:?}", e);
                ap = create_producer().expect("重建投影生产者失败");
            }
        }
        count += 1;
        if count == 15 {
            error!("尝试 {count} 次仍然失败，退出应用！");
            break;
        }
        sleep(Duration::from_secs(PROJECTOR_CONFIG.secs)).await;
    }
}

async fn process(
    topics: &Vec<&'static str>,
    ap: &FutureProducer,
    tc: &StreamConsumer,
    ready: Arc<Notify>,
    notify: Arc<Notify>,
) -> Result<(), ProjectError> {
    tc.subscribe(topics)?;
    let mut msgs = AHashMap::with_capacity(PROJECTOR_CONFIG.partitions);
    let mut offsets = AHashMap::with_capacity(PROJECTOR_CONFIG.capacity);
    let mut last_flush = Instant::now();
    let mut interval = tokio::time::interval(Duration::from_millis(1));
    let mut count: usize = 0;
    info!("成功初始化投影者事务");

    let notified = notify.notified();
    tokio::pin!(notified);
    ready.notify_one();
    loop {
        tokio::select! {
            biased;
            _ = &mut notified => {
                info!("收到关闭信号，开始优雅退出");
                if !msgs.is_empty() {
                    process_batch(ap, tc, &mut msgs, &mut offsets, "优雅退出").await?;
                }
                break Ok(());
            }
            _ = interval.tick() => {
                if !msgs.is_empty() && last_flush.elapsed() > Duration::from_millis(PROJECTOR_CONFIG.interval) {
                    process_batch(ap, tc, &mut msgs, &mut offsets, "触及提交间隔阈值").await?;
                    last_flush = Instant::now();
                    count = 0;
                }
            }
            data = tc.recv() => match data {
                Ok(msg) => match process_message(&msg) {
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
                                if msgs.len() == PROJECTOR_CONFIG.partitions {
                                    process_batch(ap, tc, &mut msgs, &mut offsets, "触及分区数阈值").await?;
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
                        if count == PROJECTOR_CONFIG.capacity {
                            process_batch(ap, tc, &mut msgs, &mut offsets, "触及提交计数阈值").await?;
                            last_flush = Instant::now();
                            count = 0;
                        }
                    }
                    Ok(_) => continue,
                    Err(e) => break Err(ProjectError::UniError(e)),
                }
                Err(e) => break Err(ProjectError::KafkaError(e)),
            }
        }
    }
}

async fn process_batch(
    ap: &FutureProducer,
    tc: &StreamConsumer,
    msgs: &mut AHashMap<String, Vec<Vec<u8>>>,
    offsets: &mut AHashMap<(String, i32), i64>,
    reason: &str,
) -> Result<(), ProjectError> {
    info!("{reason}，提交批量投影");
    let cgm = tc.group_metadata().ok_or(ProjectError::MetadataError)?;
    let msg_vec: Vec<(String, Vec<Vec<u8>>)> = msgs.drain().collect();
    let offset_vec: Vec<((String, i32), i64)> = offsets.drain().collect();
    let mut delivery_futures = Vec::with_capacity(msg_vec.len());

    ap.begin_transaction()?;

    for (topic, payloads) in msg_vec {
        for payload in payloads {
            let record: FutureRecord<'_, (), Vec<u8>> = FutureRecord::to(&topic).payload(&payload);
            match ap.send_result(record) {
                Ok(delevery_future) => {
                    delivery_futures.push(delevery_future);
                }
                Err((e, _)) => {
                    ap.abort_transaction(Duration::from_secs(30))?;
                    return Err(e.into());
                }
            }
        }
    }

    for fut in delivery_futures {
        match fut.await {
            Ok(Ok(Delivery {
                partition,
                offset,
                timestamp: _,
            })) => {
                debug!("转存的事件写到分区 {partition} 偏移 {offset}")
            }
            Ok(Err((e, _))) => {
                ap.abort_transaction(Duration::from_secs(30))?;
                return Err(e.into());
            }
            Err(_) => {
                ap.abort_transaction(Duration::from_secs(30))?;
                return Err(KafkaError::Canceled.into());
            }
        }
    }

    let mut offsets = TopicPartitionList::new();
    for ((topic, partition), offset) in offset_vec {
        offsets.add_partition_offset(&topic, partition, rdkafka::Offset::Offset(offset + 1))?;
    }
    if let Err(e) = ap.send_offsets_to_transaction(&offsets, &cgm, Duration::from_secs(30)) {
        ap.abort_transaction(Duration::from_secs(30))?;
        return Err(e.into());
    }

    debug!("提交事务");
    if let Err(e) = ap.commit_transaction(Duration::from_secs(30)) {
        ap.abort_transaction(Duration::from_secs(30))?;
        return Err(e.into());
    }

    info!("完成批量投影");
    Ok(())
}

fn process_message(msg: &BorrowedMessage<'_>) -> Result<(Uuid, Vec<u8>, UniResponse), UniError> {
    let key = msg.key().ok_or("消息键不存在")?;
    let agg_id = Uuid::from_slice(key).map_err(|e| UniError::MsgError(e.to_string()))?;
    debug!("提取聚合Id：{agg_id} , 偏移：{}", msg.offset());

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
