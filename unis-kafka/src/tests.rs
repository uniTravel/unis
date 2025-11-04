//! 内部集成测试

mod reader_test;
mod stream_test;
mod topic_test;

use super::*;
use domain::note;
use home::home_dir;
use rdkafka::admin::{AdminOptions, NewTopic, TopicReplication};
use rstest::*;
use tokio::{
    sync::OnceCell,
    time::{Duration, sleep},
};
use tracing::{Level, info};
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
        home_dir()
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

static INTERNAL_SETUP: LazyLock<()> = LazyLock::new(|| {
    fmt().with_test_writer().with_max_level(Level::DEBUG).init();
    info!("启用 {} 测试日志输出", Level::DEBUG);
    CLUSTER.create_namespace().unwrap();
    KAFKA.install(None).unwrap();
});

async fn create_topic(name: &str) {
    let topic = NewTopic::new(name, 3, TopicReplication::Fixed(3));
    let _ = ADMIN.create_topics(&[topic], &OPTS).await;
}

async fn delete_topic(name: &str) {
    let _ = ADMIN.delete_topics(&[name], &OPTS).await;
}

fn agg_topic(agg_type: &str, agg_id: Uuid) -> String {
    let mut topic = String::with_capacity(agg_type.len() + 37);
    topic.push_str(agg_type);
    topic.push_str("-");
    topic.push_str(&agg_id.to_string());
    topic
}

fn is_topic_exist(name: &str) -> bool {
    ADMIN
        .inner()
        .fetch_metadata(Some(name), Duration::from_millis(100))
        .unwrap()
        .topics()
        .iter()
        .find(|t| t.name() == name)
        .map_or(false, |topic| {
            topic.error().is_none() && !topic.partitions().is_empty()
        })
}
