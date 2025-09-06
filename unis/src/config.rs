use crate::errors::ConfigError;
use config::{Config, Environment, File};
use serde::{Deserialize, de::DeserializeOwned};
use std::{collections::HashMap, fmt::Debug, path::PathBuf};
use validator::Validate;

#[derive(Debug, Clone)]
pub(crate) struct NamedConfig<T> {
    configs: HashMap<String, T>,
}

impl<T> NamedConfig<T>
where
    T: DeserializeOwned + Validate + Clone + Send + Sync + Default + 'static,
{
    pub fn get(&self, name: &str) -> T {
        self.configs.get(name).cloned().unwrap_or_default()
    }
}

pub fn build_config(crate_dir: PathBuf) -> Result<Config, ConfigError> {
    let config_root = std::env::var("UNI_CONFIG_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| crate_dir.join("config"));
    let env = std::env::var("UNI_ENV").unwrap_or_else(|_| "dev".to_string());
    let config = Config::builder()
        .add_source(File::from(config_root.join("default")).required(false))
        .add_source(File::from(config_root.join(env)).required(false))
        .add_source(
            Environment::with_prefix("UNI")
                .separator("__")
                .list_separator(","),
        )
        .build()?;
    Ok(config)
}

pub fn load_named_config<T>(config: &Config, section: &str) -> Result<NamedConfig<T>, ConfigError>
where
    T: DeserializeOwned + Validate + Clone + Default,
{
    let configs = config.get::<HashMap<String, T>>(section)?;

    for (key, cfg) in &configs {
        cfg.validate().map_err(|e| ConfigError::ValidationError {
            section: section.to_string(),
            key: key.to_string(),
            message: e.to_string(),
        })?;
    }

    Ok(NamedConfig { configs })
}

#[derive(Debug, Deserialize, Validate, Clone)]
#[serde(default)]
pub struct AggConfig {
    pub interval: u64,
    pub low: usize,
    pub high: usize,
    pub retain: u64,
    pub coms: usize,
    pub capacity: usize,
    #[validate(range(min = 1))]
    pub concurrent: usize,
    #[validate(range(min = 7))]
    pub sems: usize,
}

impl Default for AggConfig {
    fn default() -> Self {
        Self {
            interval: 2 * 60,
            low: 200,
            high: 20000,
            retain: 2 * 24 * 60 * 60,
            coms: 2000,
            capacity: 100,
            concurrent: 16,
            sems: 100,
        }
    }
}
