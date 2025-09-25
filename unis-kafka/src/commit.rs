use crate::subscriber::SHUTDOWN_RX;
use ahash::AHashMap;
use rdkafka::{
    Message, Offset, TopicPartitionList,
    consumer::{CommitMode, Consumer, StreamConsumer},
    message::BorrowedMessage,
};
use std::sync::Arc;
use tokio::{
    sync::mpsc,
    time::{Duration, Instant},
};
use tracing::{debug, info, warn};

pub(crate) struct CommitTask {
    topic: String,
    partition: i32,
    offset: i64,
}

impl From<&BorrowedMessage<'_>> for CommitTask {
    fn from(msg: &BorrowedMessage) -> Self {
        Self {
            topic: msg.topic().to_string(),
            partition: msg.partition(),
            offset: msg.offset(),
        }
    }
}

pub(crate) async fn commit_coordinator(
    consumer: Arc<StreamConsumer>,
    mut commit_rx: mpsc::UnboundedReceiver<CommitTask>,
) {
    let mut shutdown_rx = SHUTDOWN_RX.clone();
    let mut tpl = TopicPartitionList::new();
    let mut batch = AHashMap::new();
    let mut last_flush = Instant::now();
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    let mut count: usize = 0;
    let threshold = 1000 - 1;

    loop {
        tokio::select! {
            biased;
            _ = shutdown_rx.changed(), if *shutdown_rx.borrow() => {
                info!("收到关闭信号，开始优雅退出");
                if !batch.is_empty() {
                    info!("优雅关闭，提交偏移量");
                    commit_batch(&consumer, &mut tpl, &mut batch).await;
                }
                break;
            }
            Some(task) = commit_rx.recv() => {
                batch.entry((task.topic, task.partition))
                    .and_modify(|e| *e = task.offset.max(*e))
                    .or_insert(task.offset);

                if count == threshold {
                    debug!("触及提交计数阈值，提交偏移量");
                    commit_batch(&consumer, &mut tpl, &mut batch).await;
                    last_flush = Instant::now();
                    count = 0;
                } else {
                    count += 1;
                }
            }
            _ = interval.tick() => {
                if !batch.is_empty() && last_flush.elapsed() > Duration::from_secs(5) {
                    debug!("触及提交间隔阈值，提交偏移量");
                    commit_batch(&consumer,&mut tpl, &mut batch).await;
                    last_flush = Instant::now();
                    count = 0;
                }
            }
        }
    }
}

async fn commit_batch(
    consumer: &StreamConsumer,
    tpl: &mut TopicPartitionList,
    batch: &mut AHashMap<(String, i32), i64>,
) {
    for ((topic, partition), offset) in batch.drain() {
        if let Err(e) = tpl.add_partition_offset(&topic, partition, Offset::Offset(offset)) {
            warn!("添加偏移量{offset}到主题分区{topic}|{partition}失败：{e}");
        }
    }

    if let Err(e) = consumer.commit(&tpl, CommitMode::Async) {
        warn!("提交失败: {e}");
    }
}
