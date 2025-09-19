use config::{Config, Environment, File};
use serde::{Deserialize, de::DeserializeOwned};
use std::{collections::HashMap, fmt::Debug, path::PathBuf};
use tracing::error;
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

pub fn build_config(crate_dir: PathBuf) -> Config {
    let config_root = std::env::var("UNI_CONFIG_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| crate_dir.join("config"));
    let env = std::env::var("UNI_ENV").unwrap_or_else(|_| "dev".to_string());
    match Config::builder()
        .add_source(File::from(config_root.join("default")).required(false))
        .add_source(File::from(config_root.join(env)).required(false))
        .add_source(
            Environment::with_prefix("UNI")
                .separator("__")
                .list_separator(","),
        )
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            error!("加载配置失败：{e}");
            panic!("加载配置失败");
        }
    }
}

pub fn load_named_config<T>(config: &Config, section: &str) -> NamedConfig<T>
where
    T: DeserializeOwned + Validate + Clone + Default,
{
    let configs = match config.get::<HashMap<String, T>>(section) {
        Ok(c) => c,
        Err(e) => {
            error!("加载命名配置'{section}'失败：{e}");
            panic!("加载命名配置失败");
        }
    };

    for (key, cfg) in &configs {
        if let Err(e) = cfg.validate() {
            error!("[{section}.{key}]命名配置验证失败：{e}");
            panic!("命名配置验证失败");
        }
    }

    NamedConfig { configs }
}

#[derive(Debug, Deserialize, Validate, Clone)]
#[serde(default)]
pub struct AggConfig {
    pub interval: u64,
    pub low: usize,
    pub high: usize,
    pub retain: u64,
    #[validate(range(min = 2, max = 120))]
    pub latest: i64,
    pub coms: usize,
    pub capacity: usize,
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
            latest: 30,
            coms: 2000,
            capacity: 100,
            sems: 100,
        }
    }
}
