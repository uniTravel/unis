use crate::subscriber::SUBSCRIBER_CONFIG;
use rdkafka::admin::{AdminOptions, NewTopic, TopicReplication};
use std::sync::LazyLock;
use tokio::{
    sync::mpsc,
    time::{Duration, Instant},
};
use tracing::{debug, debug_span, error, info};
use uuid::Uuid;

pub(crate) static TOPIC_TX: LazyLock<mpsc::UnboundedSender<TopicTask>> = LazyLock::new(|| {
    let (topic_tx, topic_rx) = mpsc::unbounded_channel::<TopicTask>();
    LazyLock::force(&crate::ADMIN);
    tokio::spawn(topic_creator(topic_rx));
    topic_tx
});

pub(crate) struct TopicTask {
    pub agg_type: &'static str,
    pub agg_id: Uuid,
}

async fn topic_creator(mut topic_rx: mpsc::UnboundedReceiver<TopicTask>) {
    info!("启动聚合主题创建者");
    let capacity = 20;
    let mut batch = Vec::with_capacity(capacity);
    let mut last_flush = Instant::now();
    let mut interval = tokio::time::interval(Duration::from_millis(1));
    let mut count: usize = 0;
    let threshold = capacity - 1;
    let opts = AdminOptions::new()
        .operation_timeout(Some(Duration::from_secs(3)))
        .request_timeout(Some(Duration::from_secs(5)));

    loop {
        tokio::select! {
            biased;
            _ = interval.tick() => {
                if !batch.is_empty() && last_flush.elapsed() > Duration::from_millis(5) {
                    debug!("触及提交间隔阈值，提交聚合主题创建");
                    topic_batch(&mut batch, &opts).await;
                    debug!("结束聚合主题批量创建");
                    last_flush = Instant::now();
                    count = 0;
                }
            }
            data = topic_rx.recv() => {
                match data {
                    Some(TopicTask{agg_type, agg_id}) => {
                        let mut topic = String::with_capacity(agg_type.len() + 37);
                        let span = debug_span!("create_topic", agg_type, %agg_id);
                        span.in_scope(|| {
                            topic.push_str(agg_type);
                            topic.push_str("-");
                            topic.push_str(&agg_id.to_string());
                            debug!("收到聚合主题创建任务");
                            batch.push(topic);
                        });

                        if count == threshold {
                            debug!("触及提交计数阈值，提交聚合主题创建");
                            topic_batch(&mut batch, &opts).await;
                            last_flush = Instant::now();
                            count = 0;
                        } else {
                            count += 1;
                        }
                    }
                    None => {
                        info!("发送端均已关闭，开始优雅退出");
                        if !batch.is_empty() {
                            info!("优雅关闭，提交聚合主题创建");
                            topic_batch(&mut batch, &opts).await;
                        }
                        break;
                    }
                }
            }
        }
    }
}

async fn topic_batch(batch: &mut Vec<String>, opts: &AdminOptions) {
    let mut topics = Vec::new();
    let tns: Vec<String> = batch.drain(..).collect();

    for topic in &tns {
        let topic = NewTopic::new(
            &topic,
            1,
            TopicReplication::Fixed(SUBSCRIBER_CONFIG.replicas),
        )
        .set("retention.ms", "-1");
        topics.push(topic);
    }

    debug!("开始聚合主题批量创建");
    if let Err(e) = crate::ADMIN.create_topics(&topics, opts).await {
        error!("批量创建聚合主题失败: {e}");
    }
}
