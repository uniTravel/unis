//! # **unis** 核心配置
//!
//!

use config::{Config, Environment, File};
use serde::{Deserialize, de::DeserializeOwned};
use std::{collections::HashMap, fmt::Debug, path::PathBuf};
use tracing::error;
use validator::Validate;

/// 命名配置结构
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
}

/// 构建配置
pub fn build_config(crate_dir: PathBuf) -> Config {
    let config_root = std::env::var("UNI_CONFIG_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| crate_dir.join("config"));
    let env = std::env::var("UNI_ENV").unwrap_or_else(|_| "dev".to_owned());
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

/// 加载命名配置
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

/// 订阅者聚合配置结构
#[derive(Debug, Deserialize, Validate, Clone)]
#[serde(default)]
pub struct SubscribeConfig {
    /// 是否热点
    pub hotspot: bool,
    /// 缓存刷新间隔， 单位秒
    pub interval: u64,
    /// 缓存容量下限
    pub low: usize,
    /// 缓存容量上限
    pub high: usize,
    /// 缓存保留时长，单位秒
    pub retain: u64,
    /// 恢复最近命令操作记录的时长，单位分钟
    #[validate(range(min = 2, max = 120))]
    pub latest: i64,
    /// 信号量
    #[validate(range(min = 7))]
    pub sems: usize,
}

impl Default for SubscribeConfig {
    fn default() -> Self {
        Self {
            hotspot: false,
            interval: 30 * 60,
            low: 200,
            high: 20000,
            retain: 2 * 24 * 60 * 60,
            latest: 30,
            sems: 100,
        }
    }
}

/// 发送者聚合配置结构
#[derive(Debug, Deserialize, Validate, Clone)]
#[serde(default)]
pub struct SendConfig {
    /// 是否热点
    pub hotspot: bool,
    /// 缓存刷新间隔， 单位秒
    pub interval: u64,
    /// 缓存保留时长，单位秒
    pub retain: u64,
    /// 信号量
    #[validate(range(min = 7))]
    pub sems: usize,
}

impl Default for SendConfig {
    fn default() -> Self {
        Self {
            hotspot: false,
            interval: 24 * 60 * 60,
            retain: 2 * 24 * 60 * 60,
            sems: 100,
        }
    }
}
