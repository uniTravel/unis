pub(crate) use domain::note::{self, CreateNote, NoteCommand};
pub(crate) use rdkafka::{
    ClientConfig,
    admin::{AdminClient, AdminOptions, NewTopic, TopicReplication},
    client::DefaultClientContext,
};
pub(crate) use rstest::{fixture, rstest};
pub(crate) use std::{
    path::PathBuf,
    sync::{Arc, LazyLock},
};
pub(crate) use tokio::{sync::OnceCell, time::Duration};
use tracing::{Level, error};
use tracing_appender::non_blocking;
use tracing_subscriber::fmt;
pub(crate) use unis::{
    Response, config,
    domain::{Aggregate, Request},
    test_utils::kube::{HelmRelease, KubeCluster},
};
pub(crate) use unis_kafka::{sender, subscriber};
pub(crate) use uuid::Uuid;

pub(crate) static ADMIN: LazyLock<AdminClient<DefaultClientContext>> = LazyLock::new(|| {
    let config = config::build_config(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    let bootstrap = match config.get::<String>("bootstrap") {
        Ok(c) => c,
        Err(e) => {
            error!("加载'bootstrap'配置失败：{e}");
            panic!("加载'bootstrap'配置失败");
        }
    };
    ClientConfig::new()
        .set("bootstrap.servers", bootstrap)
        .create()
        .expect("管理客户端创建失败")
});

pub(crate) static OPTS: LazyLock<AdminOptions> = LazyLock::new(|| {
    AdminOptions::new()
        .operation_timeout(Some(Duration::from_secs(3)))
        .request_timeout(Some(Duration::from_secs(5)))
});

static EXTERNAL_SETUP: OnceCell<()> = OnceCell::const_new();
#[fixture]
pub(crate) async fn external_setup() {
    EXTERNAL_SETUP
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
            let namespace = "external";
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
pub(crate) async fn ctx_subscriber() -> Arc<subscriber::app::App> {
    subscriber::app::test_context().await
}

#[fixture]
pub(crate) async fn ctx_sender() -> Arc<sender::app::App> {
    sender::app::test_context().await
}
