use std::{collections::HashMap, sync::LazyLock};
use tokio::time::Duration;
use unis::{
    config::{self, NamedConfig, SendConfig, SubscribeConfig, load_hostname, load_name},
    domain,
};

fn load_bootstrap(cfg: &::config::Config) -> String {
    match cfg.get("bootstrap") {
        Ok(c) => c,
        Err(e) => {
            panic!("加载'bootstrap'配置失败：{e}");
        }
    }
}

fn load_timeout(cfg: &::config::Config) -> Duration {
    match cfg.get("timeout") {
        Ok(t) => Duration::from_secs(t),
        Err(_) => Duration::from_secs(45),
    }
}

fn load_subscriber() -> SubscriberConfig {
    let cfg = config::build_config();
    let name = load_name(&cfg);
    let bootstrap = load_bootstrap(&cfg);
    let replicas = cfg.get("replicas").unwrap_or(3);
    let aggs = cfg.get("aggs").unwrap_or(16);
    let timeout = load_timeout(&cfg);
    let subscriber = config::load_named_config(&cfg, "subscriber");
    let cc = config::load_named_setting(&cfg, "cc");
    let tp = match cfg.get::<HashMap<String, String>>("tp") {
        Ok(c) => c,
        Err(e) => {
            panic!("加载聚合类型生产者配置失败：{e}");
        }
    };
    let admin = match cfg.get::<HashMap<String, String>>("admin") {
        Ok(c) => c,
        Err(e) => {
            panic!("加载主题管理客户端配置失败：{e}");
        }
    };
    SubscriberConfig {
        name,
        bootstrap,
        replicas,
        aggs,
        timeout,
        subscriber,
        cc,
        tp,
        admin,
    }
}

fn load_projector() -> ProjectorConfig {
    let cfg = config::build_config();
    let name = load_name(&cfg);
    let hostname = load_hostname(&cfg);
    let bootstrap = load_bootstrap(&cfg);
    let capacity = cfg.get("capacity").unwrap_or(100);
    let partitions = cfg.get("partitions").unwrap_or(10);
    let interval = cfg.get("interval").unwrap_or(50);
    let tries = cfg.get("tries").unwrap_or(15);
    let secs = cfg.get("secs").unwrap_or(45);
    let pc = match cfg.get::<HashMap<String, String>>("pc") {
        Ok(c) => c,
        Err(e) => {
            panic!("加载投影消费者配置失败：{e}");
        }
    };
    let pp = match cfg.get::<HashMap<String, String>>("pp") {
        Ok(c) => c,
        Err(e) => {
            panic!("加载投影生产者配置失败：{e}");
        }
    };
    ProjectorConfig {
        name,
        hostname,
        bootstrap,
        capacity,
        partitions,
        interval,
        tries,
        secs,
        pc,
        pp,
    }
}

fn load_sender() -> SenderConfig {
    let cfg = config::build_config();
    let name = load_name(&cfg);
    let hostname = load_hostname(&cfg);
    let bootstrap = load_bootstrap(&cfg);
    let timeout = load_timeout(&cfg);
    let sender = config::load_named_config(&cfg, "sender");
    let tc = config::load_named_setting(&cfg, "tc");
    let cp = match cfg.get::<HashMap<String, String>>("cp") {
        Ok(c) => c,
        Err(e) => {
            panic!("加载聚合命令生产者配置失败：{e}");
        }
    };
    SenderConfig {
        name,
        hostname,
        bootstrap,
        timeout,
        sender,
        tc,
        cp,
    }
}

#[derive(Debug, Clone)]
pub struct SubscriberConfig {
    pub name: String,
    pub bootstrap: String,
    pub replicas: i32,
    pub aggs: usize,
    pub timeout: Duration,
    pub subscriber: NamedConfig<SubscribeConfig>,
    pub cc: HashMap<String, HashMap<String, String>>,
    pub tp: HashMap<String, String>,
    pub admin: HashMap<String, String>,
}

static SUBSCRIBER: LazyLock<SubscriberConfig> = LazyLock::new(|| load_subscriber());

impl domain::Config for SubscriberConfig {
    #[inline(always)]
    fn get() -> &'static Self {
        &SUBSCRIBER
    }

    #[inline(always)]
    fn name() -> &'static str {
        &SUBSCRIBER.name
    }
}

#[derive(Debug, Clone)]
pub struct ProjectorConfig {
    pub name: String,
    pub hostname: String,
    pub bootstrap: String,
    pub capacity: usize,
    pub partitions: usize,
    pub interval: u64,
    pub tries: usize,
    pub secs: u64,
    pub pc: HashMap<String, String>,
    pub pp: HashMap<String, String>,
}

static PROJECTOR: LazyLock<ProjectorConfig> = LazyLock::new(|| load_projector());

impl domain::Config for ProjectorConfig {
    #[inline(always)]
    fn get() -> &'static Self {
        &PROJECTOR
    }

    #[inline(always)]
    fn name() -> &'static str {
        &PROJECTOR.name
    }
}

#[derive(Debug, Clone)]
pub struct SenderConfig {
    pub name: String,
    pub hostname: String,
    pub bootstrap: String,
    pub timeout: Duration,
    pub sender: NamedConfig<SendConfig>,
    pub tc: HashMap<String, HashMap<String, String>>,
    pub cp: HashMap<String, String>,
}

static SENDER: LazyLock<SenderConfig> = LazyLock::new(|| load_sender());

impl domain::Config for SenderConfig {
    #[inline(always)]
    fn get() -> &'static Self {
        &SENDER
    }

    #[inline(always)]
    fn name() -> &'static str {
        &SENDER.name
    }
}
