//! ## Kubernetes 集群部署工具
//!
//!
#![cfg(feature = "test-utils")]

use crate::test_utils::command::CommandBuilder;
use std::{
    fs::{OpenOptions, TryLockError, create_dir_all},
    io::Write,
    path::PathBuf,
};
use tokio::time::{Duration, Instant, sleep};
use tracing::{info, warn};

/// Kubernetes 集群
pub struct KubeCluster {
    /// 命名空间
    pub namespace: String,
}

impl KubeCluster {
    /// Kubernetes 集群构造函数
    pub fn new(namespace: &str) -> Self {
        if namespace == "default" {
            panic!("命名空间不能设为 default")
        }
        KubeCluster {
            namespace: namespace.to_string(),
        }
    }

    /// 创建命名空间
    pub fn create_namespace(&self) -> Result<(), String> {
        let (success, stdout, stderr) = CommandBuilder::new("kubectl")
            .args(["create", "namespace", &self.namespace])
            .execute(&format!("create namespace {}", self.namespace))?;

        match (
            success,
            stderr.contains("AlreadyExists"),
            stdout.contains("AlreadyExists"),
        ) {
            (true, _, _) => {
                info!("成功创建命名空间 {}", self.namespace);
                Ok(())
            }
            (false, true, _) | (false, _, true) => Ok(()),
            (false, _, _) => Err(format!(
                "创建命名空间 {} 失败：Stdout: {}；Stderr: {}",
                self.namespace,
                stdout.trim(),
                stderr.trim()
            )),
        }
    }

    /// 删除命名空间
    pub fn delete_namespace(&self) -> Result<(), String> {
        let (success, stdout, stderr) = CommandBuilder::new("kubectl")
            .args(["delete", "namespace", &self.namespace])
            .execute(&format!("delete namespace {}", self.namespace))?;

        if success {
            info!("成功删除命名空间 {}", self.namespace);
            Ok(())
        } else {
            Err(format!(
                "删除命名空间 {} 失败：Stdout: {}；Stderr: {}",
                self.namespace,
                stdout.trim(),
                stderr.trim()
            ))
        }
    }
}

/// Helm 发布
pub struct HelmRelease {
    name: String,
    chart: PathBuf,
    namespace: String,
    lock_dir: PathBuf,
}

impl HelmRelease {
    /// Helm 发布构造函数
    pub fn new(name: &str, chart: PathBuf, namespace: &str) -> Self {
        let lock_dir = std::env::temp_dir().join(name);

        HelmRelease {
            name: name.to_string(),
            chart,
            namespace: namespace.to_string(),
            lock_dir,
        }
    }

    fn lock_file(&self) -> PathBuf {
        create_dir_all(&self.lock_dir).unwrap();
        self.lock_dir.join(format!("{}.lock", self.name))
    }

    async fn status(&self) -> Result<bool, String> {
        let timeout = Duration::from_secs(5);
        let start = Instant::now();

        while start.elapsed() < timeout {
            let (success, stdout, stderr) = CommandBuilder::new("helm")
                .args(["status", &self.name, "--namespace", &self.namespace])
                .execute(&format!(
                    "status {} --namespace {}",
                    &self.name, &self.namespace
                ))?;

            if success {
                let status_line = stdout
                    .lines()
                    .find(|&l| l.starts_with("STATUS"))
                    .ok_or(format!("Helm 发布 {} 未包含状态信息", self.name))?;
                match status_line[7..].trim() {
                    "deployed" => return Ok(true),
                    status => info!("Helm 发布 {} 状态：{status}", self.name),
                }
            } else if stderr.contains("not found") {
                return Ok(false);
            } else {
                return Err(format!(
                    "获取 Helm 发布 {} 状态失败：Stdout：{}；Stderr：{}",
                    self.name,
                    stdout.trim(),
                    stderr.trim()
                ));
            }

            sleep(Duration::from_millis(500)).await;
        }
        Err(format!("Helm 发布 {} 获取就绪状态超时", self.name))
    }

