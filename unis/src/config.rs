//! # **unis** 核心配置
//!
//!

use config::{Config, Environment, File};
use serde::{Deserialize, de::DeserializeOwned};
use std::{collections::HashMap, fmt::Debug, path::PathBuf};
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
    pub fn get(&'static self, name: &str) -> &'static T {
        self.configs.get(name).expect("命名配置不存在")
    }
}

#[cfg(feature = "sender")]
pub(crate) fn i18n_config() -> Config {
    let config_root = std::env::var("UNI_CONFIG_ROOT")
        .map(PathBuf::from)
        .unwrap_or(PathBuf::from("./config"));
    match Config::builder()
        .add_source(File::from(config_root.join("i18n")).required(false))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            panic!("加载配置失败：{e}");
        }
    }
}

/// 构建配置
pub fn build_config() -> Config {
    let config_root = std::env::var("UNI_CONFIG_ROOT")
        .map(PathBuf::from)
        .unwrap_or(PathBuf::from("./config"));
    let env = std::env::var("UNI_ENV").unwrap_or("dev".to_owned());
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
            panic!("加载配置失败：{e}");
        }
    }
}

/// 加载产品名称
pub fn load_name(cfg: &::config::Config) -> String {
    match cfg.get::<String>("name") {
        Ok(c) => {
            let len = c.chars().count();
            if len < 2 || len > 10 {
                panic!("长度应介于 2 到 10 之间");
            }
            if !c.chars().all(|c| c.is_ascii_alphabetic()) {
                panic!("应为 ASCII 字母");
            }
            c
        }
        Err(e) => {
            panic!("加载'name'配置失败：{e}");
        }
    }
}

/// 加载 Hostname
pub fn load_hostname(cfg: &::config::Config) -> String {
    match cfg.get("hostname") {
        Ok(c) => c,
        Err(e) => {
            panic!("加载'hostname'配置失败：{e}");
        }
    }
}

/// 加载配置到命名配置结构
pub fn load_named_config<T>(config: &Config, section: &str) -> NamedConfig<T>
where
    T: DeserializeOwned + Validate + Clone + Default,
{
    let configs = match config.get::<HashMap<String, T>>(section) {
        Ok(c) => c,
        Err(e) => {
            panic!("加载命名配置'{section}'失败：{e}");
        }
    };

    for (key, cfg) in &configs {
        if let Err(e) = cfg.validate() {
            panic!("[{section}.{key}]命名配置验证失败：{e}");
        }
    }

    NamedConfig { configs }
}

/// 加载命名配置
pub fn load_named_setting(
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
                panic!("加载命名配置'{section}'失败：{e}");
            }
        };
        result.insert(section, map);
    }

    result
}

/// 订阅者聚合配置结构
#[derive(Debug, Deserialize, Validate, Clone)]
#[serde(default)]
pub struct SubscribeConfig {
    /// 是否热点
    pub hotspot: bool,
    /// 缓存刷新间隔， 单位分钟
    pub interval: u64,
    /// 缓存容量下限
    pub low: usize,
    /// 缓存容量上限
    pub high: usize,
    /// 缓存保留时长，单位小时
    pub retain: u64,
}

impl Default for SubscribeConfig {
    fn default() -> Self {
        Self {
            hotspot: false,
            interval: 30,
            low: 200,
            high: 20000,
            retain: 2 * 24,
        }
    }
}

/// 发送者聚合配置结构
#[derive(Debug, Deserialize, Validate, Clone)]
#[serde(default)]
pub struct SendConfig {
    /// 是否热点
    pub hotspot: bool,
    /// 缓存刷新间隔， 单位分钟
    pub interval: u64,
    /// 缓存保留时长，单位分钟
    pub retain: u64,
}

impl Default for SendConfig {
    fn default() -> Self {
        Self {
            hotspot: false,
            interval: 5,
            retain: 30,
        }
    }
}
