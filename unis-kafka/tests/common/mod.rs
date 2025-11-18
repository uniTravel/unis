#![allow(dead_code)]

use rdkafka::{
    ClientConfig,
    admin::{AdminClient, AdminOptions},
    client::DefaultClientContext,
};
use std::{collections::HashMap, path::PathBuf, sync::LazyLock};
use tokio::time::Duration;
use tracing::{Level, info};
use tracing_subscriber::fmt;
use unis::config::build_config;
use unis_test::kube::{HelmRelease, KubeCluster};

const NAMESPACE: &str = "external";
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

pub(crate) static OPTS: LazyLock<AdminOptions> = LazyLock::new(|| {
    AdminOptions::new()
        .operation_timeout(Some(Duration::from_secs(3)))
        .request_timeout(Some(Duration::from_secs(5)))
});

pub(crate) static EXTERNAL_SETUP: LazyLock<()> = LazyLock::new(|| {
    LazyLock::force(&ADMIN);
    LazyLock::force(&OPTS);
    CLUSTER.create_namespace().unwrap();
    KAFKA.install(None).unwrap();
});

pub(crate) static CFG: LazyLock<config::Config> = LazyLock::new(|| {
    let config = build_config(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    fmt().with_test_writer().with_max_level(Level::DEBUG).init();
    info!("启用 {} 测试日志输出", Level::DEBUG);
    config
});

pub(crate) static ADMIN: LazyLock<AdminClient<DefaultClientContext>> =
    LazyLock::new(|| config(HashMap::new()).create().expect("管理客户端创建失败"));

pub(crate) fn config(settings: HashMap<&str, &str>) -> ClientConfig {
    let bootstrap = CFG.get::<String>("bootstrap").unwrap();
    let mut config = ClientConfig::new();
    for (key, value) in settings {
        config.set(key, value);
    }
    config.set("bootstrap.servers", bootstrap);
    config
}
