//! # Kafka 投影者

use crate::config::ProjectorConfig;
use ahash::{AHashMap, RandomState};
use dashmap::DashMap;
use rdkafka::{
    ClientConfig, Message, TopicPartitionList,
    consumer::{Consumer, StreamConsumer},
    error::KafkaError,
    message::BorrowedMessage,
    producer::{FutureProducer, FutureRecord, Producer, future_producer::Delivery},
};
use std::{
    any::TypeId,
    sync::{
        Arc, LazyLock,
        atomic::{AtomicBool, Ordering},
    },
};
use thiserror::Error;
use tokio::{
    sync::Notify,
    task::JoinSet,
    time::{Duration, Instant, sleep},
};
use tracing::{Instrument, debug, error, info, instrument};
use unis::{UniResponse, create_span, domain::Config, errors::UniError};
use uuid::Uuid;

pub use unis::app::context;
pub use unis::domain::Aggregate;

static PROJECTOR_CONFIG: LazyLock<ProjectorConfig> = LazyLock::new(|| ProjectorConfig::get());

/// 聚合主题特征
pub trait Topic: 'static {
    /// 获取聚合类型主题
    fn topic() -> &'static str;
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
            let topic = format!("{}.{}", PROJECTOR_CONFIG.name, agg_type);
            Box::leak(Box::new(topic))
        })
    }
}

