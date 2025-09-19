use crate::{
    config::{AggConfig, NamedConfig, build_config, load_named_config},
    domain,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{OnceLock, RwLock},
};
use tokio::time::Duration;
use tracing::error;

static SUBSCRIBER_CONFIG: OnceLock<RwLock<SubscriberConfig>> = OnceLock::new();
static SENDER_CONFIG: OnceLock<RwLock<SenderConfig>> = OnceLock::new();

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

fn load_subscriber() -> SubscriberConfig {
    let config = build_config(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    let bootstrap = match config.get("bootstrap") {
        Ok(c) => c,
        Err(e) => {
            error!("加载'bootstrap'配置失败：{e}");
            panic!("加载'bootstrap'配置失败");
        }
    };
    let aggs = config.get("aggs").unwrap_or(16);
    let timeout = config.get("timeout").unwrap_or(Duration::from_secs(45));
    let aggregates = load_named_config::<AggConfig>(&config, "aggregates");
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
        aggs,
        timeout,
        aggregates,
        cc,
        tp,
    }
}

fn load_sender() -> SenderConfig {
    let config = build_config(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    let bootstrap = match config.get("bootstrap") {
        Ok(c) => c,
        Err(e) => {
            error!("加载'bootstrap'配置失败：{e}");
            panic!("加载'bootstrap'配置失败");
        }
    };
    let hostname = match config.get("hostname") {
        Ok(c) => c,
        Err(e) => {
            error!("加载'hostname'配置失败：{e}");
            panic!("加载'hostname'配置失败");
        }
    };
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
        cp,
    }
}

#[derive(Debug, Clone)]
pub struct SubscriberConfig {
    pub bootstrap: String,
    pub aggs: usize,
    pub timeout: Duration,
    pub aggregates: NamedConfig<AggConfig>,
    pub cc: HashMap<String, HashMap<String, String>>,
    pub tp: HashMap<String, String>,
}

impl domain::Config for SubscriberConfig {
    fn get() -> Self {
        match SUBSCRIBER_CONFIG
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
        let cfg = match SUBSCRIBER_CONFIG.get() {
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
    pub cp: HashMap<String, String>,
}

impl domain::Config for SenderConfig {
    fn get() -> Self {
        match SENDER_CONFIG
            .get_or_init(|| RwLock::new(load_sender()))
            .read()
        {
            Ok(cfg) => cfg.clone(),
            Err(e) => {
                error!("获取发送者配置失败：{e}");
                panic!("获取发送者配置失败");
            }
        }
    }

    fn reload() {
        let cfg = match SENDER_CONFIG.get() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Config;

    #[test]
    fn test_config_aggregate() {
        let cfg = SubscriberConfig::get();
        let agg = cfg.aggregates.get("note");
        assert_eq!(agg.interval, 120);
        assert_eq!(agg.low, 200);
        assert_eq!(agg.high, 20000);
        assert_eq!(agg.retain, 7200);
        assert_eq!(agg.latest, 30);
        assert_eq!(agg.coms, 2000);
        assert_eq!(agg.capacity, 100);
        assert_eq!(agg.sems, 100);
    }

    #[test]
    fn test_config_subscriber() {
        let cfg = SubscriberConfig::get();
        assert_eq!(cfg.bootstrap, "localhost:9092");
        let cc = cfg.cc.get("note").unwrap();
        assert_eq!(cc.get("enable.auto.commit").unwrap(), "false");
        let tp = &cfg.tp;
        assert_eq!(tp.get("linger.ms").unwrap(), "1");
    }

    #[test]
    fn test_config_sender() {
        let cfg = SenderConfig::get();
        assert_eq!(cfg.bootstrap, "localhost:9092");
        let cp = &cfg.cp;
        assert_eq!(cp.get("linger.ms").unwrap(), "1");
    }
}
