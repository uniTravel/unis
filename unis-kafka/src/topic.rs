use crate::subscriber::SUBSCRIBER_CONFIG;
use rdkafka::{
    ClientConfig,
    admin::{AdminClient, AdminOptions, NewTopic, TopicReplication},
    client::DefaultClientContext,
};
use std::sync::LazyLock;
use tokio::{
    sync::mpsc,
    time::{Duration, Instant},
};
use tracing::{debug, info, warn};
use uuid::Uuid;

static ADMIN: LazyLock<AdminClient<DefaultClientContext>> = LazyLock::new(|| {
    ClientConfig::new()
        .set("bootstrap.servers", &SUBSCRIBER_CONFIG.bootstrap)
        .create()
        .expect("恢复命令操作记录消费者创建失败")
});

pub(crate) static TOPIC_TX: LazyLock<mpsc::UnboundedSender<TopicTask>> = LazyLock::new(|| {
    let (topic_tx, topic_rx) = mpsc::unbounded_channel::<TopicTask>();
    LazyLock::force(&ADMIN);
    tokio::spawn(topic_creator(topic_rx));
    topic_tx
});

pub(crate) struct TopicTask {
    pub agg_type: &'static str,
    pub agg_id: Uuid,
}

async fn topic_creator(mut topic_rx: mpsc::UnboundedReceiver<TopicTask>) {
    let mut batch = Vec::with_capacity(1000);
    let mut last_flush = Instant::now();
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    let mut count: usize = 0;
    let threshold = 1000 - 1;
    let opts = AdminOptions::new()
        .operation_timeout(Some(Duration::from_secs(30)))
        .request_timeout(Some(Duration::from_secs(45)));

    loop {
        tokio::select! {
            biased;
            _ = interval.tick() => {
                if !batch.is_empty() && last_flush.elapsed() > Duration::from_secs(5) {
                    debug!("触及提交间隔阈值，提交主题创建");
                    topic_batch(&mut batch, &opts).await;
                    last_flush = Instant::now();
                    count = 0;
                }
            }
            data = topic_rx.recv() => {
                match data {
                    Some(TopicTask{agg_type, agg_id}) => {
                        let mut topic = String::with_capacity(agg_type.len() + 37);
                        topic.push_str(agg_type);
                        topic.push_str("-");
                        topic.push_str(&agg_id.to_string());
                        batch.push(topic);

                        if count == threshold {
                            debug!("触及提交计数阈值，提交主题创建");
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
                            info!("优雅关闭，提交主题创建");
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
        let t = NewTopic::new(&topic, 1, TopicReplication::Fixed(2)).set("retention.ms", "-1");
        topics.push(t);
    }

    if let Err(e) = ADMIN.create_topics(&topics, opts).await {
        warn!("批量创建主题失败: {e}");
    }
}
