use crate::{
    config::{AggConfig, NamedConfig, build_config, load_named_config},
    domain,
    errors::ConfigError,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{OnceLock, RwLock},
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
    let bootstrap = config.get("bootstrap").unwrap();
    let cc = load_named_setting(&config, "cc").unwrap();
    let tp = config.get::<HashMap<String, String>>("tp").unwrap();
    SubscriberConfig {
        aggregates,
        bootstrap,
        cc,
        tp,
    }
}

fn load_sender() -> SenderConfig {
    let config = build_config(PathBuf::from(env!("CARGO_MANIFEST_DIR"))).unwrap();
    let aggregates = load_named_config::<AggConfig>(&config, "aggregates").unwrap();
    let bootstrap = config.get("bootstrap").unwrap();
    SenderConfig {
        aggregates,
        bootstrap,
    }
}

#[derive(Debug, Clone)]
pub struct SubscriberConfig {
    pub aggregates: NamedConfig<AggConfig>,
    pub bootstrap: String,
    pub cc: HashMap<String, HashMap<String, String>>,
    pub tp: HashMap<String, String>,
}

impl domain::Config for SubscriberConfig {
    fn get() -> Result<Self, ConfigError> {
        let cfg = SUBSCRIBER_CONFIG
            .get_or_init(|| RwLock::new(load_subscriber()))
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

#[derive(Debug, Clone)]
pub struct SenderConfig {
    pub aggregates: NamedConfig<AggConfig>,
    pub bootstrap: String,
}

impl domain::Config for SenderConfig {
    fn get() -> Result<Self, ConfigError> {
        let cfg = SENDER_CONFIG
            .get_or_init(|| RwLock::new(load_sender()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Config;

    #[test]
    fn test_config_aggregate() {
        let cfg = SubscriberConfig::get().unwrap();
        let agg = cfg.aggregates.get("note");
        assert_eq!(agg.interval, 120);
        assert_eq!(agg.low, 200);
        assert_eq!(agg.high, 20000);
        assert_eq!(agg.retain, 7200);
        assert_eq!(agg.capacity, 100);
    }

    #[test]
    fn test_config_subscriber() {
        let cfg = SubscriberConfig::get().unwrap();
        assert_eq!(cfg.bootstrap, "localhost:9092");
        let cc = cfg.cc.get("note").unwrap();
        assert_eq!(cc.get("group.id").unwrap(), "cc-group");
        let tp = &cfg.tp;
        assert_eq!(tp.get("linger.ms").unwrap(), "1");
    }

    #[test]
    fn test_config_sender() {
        let cfg = SubscriberConfig::get().unwrap();
        assert_eq!(cfg.bootstrap, "localhost:9092");
    }
}
