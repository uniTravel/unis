#![allow(dead_code)]

use rdkafka::{ClientConfig, admin::AdminClient, client::DefaultClientContext};
use std::{collections::HashMap, path::PathBuf, sync::LazyLock};
use tracing::Level;
use tracing_subscriber::fmt;
use unis::config::build_config;

pub(crate) static CFG: LazyLock<config::Config> = LazyLock::new(|| {
    let config = build_config(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    fmt().with_test_writer().with_max_level(Level::DEBUG).init();
    config
});

pub(crate) static ADMIN: LazyLock<AdminClient<DefaultClientContext>> =
    LazyLock::new(|| config(HashMap::new()).create().unwrap());

pub(crate) fn config(settings: HashMap<&str, &str>) -> ClientConfig {
    let bootstrap = CFG.get::<String>("bootstrap").unwrap();
    let mut config = ClientConfig::new();
    for (key, value) in settings {
        config.set(key, value);
    }
    config.set("bootstrap.servers", bootstrap);
    config
}
