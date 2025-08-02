use crate::errors::ConfigError;
use config::{Config, Environment, File};
use serde::{Deserialize, de::DeserializeOwned};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, Once},
};
use validator::Validate;

struct GlobalConfig {
    config: Mutex<Option<Arc<UniConfig>>>,
    init_once: Once,
}

impl GlobalConfig {
    const fn new() -> Self {
        Self {
            config: Mutex::new(None),
            init_once: Once::new(),
        }
    }

    fn initialize(&self, new_config: UniConfig) -> Result<(), ConfigError> {
        let mut initialized = false;

        self.init_once.call_once(|| {
            *self.config.lock().unwrap() = Some(Arc::new(new_config));
            initialized = true;
        });

        if !initialized {
            Err(ConfigError::AlreadyInitialized)
        } else {
            Ok(())
        }
    }

    fn get(&self) -> Result<Arc<UniConfig>, ConfigError> {
        self.config
            .lock()
            .unwrap()
            .as_ref()
            .map(Arc::clone)
            .ok_or(ConfigError::NotInitialized)
    }

    fn reload(&self, new_config: UniConfig) -> Result<(), ConfigError> {
        *self.config.lock().unwrap() = Some(Arc::new(new_config));
        Ok(())
    }
}

static GLOBAL_CONFIG: GlobalConfig = GlobalConfig::new();

#[derive(Debug, Clone)]
pub struct NamedConfig<T> {
    configs: HashMap<String, T>,
}

impl<T> NamedConfig<T>
where
    T: DeserializeOwned + Validate + Clone + Send + Sync + Default + 'static,
{
    pub fn get(&self, name: &str) -> T {
        self.configs.get(name).cloned().unwrap_or_default()
    }

    pub fn default(&self) -> T {
        T::default()
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.configs.keys()
    }
}

fn load_config() -> Result<UniConfig, ConfigError> {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config_root = std::env::var("UNI_CONFIG_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| crate_dir.join("config"));
    let env = std::env::var("UNI_ENV").unwrap_or_else(|_| "dev".into());
    let config = Config::builder()
        .add_source(File::from(config_root.join("default")).required(false))
        .add_source(File::from(config_root.join(env)).required(false))
        .add_source(
            Environment::with_prefix("UNI")
                .separator("__")
                .list_separator(","),
        )
        .build()?;

    let aggregates = load_named_config::<AggConfig>(&config, "aggregates")?;
    Ok(UniConfig { aggregates })
}

fn load_named_config<T>(config: &Config, section: &str) -> Result<NamedConfig<T>, ConfigError>
where
    T: DeserializeOwned + Validate + Clone + Default,
{
    let configs = config.get::<HashMap<String, T>>(&format!("{}", section))?;

    for (key, cfg) in &configs {
        cfg.validate().map_err(|e| ConfigError::ValidationError {
            section: section.to_string(),
            key: key.to_string(),
            message: e.to_string(),
        })?;
    }

    Ok(NamedConfig { configs })
}

#[derive(Debug, Clone)]
pub struct UniConfig {
    pub aggregates: NamedConfig<AggConfig>,
}

impl UniConfig {
    pub fn initialize() -> Result<(), ConfigError> {
        let config = load_config()?;
        GLOBAL_CONFIG.initialize(config)
    }

    pub fn get() -> Result<Arc<Self>, ConfigError> {
        GLOBAL_CONFIG.get()
    }

    pub fn reload() -> Result<(), ConfigError> {
        let config = load_config()?;
        GLOBAL_CONFIG.reload(config)
    }
}

#[derive(Debug, Deserialize, Validate, Clone)]
#[serde(default)]
pub struct AggConfig {
    pub interval: u64,
    pub capacity: usize,
    pub cache_size: usize,
}

impl Default for AggConfig {
    fn default() -> Self {
        Self {
            interval: 15,
            capacity: 100,
            cache_size: 20000,
        }
    }
}
