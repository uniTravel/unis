//! # Kafka 投影者

use crate::config::ProjectorConfig;
use ahash::{AHashMap, RandomState};
use dashmap::DashMap;
use rdkafka::{
    ClientConfig, Message, TopicPartitionList,
    consumer::{Consumer, StreamConsumer},
    error::KafkaError,
    message::{BorrowedMessage, Header, OwnedHeaders},
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
use tracing::{Instrument, error, info, info_span, instrument, warn};
use unis::{UniResponse, domain::Config, errors::UniError, link_context};
use uuid::Uuid;

pub use unis::app::context;
pub use unis::domain::Aggregate;

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
            let topic = format!("{}.{}", ProjectorConfig::get().name, agg_type);
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
}

fn create_producer() -> Result<FutureProducer, KafkaError> {
    let cfg_projector = ProjectorConfig::get();
    let transaction_id = format!("{}-{}", cfg_projector.name, cfg_projector.hostname);
    let mut config = ClientConfig::new();
    for (key, value) in cfg_projector.pp.clone() {
        config.set(key, value);
    }

    let ap: FutureProducer = config
        .set("bootstrap.servers", &cfg_projector.bootstrap)
        .set("transactional.id", transaction_id)
        .create()?;
    ap.init_transactions(std::time::Duration::from_secs(30))?;
    Ok(ap)
}

fn create_consumer() -> Result<StreamConsumer, KafkaError> {
    let cfg_projector = ProjectorConfig::get();
    let mut config = ClientConfig::new();
    for (key, value) in cfg_projector.pc.clone() {
        config.set(key, value);
    }
    config
        .set("bootstrap.servers", &cfg_projector.bootstrap)
        .set("group.id", &cfg_projector.name)
        .create()
}

static INITIATED: AtomicBool = AtomicBool::new(false);

