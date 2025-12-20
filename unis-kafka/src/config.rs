use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{OnceLock, RwLock},
};
use tokio::time::Duration;
use tracing::error;
use unis::{
    config::{NamedConfig, SendConfig, SubscribeConfig, build_config, load_named_config},
    domain,
};

static SUBSCRIBER: OnceLock<RwLock<SubscriberConfig>> = OnceLock::new();
static SENDER: OnceLock<RwLock<SenderConfig>> = OnceLock::new();

fn load_named_setting(
    config: &config::Config,
    section: &str,
) -> HashMap<String, HashMap<String, String>> {
    let mut result = HashMap::new();
    let input = config
        .get::<HashMap<String, config::Value>>(section)
        .unwrap_or(HashMap::new());

    for (section, value) in input {
        let map = match value.try_deserialize::<HashMap<String, String>>() {
            Ok(c) => c,
            Err(e) => {
                error!("加载命名配置'{section}'失败：{e}");
                panic!("加载命名配置失败");
            }
        };
        result.insert(section, map);
    }

    result
}

#[inline]
pub(crate) fn load_bootstrap(config: &config::Config) -> String {
    match config.get("bootstrap") {
        Ok(c) => c,
        Err(e) => {
            error!("加载'bootstrap'配置失败：{e}");
            panic!("加载'bootstrap'配置失败");
        }
    }
}

#[inline]
pub(crate) fn load_hostname(config: &config::Config) -> String {
    match config.get("hostname") {
        Ok(c) => c,
        Err(e) => {
            error!("加载'hostname'配置失败：{e}");
            panic!("加载'hostname'配置失败");
        }
    }
}

#[inline]
fn load_timeout(config: &config::Config) -> Duration {
    match config.get("timeout") {
        Ok(t) => Duration::from_secs(t),
        Err(_) => Duration::from_secs(45),
    }
}

fn load_subscriber() -> SubscriberConfig {
    let config = build_config(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    let bootstrap = load_bootstrap(&config);
    let replicas = config.get("replicas").unwrap_or(3);
    let aggs = config.get("aggs").unwrap_or(16);
    let timeout = load_timeout(&config);
    let subscriber = load_named_config(&config, "subscriber");
    let cc = load_named_setting(&config, "cc");
    let tp = match config.get::<HashMap<String, String>>("tp") {
        Ok(c) => c,
        Err(e) => {
            error!("加载聚合类型生产者配置失败：{e}");
            panic!("加载聚合类型生产者配置失败");
        }
    };
    SubscriberConfig {
        bootstrap,
        replicas,
        aggs,
        timeout,
        subscriber,
        cc,
        tp,
    }
}

fn load_sender() -> SenderConfig {
    let config = build_config(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    let bootstrap = load_bootstrap(&config);
    let hostname = load_hostname(&config);
    let timeout = load_timeout(&config);
    let sender = load_named_config(&config, "sender");
    let tc = load_named_setting(&config, "tc");
    let cp = match config.get::<HashMap<String, String>>("cp") {
        Ok(c) => c,
        Err(e) => {
            error!("加载聚合命令生产者配置失败：{e}");
            panic!("加载聚合命令生产者配置失败");
        }
    };
    SenderConfig {
        bootstrap,
        hostname,
        timeout,
        sender,
        tc,
        cp,
    }
}

#[derive(Debug, Clone)]
pub struct SubscriberConfig {
    pub bootstrap: String,
    pub replicas: i32,
    pub aggs: usize,
    pub timeout: Duration,
    pub subscriber: NamedConfig<SubscribeConfig>,
    pub cc: HashMap<String, HashMap<String, String>>,
    pub tp: HashMap<String, String>,
}

impl domain::Config for SubscriberConfig {
    fn get() -> Self {
        match SUBSCRIBER
            .get_or_init(|| RwLock::new(load_subscriber()))
            .read()
        {
            Ok(cfg) => cfg.clone(),
            Err(e) => {
                error!("获取订阅者配置失败：{e}");
                panic!("获取订阅者配置失败");
            }
        }
    }

    fn reload() {
        let cfg = match SUBSCRIBER.get() {
            Some(c) => c,
            None => {
                error!("订阅者配置未初始化");
                panic!("订阅者配置未初始化");
            }
        };
        let mut cell = match cfg.write() {
            Ok(c) => c,
            Err(e) => {
                error!("订阅者配置重新加载失败：{e}");
                panic!("订阅者配置重新加载失败");
            }
        };
        *cell = load_subscriber();
    }
}

#[derive(Debug, Clone)]
pub struct SenderConfig {
    pub bootstrap: String,
    pub hostname: String,
    pub timeout: Duration,
    pub sender: NamedConfig<SendConfig>,
    pub tc: HashMap<String, HashMap<String, String>>,
    pub cp: HashMap<String, String>,
}

impl domain::Config for SenderConfig {
    fn get() -> Self {
        match SENDER.get_or_init(|| RwLock::new(load_sender())).read() {
            Ok(cfg) => cfg.clone(),
            Err(e) => {
                error!("获取发送者配置失败：{e}");
                panic!("获取发送者配置失败");
            }
        }
    }

    fn reload() {
        let cfg = match SENDER.get() {
            Some(c) => c,
            None => {
                error!("发送者配置未初始化");
                panic!("发送者配置未初始化");
            }
        };
        let mut cell = match cfg.write() {
            Ok(c) => c,
            Err(e) => {
                error!("发送者配置重新加载失败：{e}");
                panic!("发送者配置重新加载失败");
            }
        };
        *cell = load_sender();
    }
}
