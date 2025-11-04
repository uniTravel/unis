use std::path::PathBuf;

use tracing::{info, warn};

use crate::command::CommandBuilder;

pub struct KubeCluster {
    pub namespace: String,
}

impl KubeCluster {
    pub fn new(namespace: &str) -> Self {
        if namespace == "default" {
            panic!("命名空间不能设为 default")
        }
        KubeCluster {
            namespace: namespace.to_string(),
        }
    }

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
            (false, true, _) | (false, _, true) => {
                warn!("命名空间 {} 已存在", self.namespace);
                Ok(())
            }
            (false, _, _) => Err(format!(
                "创建命名空间 {} 失败：Stdout: {}；Stderr: {}",
                self.namespace,
                stdout.trim(),
                stderr.trim()
            )),
        }
    }

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

pub struct HelmRelease {
    pub name: String,
    pub chart: PathBuf,
    pub namespace: String,
}

impl HelmRelease {
    pub fn new(name: &str, chart: PathBuf, namespace: &str) -> Self {
        HelmRelease {
            name: name.to_string(),
            chart,
            namespace: namespace.to_string(),
        }
    }

    pub fn install(&self, values: Option<&str>) -> Result<(), String> {
        if self.status().is_ok() {
            return Ok(());
        }

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
            warn!("Helm 发布 {} 不存在", self.name);
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

    pub fn status(&self) -> Result<String, String> {
        let (success, stdout, stderr) = CommandBuilder::new("helm")
            .args(["status", &self.name, "--namespace", &self.namespace])
            .execute(&format!(
                "status {} --namespace {}",
                &self.name, &self.namespace
            ))?;

        if success {
            Ok(stdout)
        } else if stderr.contains("not found") {
            warn!("Helm 发布 {} 不存在", self.name);
            Err(format!("Helm 发布 {} 不存在", self.name))
        } else {
            Err(format!(
                "获取 Helm 发布 {} 状态失败：Stdout：{}；Stderr：{}",
                self.name,
                stdout.trim(),
                stderr.trim()
            ))
        }
    }
}