/// 启动投影
pub async fn launch(ctx: &'static unis::app::Context, topics: Vec<&'static str>) {
    let cfg_projector = ProjectorConfig::get();
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
                        error!(error = ?e, "处理消息数据出错，即将退出应用");
                        break;
                    }
                    Err(ProjectError::MetadataError) => error!("获取消费组元数据失败"),
                    Err(ProjectError::KafkaError(e)) => {
                        error!(error = ?e, "投影处理错误");
                        ap = create_producer().expect("重建投影生产者失败");
                    }
                }
                count += 1;
                if count == cfg_projector.tries {
                    error!("尝试 {count} 次仍然失败，退出应用！");
                    break;
                }
                sleep(Duration::from_secs(cfg_projector.secs)).await;
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
    let cfg_projector = ProjectorConfig::get();
    tc.subscribe(topics)?;
    let mut agg_msgs = AHashMap::with_capacity(cfg_projector.partitions);
    let mut offsets = AHashMap::with_capacity(cfg_projector.capacity);
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
                if !agg_msgs.is_empty() && last_flush.elapsed() > Duration::from_millis(cfg_projector.interval) {
                    process_batch(ap, tc, &mut agg_msgs, &mut offsets, "触及提交间隔阈值").await?;
                    last_flush = Instant::now();
                    count = 0;
                }
            }
            data = tc.recv() => match data {
                Ok(msg) => match process_message(&msg) {
                    Ok((agg_id, com_id, span_id, revision, payload, res)) if res == UniResponse::Success => {
                        let agg_type = msg.topic().to_owned();
                        let mut topic = String::with_capacity(agg_type.len() + 37);
                        topic.push_str(&agg_type);
                        topic.push_str("-");
                        topic.push_str(&agg_id.to_string());
                        let partition = msg.partition();
                        let offset = msg.offset();

                        match agg_msgs.get_mut(&topic) {
                            Some(msgs) => msgs.push((com_id, span_id, revision, payload)),
                            None => {
                                if agg_msgs.len() == cfg_projector.partitions {
                                    process_batch(ap, tc, &mut agg_msgs, &mut offsets, "触及分区数阈值").await?;
                                    last_flush = Instant::now();
                                    count = 0;
                                }
                                agg_msgs.insert(topic, vec![(com_id, span_id, revision, payload)]);
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
                        if count == cfg_projector.capacity {
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

#[instrument(name = "batch_project", err(Debug), skip_all)]
async fn process_batch(
    ap: &FutureProducer,
    tc: &StreamConsumer,
    agg_msgs: &mut AHashMap<String, Vec<([u8; 16], [u8; 8], [u8; 8], Vec<u8>)>>,
    offsets: &mut AHashMap<(String, i32), i64>,
    reason: &str,
) -> Result<(), ProjectError> {
    info!("提交批量投影：{reason}");
    let cgm = tc.group_metadata().ok_or(ProjectError::MetadataError)?;
    let msg_vec: Vec<(String, Vec<([u8; 16], [u8; 8], [u8; 8], Vec<u8>)>)> =
        agg_msgs.drain().collect();
    let offset_vec: Vec<((String, i32), i64)> = offsets.drain().collect();
    let mut join_set: JoinSet<Result<(), KafkaError>> = JoinSet::new();

    ap.begin_transaction()?;

    for (topic, msgs) in msg_vec {
        for (com_id, span_id, revision, payload) in msgs {
            let sp = link_context(info_span!("project"), com_id, span_id);
            let record = FutureRecord::to(&topic)
                .payload(&payload)
                .key(&com_id)
                .headers(OwnedHeaders::new_with_capacity(1).insert(Header {
                    key: "revision",
                    value: Some(&revision),
                }));
            match ap.send_result(record) {
                Ok(future) => {
                    join_set.spawn(
                        async {
                            match future.await {
                                Ok(Ok(Delivery {
                                    partition,
                                    offset,
                                    timestamp: _,
                                })) => {
                                    info!("转存的事件写到分区 {partition} 偏移 {offset}");
                                    Ok(())
                                }
                                Ok(Err((e, _))) => {
                                    error!(error = ?e, "消息投递失败");
                                    Err(e)
                                }
                                Err(e) => {
                                    error!(error = ?e, "任务取消");
                                    Ok(())
                                }
                            }
                        }
                        .instrument(sp),
                    );
                }
                Err((e, _)) => {
                    warn!("生产消息有误，开始中止事务");
                    ap.abort_transaction(Duration::from_secs(30))?;
                    return Err(e.into());
                }
            }
        }
    }

    let mut err: Option<KafkaError> = None;
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Err(e)) => {
                join_set.abort_all();
                if err.is_none() {
                    err = Some(e);
                }
            }
            _ => {}
        }
    }

    if let Some(e) = err {
        warn!("投递消息有误，开始中止事务");
        ap.abort_transaction(Duration::from_secs(30))?;
        return Err(e.into());
    }

    let mut offsets = TopicPartitionList::new();
    for ((topic, partition), offset) in offset_vec {
        offsets.add_partition_offset(&topic, partition, rdkafka::Offset::Offset(offset + 1))?;
    }
    if let Err(e) = ap.send_offsets_to_transaction(&offsets, &cgm, Duration::from_secs(30)) {
        warn!("提交偏移失败，开始中止事务");
        ap.abort_transaction(Duration::from_secs(30))?;
        return Err(e.into());
    }

    info!("开始提交事务");
    if let Err(e) = ap.commit_transaction(Duration::from_secs(30)) {
        warn!("提交事务失败，开始中止事务");
        ap.abort_transaction(Duration::from_secs(30))?;
        return Err(e.into());
    }

    info!("完成批量投影事务");
    Ok(())
}

fn process_message(
    msg: &BorrowedMessage<'_>,
) -> Result<(Uuid, [u8; 16], [u8; 8], [u8; 8], Vec<u8>, UniResponse), UniError> {
    let agg_id = crate::get_agg_key(msg)?;
    let headers = msg.headers().ok_or("消息头不存在")?;
    let com_id = crate::get_com_id(headers)?;
    let span_id = crate::get_span_id(headers)?;
    let revison = crate::get_revision(headers)?;
    let res = crate::get_response(headers)?;
    let payload = msg.payload().ok_or("消息体不存在")?.to_vec();
    Ok((agg_id, com_id, span_id, revison, payload, res))
}
