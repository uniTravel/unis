//! 配置管理
//!
//! 全局共享配置。[`UniConfig`] 是配置根，内含一般配置与命名配置 [`NamedConfig`] 。
//!
//! > 【**命名配置**】当需要多个并排、结构相同的配置节时，可通过命名配置处理。例如：
//! > ```toml
//! > [aggregates.note]
//! > capacity = 100
//! > interval = 120
//! > low = 200
//! > high = 20000
//! > retain = 7200
//! > [aggregates.author]
//! > capacity = 20
//! > interval = 120
//! > low = 100
//! > high = 2000
//! > retain = 7200
//! > ```
//! > 以上配置文件，aggregates 包含多个同构配置，可通过 "note"、"author" 分别获取。
//!
//! # 配置如何加载
//!
//! 首先，系统会尝试读取 UNI_CONFIG_ROOT 环境变量，有则以此作为配置文件访问路径，
//! 否则以当前 crate 根路径下的 config 文件夹作为配置文件访问路径。然后按以下顺序加载：
//!
//! 1. 名为 default 的配置文件。
//! 2. 根据环境变量 UNI_ENV 命名的配置文件，若无此环境变量，则以 dev 为配置文件名。
//! 3. 读取环境变量，环境变量以UNI为前缀，并以双下划线分隔。
//!
//! 后加载的配置会覆盖先加载的配置。
//!
use crate::errors::ConfigError;
use config::{Config, Environment, File};
use serde::{Deserialize, de::DeserializeOwned};
use std::{collections::HashMap, fmt::Debug, path::PathBuf};
use validator::Validate;

/// 命名配置
#[derive(Debug, Clone)]
pub struct NamedConfig<T> {
    configs: HashMap<String, T>,
}

impl<T> NamedConfig<T>
where
    T: DeserializeOwned + Validate + Clone + Send + Sync + Default + 'static,
{
    /// 获取命名配置
    pub fn get(&self, name: &str) -> T {
        self.configs.get(name).cloned().unwrap_or_default()
    }

    /// 获取缺省配置
    pub fn default(&self) -> T {
        T::default()
    }

    /// 获取命名配置键集合
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.configs.keys()
    }
}

/// 构建配置
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

/// 加载命名配置
///
/// 载入 [`NamedConfig`] 结构体。
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

/// 聚合配置
#[derive(Debug, Deserialize, Validate, Clone)]
#[serde(default)]
pub struct AggConfig {
    /// 聚合器背压容量
    pub capacity: usize,
    /// 聚合缓存刷新间隔，单位秒
    pub interval: u64,
    /// 聚合缓存容量下限
    pub low: usize,
    /// 聚合缓存容量上限
    pub high: usize,
    /// 聚合缓存保留时长，单位秒
    pub retain: u64,
}

impl Default for AggConfig {
    fn default() -> Self {
        Self {
            capacity: 100,
            interval: 2 * 60,
            low: 200,
            high: 20000,
            retain: 2 * 24 * 60 * 60,
        }
    }
}