#[derive(Debug, Error)]
enum ProjectError {
    #[error("{0}")]
    UniError(#[from] unis::errors::UniError),
    #[error("无法获取消费组元数据")]
    MetadataError,
    #[error("{0}")]
    KafkaError(#[from] rdkafka::error::KafkaError),
    #[error("{0}")]
    JoinError(#[from] tokio::task::JoinError),
}

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

static INITIATED: AtomicBool = AtomicBool::new(false);

/// 启动投影
pub async fn launch(ctx: &'static unis::app::Context, topics: Vec<&'static str>) {
    if INITIATED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
        .is_ok()
    {
        ctx.spawn_notify(move |ready, notify| async move {
            let mut count = 0;
            let tc = create_consumer().expect("创建投影消费者失败");
            let mut ap = create_producer().expect("初创投影生产者失败");
            loop {
                match process(&topics, &ap, &tc, Arc::clone(&ready), Arc::clone(&notify)).await {
                    Ok(()) => break,
                    Err(ProjectError::UniError(e)) => {
                        error!(error = ?e, "投影处理错误");
                        break;
                    }
                    Err(ProjectError::MetadataError) => error!("获取消费组元数据失败"),
                    Err(ProjectError::KafkaError(e)) => {
                        error!(error = ?e, "投影处理错误");
                        ap = create_producer().expect("重建投影生产者失败");
                    }
                    Err(ProjectError::JoinError(e)) => {
                        error!(error = ?e, "投影处理错误");
                        ap = create_producer().expect("重建投影生产者失败");
                    }
                }
                count += 1;
                if count == PROJECTOR_CONFIG.tries {
                    error!("尝试 {count} 次仍然失败，退出应用！");
                    break;
                }
                sleep(Duration::from_secs(PROJECTOR_CONFIG.secs)).await;
            }
        })
        .await;
    } else {
        panic!("投影只能启动一次");
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
    let mut agg_msgs = AHashMap::with_capacity(PROJECTOR_CONFIG.partitions);
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
                if !agg_msgs.is_empty() {
                    process_batch(ap, tc, &mut agg_msgs, &mut offsets, "优雅退出").await?;
                }
                break Ok(());
            }
            _ = interval.tick() => {
                if !agg_msgs.is_empty() && last_flush.elapsed() > Duration::from_millis(PROJECTOR_CONFIG.interval) {
                    process_batch(ap, tc, &mut agg_msgs, &mut offsets, "触及提交间隔阈值").await?;
                    last_flush = Instant::now();
                    count = 0;
                }
            }
            data = tc.recv() => match data {
                Ok(msg) => match process_message(&msg) {
                    Ok((agg_id, com_id, span_id, payload, res)) if res == UniResponse::Success => {
                        let agg_type = msg.topic().to_string();
                        let mut topic = String::with_capacity(agg_type.len() + 37);
                        topic.push_str(&agg_type);
                        topic.push_str("-");
                        topic.push_str(&agg_id.to_string());
                        let partition = msg.partition();
                        let offset = msg.offset();

                        match agg_msgs.get_mut(&topic) {
                            Some(msgs) => msgs.push((com_id, span_id, payload)),
                            None => {
                                if agg_msgs.len() == PROJECTOR_CONFIG.partitions {
                                    process_batch(ap, tc, &mut agg_msgs, &mut offsets, "触及分区数阈值").await?;
                                    last_flush = Instant::now();
                                    count = 0;
                                }
                                agg_msgs.insert(topic, vec![(com_id, span_id, payload)]);
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
                            process_batch(ap, tc, &mut agg_msgs, &mut offsets, "触及提交计数阈值").await?;
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

#[instrument(name = "batch_project", skip_all)]
async fn process_batch(
    ap: &FutureProducer,
    tc: &StreamConsumer,
    agg_msgs: &mut AHashMap<String, Vec<([u8; 16], [u8; 8], Vec<u8>)>>,
    offsets: &mut AHashMap<(String, i32), i64>,
    reason: &str,
) -> Result<(), ProjectError> {
    info!("提交批量投影：{reason}");
    let tx_span = tracing::Span::current();
    let cgm = tc.group_metadata().ok_or(ProjectError::MetadataError)?;
    let msg_vec: Vec<(String, Vec<([u8; 16], [u8; 8], Vec<u8>)>)> = agg_msgs.drain().collect();
    let offset_vec: Vec<((String, i32), i64)> = offsets.drain().collect();
    let mut join_set = JoinSet::new();

    ap.begin_transaction()?;

    for (topic, msgs) in msg_vec {
        for (com_id, span_id, payload) in msgs {
            let sp = create_span("project_command", com_id, span_id);
            sp.follows_from(&tx_span);
            let record = FutureRecord::to(&topic).payload(&payload).key(&com_id);
            match ap.send_result(record) {
                Ok(future) => {
                    join_set.spawn(async { future.instrument(sp).await });
                }
                Err((e, _)) => {
                    ap.abort_transaction(Duration::from_secs(30))?;
                    join_set.abort_all();
                    return Err(e.into());
                }
            }
        }
    }

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok(Ok(Delivery {
                partition,
                offset,
                timestamp: _,
            }))) => {
                info!("转存的事件写到分区 {partition} 偏移 {offset}");
            }
            Ok(Ok(Err((e, _)))) => {
                error!(error = ?e, "消息投递失败，中止事务");
                ap.abort_transaction(Duration::from_secs(30))?;
                join_set.abort_all();
                return Err(e.into());
            }
            Ok(Err(e)) => {
                error!(error = ?e, "任务取消，中止事务");
                ap.abort_transaction(Duration::from_secs(30))?;
                join_set.abort_all();
                return Err(KafkaError::Canceled.into());
            }
            Err(e) => {
                error!(error = ?e, "任务执行失败，中止事务");
                ap.abort_transaction(Duration::from_secs(30))?;
                join_set.abort_all();
                return Err(e.into());
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

fn process_message(
    msg: &BorrowedMessage<'_>,
) -> Result<(Uuid, [u8; 16], [u8; 8], Vec<u8>, UniResponse), UniError> {
    let agg_id = crate::get_agg_key(msg)?;
    let headers = msg.headers().ok_or("消息头不存在")?;
    let com_id = crate::get_com_id(headers)?;
    let span_id = crate::get_span_id(headers)?;
    let res = crate::get_response(headers)?;
    let payload = msg.payload().ok_or("消息体不存在")?.to_vec();
    Ok((agg_id, com_id, span_id, payload, res))
}
