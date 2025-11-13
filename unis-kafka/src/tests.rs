//! 内部集成测试

mod reader_test;
mod stream_test;
mod topic_test;

use super::*;
use crate::{stream::Writer, subscriber::SUBSCRIBER_CONFIG};
use domain::note;
use rdkafka::admin::{AdminOptions, NewTopic, TopicReplication};
use rstest::*;
use tokio::{
    sync::OnceCell,
    time::{Duration, sleep},
};
use tracing::{Level, debug, info, warn};
use tracing_subscriber::fmt;
use unis::{
    Response,
    domain::{Aggregate, Stream},
};
use unis_test::kube::{HelmRelease, KubeCluster};
use uuid::Uuid;

const NAMESPACE: &str = "internal";
static CLUSTER: LazyLock<KubeCluster> = LazyLock::new(|| KubeCluster::new(NAMESPACE));
static KAFKA: LazyLock<HelmRelease> = LazyLock::new(|| {
    HelmRelease::new(
        "kafka",
        std::env::home_dir()
            .unwrap()
            .join(".cache/helm/repository/kafka-0.1.0.tgz"),
        NAMESPACE,
    )
});

static OPTS: LazyLock<AdminOptions> = LazyLock::new(|| {
    AdminOptions::new()
        .operation_timeout(Some(Duration::from_secs(3)))
        .request_timeout(Some(Duration::from_secs(5)))
});

static STREAM: LazyLock<Writer> = LazyLock::new(|| {
    let agg_type = note::Note::topic();
    let cfg_name = agg_type.rsplit(".").next().expect("获取聚合名称失败");
    let cfg = SUBSCRIBER_CONFIG.subscriber.get(cfg_name);
    Writer::new(&cfg)
});

static INTERNAL_SETUP: LazyLock<()> = LazyLock::new(|| {
    fmt().with_test_writer().with_max_level(Level::DEBUG).init();
    info!("启用 {} 测试日志输出", Level::DEBUG);
    LazyLock::force(&ADMIN);
    LazyLock::force(&OPTS);
    CLUSTER.create_namespace().unwrap();
    KAFKA.install(None).unwrap();
});

async fn is_topic_exist(name: &str) -> bool {
    let max_retries = 10;
    let mut retry = 0;
    sleep(Duration::from_millis(20)).await;
    loop {
        match ADMIN
            .inner()
            .fetch_metadata(Some(name), Duration::from_millis(100))
        {
            Ok(metadata) => {
                if metadata
                    .topics()
                    .iter()
                    .any(|t| t.name() == name && t.error().is_none() && !t.partitions().is_empty())
                {
                    return true;
                }
            }
            Err(e) => {
                warn!("获取主题 {name} 的元数据失败：{e}");
            }
        }

        retry += 1;
        if retry > max_retries {
            return false;
        }
        sleep(Duration::from_millis(500)).await;
        debug!("第 {retry} 次验证主题 {name} 是否存在");
    }
}

async fn is_agg_topic_exist(agg_type: &str, agg_id: Uuid) -> bool {
    let mut topic = String::with_capacity(agg_type.len() + 37);
    topic.push_str(agg_type);
    topic.push_str("-");
    topic.push_str(&agg_id.to_string());
    is_topic_exist(&topic).await
}
