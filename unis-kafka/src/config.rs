use std::{collections::HashMap, path::PathBuf, sync::OnceLock, sync::RwLock};
use unis::{
    config::{AggConfig, NamedConfig, build_config, load_named_config},
    domain,
    errors::ConfigError,
};

static SUBSCRIBER_CONFIG: OnceLock<RwLock<SubscriberConfig>> = OnceLock::new();
static SENDER_CONFIG: OnceLock<RwLock<SenderConfig>> = OnceLock::new();

fn load_named_setting(
    config: &config::Config,
    section: &str,
) -> Result<HashMap<String, HashMap<String, String>>, ConfigError> {
    let mut result = HashMap::new();
    let input = config
        .get::<HashMap<String, config::Value>>(section)
        .unwrap_or(HashMap::new());

    for (section, value) in input {
        let map = value.try_deserialize::<HashMap<String, String>>()?;
        result.insert(section, map);
    }

    Ok(result)
}

fn load_subscriber() -> SubscriberConfig {
    let config = build_config(PathBuf::from(env!("CARGO_MANIFEST_DIR"))).unwrap();
    let aggregates = load_named_config::<AggConfig>(&config, "aggregates").unwrap();
    let cc = load_named_setting(&config, "cc").unwrap();
    let ep = load_named_setting(&config, "ep").unwrap();
    SubscriberConfig { aggregates, cc, ep }
}

fn load_sender() -> SenderConfig {
    let config = build_config(PathBuf::from(env!("CARGO_MANIFEST_DIR"))).unwrap();
    let aggregates = load_named_config::<AggConfig>(&config, "aggregates").unwrap();
    SenderConfig { aggregates }
}

/// 订阅者配置
#[derive(Debug, Clone)]
pub struct SubscriberConfig {
    /// 聚合配置集合
    pub aggregates: NamedConfig<AggConfig>,
    /// 命令消费者配置集合
    pub cc: HashMap<String, HashMap<String, String>>,
    /// 事件生产者配置集合
    pub ep: HashMap<String, HashMap<String, String>>,
}

impl domain::Config for SubscriberConfig {
    fn initialize() {
        SUBSCRIBER_CONFIG.get_or_init(|| RwLock::new(load_subscriber()));
    }

    fn get() -> Result<Self, ConfigError> {
        let cfg = SUBSCRIBER_CONFIG
            .get()
            .ok_or(ConfigError::NotInitialized)?
            .read()
            .map_err(|_| ConfigError::ReadFailed)?;
        Ok(cfg.clone())
    }

    fn reload() -> Result<(), ConfigError> {
        let mut cell = SUBSCRIBER_CONFIG
            .get()
            .ok_or(ConfigError::NotInitialized)?
            .write()
            .map_err(|_| ConfigError::WriteFailed)?;
        *cell = load_subscriber();
        Ok(())
    }
}

/// 发送者配置
#[derive(Debug, Clone)]
pub struct SenderConfig {
    /// 聚合配置集合
    pub aggregates: NamedConfig<AggConfig>,
}

impl domain::Config for SenderConfig {
    fn initialize() {
        SENDER_CONFIG.get_or_init(|| RwLock::new(load_sender()));
    }

    fn get() -> Result<Self, ConfigError> {
        let cfg = SENDER_CONFIG
            .get()
            .ok_or(ConfigError::NotInitialized)?
            .read()
            .map_err(|_| ConfigError::ReadFailed)?;
        Ok(cfg.clone())
    }

    fn reload() -> Result<(), ConfigError> {
        let mut cell = SENDER_CONFIG
            .get()
            .ok_or(ConfigError::NotInitialized)?
            .write()
            .map_err(|_| ConfigError::WriteFailed)?;
        *cell = load_sender();
        Ok(())
    }
}
