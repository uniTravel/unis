#![allow(dead_code)]

use std::{
    collections::HashMap,
    sync::{OnceLock, RwLock},
};
use tokio::time::Duration;
use tracing::error;
use unis::{
    config::{self, NamedConfig, SendConfig, SubscribeConfig},
    domain,
};

static SUBSCRIBER: OnceLock<RwLock<SubscriberConfig>> = OnceLock::new();
static SENDER: OnceLock<RwLock<SenderConfig>> = OnceLock::new();

#[inline]
pub(crate) fn load_name(cfg: &::config::Config) -> String {
    match cfg.get("name") {
        Ok(c) => c,
        Err(e) => {
            error!("加载'name'配置失败：{e}");
            panic!("加载'name'配置失败");
        }
    }
}

#[inline]
pub(crate) fn load_hostname(cfg: &::config::Config) -> String {
    match cfg.get("hostname") {
        Ok(c) => c,
        Err(e) => {
            error!("加载'hostname'配置失败：{e}");
            panic!("加载'hostname'配置失败");
        }
    }
}

#[inline]
pub(crate) fn load_bootstrap(cfg: &::config::Config) -> String {
    match cfg.get("bootstrap") {
        Ok(c) => c,
        Err(e) => {
            error!("加载'bootstrap'配置失败：{e}");
            panic!("加载'bootstrap'配置失败");
        }
    }
}

#[inline]
fn load_timeout(cfg: &::config::Config) -> Duration {
    match cfg.get("timeout") {
        Ok(t) => Duration::from_secs(t),
        Err(_) => Duration::from_secs(45),
    }
}

fn load_subscriber() -> SubscriberConfig {
    let cfg = config::build_config();
    let bootstrap = load_bootstrap(&cfg);
    let replicas = cfg.get("replicas").unwrap_or(3);
    let aggs = cfg.get("aggs").unwrap_or(16);
    let timeout = load_timeout(&cfg);
    let subscriber = config::load_named_config(&cfg, "subscriber");
    let cc = config::load_named_setting(&cfg, "cc");
    let tp = match cfg.get::<HashMap<String, String>>("tp") {
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
    let cfg = config::build_config();
    let bootstrap = load_bootstrap(&cfg);
    let hostname = load_hostname(&cfg);
    let timeout = load_timeout(&cfg);
    let sender = config::load_named_config(&cfg, "sender");
    let tc = config::load_named_setting(&cfg, "tc");
    let cp = match cfg.get::<HashMap<String, String>>("cp") {
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
