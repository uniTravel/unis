mod restore_test;
mod stream_test;

use crate::{
    config::load_bootstrap,
    subscriber::{App, SUBSCRIBER_CONFIG, app::test_context, reader::restore, stream::Writer},
};
use domain::note;
use rdkafka::{
    ClientConfig,
    admin::{AdminClient, AdminOptions, NewTopic, TopicReplication},
    client::DefaultClientContext,
};
use rstest::*;
use std::{
    path::PathBuf,
    sync::{Arc, LazyLock},
};
use tokio::{
    sync::OnceCell,
    time::{Duration, sleep},
};
use tracing::{Level, info, warn};
use tracing_appender::non_blocking;
use tracing_subscriber::fmt;
use unis::{
    Response,
    config::build_config,
    domain::{Aggregate, Stream},
    test_utils::kube::{HelmRelease, KubeCluster},
};
use uuid::Uuid;

static ADMIN: LazyLock<AdminClient<DefaultClientContext>> = LazyLock::new(|| {
    let config = build_config(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    let bootstrap = load_bootstrap(&config);
    ClientConfig::new()
        .set("bootstrap.servers", bootstrap)
        .create()
        .expect("管理客户端创建失败")
});

static OPTS: LazyLock<AdminOptions> = LazyLock::new(|| {
    AdminOptions::new()
        .operation_timeout(Some(Duration::from_secs(3)))
        .request_timeout(Some(Duration::from_secs(5)))
});

static TEST_CONTEXT: OnceCell<()> = OnceCell::const_new();
#[fixture]
async fn internal_setup() {
    TEST_CONTEXT
        .get_or_init(|| async {
            LazyLock::force(&ADMIN);
            LazyLock::force(&OPTS);
            let (non_blocking, _guard) = non_blocking(std::io::stdout());
            fmt()
                .with_max_level(Level::DEBUG)
                .with_writer(non_blocking)
                .with_target(false)
                .pretty()
                .with_test_writer()
                .init();
            let namespace = "internal";
            let cluster = KubeCluster::new(namespace);
            let kafka = HelmRelease::new(
                "kafka",
                std::env::home_dir()
                    .unwrap()
                    .join(".cache/helm/repository/kafka-0.1.0.tgz"),
                namespace,
            );
            cluster.create_namespace().unwrap();
            kafka.install(None).await.unwrap();
        })
        .await;
}

#[fixture]
async fn context() -> (Arc<App>, Writer) {
    let app = test_context().await;
    let agg_type = note::Note::topic();
    let cfg_name = agg_type.rsplit(".").next().expect("获取聚合名称失败");
    let cfg = SUBSCRIBER_CONFIG.subscriber.get(cfg_name);
    let stream = Writer::new(&cfg, app.topic_tx());
    (app, stream)
}

async fn is_topic_exist(name: &str) -> bool {
    let max_retries = 10;
    let mut retry = 0;
    sleep(Duration::from_millis(50)).await;
    loop {
        match ADMIN
            .inner()
            .fetch_metadata(Some(name), Duration::from_millis(100))
        {
            Ok(metadata) => {
                for t in metadata.topics().iter() {
                    if let Some(err) = t.error() {
                        warn!("元数据存在错误：{:?}", err);
                    }
                    if t.name() == name && t.error().is_none() && !t.partitions().is_empty() {
                        return true;
                    }
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
        info!("第 {retry} 次验证主题 {name} 是否存在");
    }
}

async fn is_agg_topic_exist(agg_type: &str, agg_id: Uuid) -> bool {
    let mut topic = String::with_capacity(agg_type.len() + 37);
    topic.push_str(agg_type);
    topic.push_str("-");
    topic.push_str(&agg_id.to_string());
    is_topic_exist(&topic).await
}
