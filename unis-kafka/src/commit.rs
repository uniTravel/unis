use ahash::AHashMap;
use rdkafka::{
    Message, Offset, TopicPartitionList,
    consumer::{CommitMode, Consumer, StreamConsumer},
    message::BorrowedMessage,
};
use std::sync::Arc;
use tokio::{
    sync::{Notify, mpsc},
    time::{Duration, Instant},
};
use tracing::{debug, error, info, instrument};

pub(crate) struct Commit {
    topic: String,
    partition: i32,
    offset: i64,
}

impl From<&BorrowedMessage<'_>> for Commit {
    fn from(msg: &BorrowedMessage) -> Self {
        Self {
            topic: msg.topic().to_owned(),
            partition: msg.partition(),
            offset: msg.offset(),
        }
    }
}

#[instrument(skip(consumer, commit_rx))]
pub(crate) async fn commit_coordinator(
    topic: &'static str,
    consumer: Arc<StreamConsumer>,
    mut commit_rx: mpsc::UnboundedReceiver<Commit>,
    ready: Arc<Notify>,
) {
    let mut tpl = TopicPartitionList::new();
    let mut batch = AHashMap::new();
    let mut last_flush = Instant::now();
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    let mut count: usize = 0;
    let threshold = 1000 - 1;

    ready.notify_one();
    loop {
        tokio::select! {
            biased;
            _ = interval.tick() => {
                if !batch.is_empty() && last_flush.elapsed() > Duration::from_secs(5) {
                    debug!("触及提交间隔阈值，提交偏移量");
                    commit_batch(&consumer,&mut tpl, &mut batch).await;
                    last_flush = Instant::now();
                    count = 0;
                }
            }
            data = commit_rx.recv() => {
                match data {
                    Some(task) => {
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
                    None => {
                        info!("发送端均已关闭，开始优雅退出");
                        if !batch.is_empty() {
                            info!("优雅关闭，提交偏移量");
                            commit_batch(&consumer, &mut tpl, &mut batch).await;
                        }
                        break;
                    }
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
            error!("添加偏移量 {offset} 到主题分区 {topic}|{partition} 失败：{e}");
        }
    }

    if let Err(e) = consumer.commit(&tpl, CommitMode::Async) {
        error!("提交失败: {e}");
    }
}