    async fn wait_for_existing_installation(&self) -> Result<(), String> {
        let timeout = Duration::from_secs(5);
        let start = Instant::now();

        while start.elapsed() < timeout {
            if self.status().await? {
                info!("等待的 Helm 发布 {} 已完成", self.name);
                return Ok(());
            }

            if !self.lock_file().exists() {
                if self.status().await? {
                    info!("等待的 Helm 发布 {} 已完成", self.name);
                    return Ok(());
                }
            }

            sleep(Duration::from_millis(200)).await;
        }
        Err(format!("等待 Helm 发布 {} 超时", self.name))
    }

    fn acquire_lock(&self) -> Result<FileLock, TryLockError> {
        let path = self.lock_file();
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .read(true)
            .open(&path)
            .map_err(|e| TryLockError::Error(e))?;

        match file.try_lock() {
            Ok(()) => {
                writeln!(&file, "PID:{}", std::process::id())
                    .map_err(|e| TryLockError::Error(e))?;
                file.sync_all().map_err(|e| TryLockError::Error(e))?;
            }
            Err(e) => return Err(e),
        }
        Ok(FileLock {
            file: Some(file),
            path,
        })
    }

    /// 安装 Helm 发布
    pub async fn install(&self, values: Option<&str>) -> Result<(), String> {
        if self.status().await? {
            return Ok(());
        }

        let _lock = match self.acquire_lock() {
            Ok(lock) => {
                info!("Helm 发布 {} 取到文件锁", self.name);
                lock
            }
            Err(TryLockError::WouldBlock) => {
                return self.wait_for_existing_installation().await;
            }
            Err(TryLockError::Error(e)) => {
                return Err(format!("Helm 发布 {} 获取文件锁失败：{e}", self.name));
            }
        };

        let args = [
            "install",
            &self.name,
            &self.chart.to_string_lossy(),
            "--namespace",
            &self.namespace,
            "--wait",
        ];
        let (success, stdout, stderr) = match values {
            Some(values_content) => {
                let mut arg = String::with_capacity(values_content.len() + 6);
                arg.push_str("--set ");
                arg.push_str(values_content);
                CommandBuilder::new("helm")
                    .args(args)
                    .arg(arg)
                    .execute(&format!("install {} {}", self.name, self.chart.display()))?
            }
            None => CommandBuilder::new("helm").args(args).execute(&format!(
                "install {} {}",
                self.name,
                self.chart.display()
            ))?,
        };

        if success {
            info!("成功安装 Helm 发布 {}", self.name);
            Ok(())
        } else {
            Err(format!(
                "安装 Helm 发布 {} 失败：Stdout: {}；Stderr: {}",
                self.name,
                stdout.trim(),
                stderr.trim()
            ))
        }
    }

    /// 卸载 Helm 发布
    pub fn uninstall(&self) -> Result<(), String> {
        let (success, stdout, stderr) = CommandBuilder::new("helm")
            .args([
                "uninstall",
                &self.name,
                "--namespace",
                &self.namespace,
                "--wait",
            ])
            .execute(&format!("uninstall {}", self.name))?;

        if success {
            info!("成功卸载 Helm 发布 {}", self.name);
            Ok(())
        } else if stderr.contains("not found") {
            info!("Helm 发布 {} 不存在", self.name);
            Ok(())
        } else {
            Err(format!(
                "卸载 Helm 发布 {} 失败：Stdout: {}；Stderr: {}",
                self.name,
                stdout.trim(),
                stderr.trim()
            ))
        }
    }
}

struct FileLock {
    file: Option<std::fs::File>,
    path: PathBuf,
}

impl Drop for FileLock {
    fn drop(&mut self) {
        if let Some(file) = self.file.take() {
            let _ = file.sync_all();
            drop(file);
        }

        std::thread::sleep(std::time::Duration::from_millis(50));

        match std::fs::remove_file(&self.path) {
            Ok(()) => {
                info!("[PID:{}] 成功删除锁文件", std::process::id());
            }
            Err(e) => {
                warn!("[PID:{}] 删除锁文件失败：{e}", std::process::id());
            }
        }
    }
}
