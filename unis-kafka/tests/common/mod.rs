use rdkafka::{
    ClientConfig,
    admin::{AdminClient, AdminOptions},
    client::DefaultClientContext,
};
use rstest::fixture;
use std::{
    path::PathBuf,
    sync::{Arc, LazyLock},
};
use tokio::{sync::OnceCell, time::Duration};
use tracing::{Level, error};
use tracing_appender::non_blocking;
use tracing_subscriber::fmt;
use unis::{
    config::build_config,
    test_utils::kube::{HelmRelease, KubeCluster},
};
use unis_kafka::subscriber::{App, app::test_context};

pub(crate) static ADMIN: LazyLock<AdminClient<DefaultClientContext>> = LazyLock::new(|| {
    let config = build_config(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
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

static TEST_CONTEXT: OnceCell<()> = OnceCell::const_new();
#[fixture]
async fn external_setup() {
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
async fn app() -> Arc<App> {
    test_context().await
}
