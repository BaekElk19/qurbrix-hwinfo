use async_trait::async_trait;
use std::collections::BTreeMap;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tracing::{debug, error, info};

use anyhow::{anyhow, Result};

/// 命令规范定义
#[derive(Debug, Clone)]
pub struct CmdSpec {
    pub key: String,
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

impl CmdSpec {
    pub fn new(key: &str, program: &str, args: Vec<String>) -> Self {
        Self {
            key: key.to_string(),
            program: program.to_string(),
            args,
            env: Vec::new(),
        }
    }

    pub fn with_env(mut self, key: &str, value: &str) -> Self {
        self.env.push((key.to_string(), value.to_string()));
        self
    }
}

/// 命令采集源
#[derive(Debug, Clone)]
pub struct CmdSource {
    pub commands: Vec<CmdSpec>,
    pub timeout_ms: u64,
}

impl CmdSource {
    pub fn new(commands: Vec<CmdSpec>, timeout_ms: u64) -> Self {
        Self {
            commands,
            timeout_ms,
        }
    }

    pub async fn run(&self, program: &str, args: &[&str], timeout_ms: u64) -> Result<String> {
        let timeout = if timeout_ms == 0 {
            self.timeout_ms
        } else {
            timeout_ms
        };
        let spec = CmdSpec {
            key: "__single__".into(),
            program: program.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            env: Vec::new(),
        };
        let (_key, output) = execute_command(spec, Duration::from_millis(timeout)).await?;
        Ok(output)
    }
}

/// 采集器 trait
#[async_trait]
pub trait Source {
    async fn collect(&self) -> Result<BTreeMap<String, String>>;
}

#[async_trait]
impl Source for CmdSource {
    async fn collect(&self) -> Result<BTreeMap<String, String>> {
        let mut results = BTreeMap::new();
        let mut tasks = Vec::new();

        // 为每个命令创建异步任务
        for cmd_spec in &self.commands {
            let spec = cmd_spec.clone();
            let timeout = Duration::from_millis(self.timeout_ms);

            tasks.push(tokio::spawn(
                async move { execute_command(spec, timeout).await },
            ));
        }

        // 等待所有任务完成
        for task in tasks {
            match task.await {
                Ok(Ok((key, output))) => {
                    results.insert(key, output);
                }
                Ok(Err(e)) => {
                    error!("Command execution failed: {}", e);
                }
                Err(e) => {
                    error!("Task join error: {}", e);
                }
            }
        }

        info!("Collected {} command outputs", results.len());
        Ok(results)
    }
}

/// 执行单个命令
async fn execute_command(cmd_spec: CmdSpec, timeout: Duration) -> Result<(String, String)> {
    debug!(
        "Executing command: {} {:?}",
        cmd_spec.program, cmd_spec.args
    );

    let mut command = Command::new(&cmd_spec.program);

    // 设置参数
    command.args(&cmd_spec.args);

    // 设置环境变量
    for (key, value) in &cmd_spec.env {
        command.env(key, value);
    }

    // 配置命令执行
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command.kill_on_drop(true);

    // 执行命令并设置超时
    let output = tokio::time::timeout(timeout, command.output()).await??;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        debug!("Command {} completed successfully", cmd_spec.key);
        Ok((cmd_spec.key, stdout))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(anyhow!(
            "Command {} failed with status {}: {}",
            cmd_spec.key,
            output.status,
            stderr
        ))
    }
}

/// 文件采集源（用于读取 /sys, /proc 等）
#[derive(Debug, Clone)]
pub struct FileSource {
    pub files: Vec<FileSpec>,
}

#[derive(Debug, Clone)]
pub struct FileSpec {
    pub key: String,
    pub path: String,
}

impl FileSpec {
    pub fn new(key: &str, path: &str) -> Self {
        Self {
            key: key.to_string(),
            path: path.to_string(),
        }
    }
}

#[async_trait]
impl Source for FileSource {
    async fn collect(&self) -> Result<BTreeMap<String, String>> {
        let mut results = BTreeMap::new();

        for file_spec in &self.files {
            match tokio::fs::read_to_string(&file_spec.path).await {
                Ok(content) => {
                    results.insert(file_spec.key.clone(), content);
                }
                Err(e) => {
                    error!("Failed to read file {}: {}", file_spec.path, e);
                    // 文件读取失败时插入空字符串而不是错误
                    results.insert(file_spec.key.clone(), String::new());
                }
            }
        }

        info!("Collected {} file contents", results.len());
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cmd_source_creation() {
        let source = CmdSource::new(
            vec![CmdSpec::new("echo_test", "echo", vec!["hello".to_string()])],
            5000,
        );

        let result = source.collect().await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.contains_key("echo_test"));
    }

    #[tokio::test]
    async fn test_file_source_creation() {
        let source = FileSource {
            files: vec![FileSpec::new("cargo_toml", "Cargo.toml")],
        };

        let result = source.collect().await;
        assert!(result.is_ok());
    }
}
