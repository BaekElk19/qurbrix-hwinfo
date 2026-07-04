use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::SystemTime;
use anyhow::{anyhow, Result};

use hw_model::*;
use hw_collect::{collect_system_info, refresh_system_info};

/// API 服务结构体
pub struct HardwareApi {
    cache: Option<SystemInfo>,
    last_update: Option<SystemTime>,
}

impl HardwareApi {
    pub fn new() -> Self {
        Self {
            cache: None,
            last_update: None,
        }
    }

    /// 获取完整的系统信息（JSON 格式）
    pub async fn get_system_info(&mut self) -> Result<Value> {
        let info = collect_system_info().await?;
        self.cache = Some(info.clone());
        self.last_update = Some(SystemTime::now());
        
        Ok(serde_json::to_value(info)?)
    }

    /// 强制刷新系统信息
    pub async fn refresh_system_info(&mut self) -> Result<Value> {
        let info = refresh_system_info().await?;
        self.cache = Some(info.clone());
        self.last_update = Some(SystemTime::now());
        
        Ok(serde_json::to_value(info)?)
    }

    /// 获取特定设备类型的信息
    pub async fn get_device_info(&mut self, device_kind: &str) -> Result<Value> {
        let system_info = collect_system_info().await?;
        
        let devices = match device_kind.to_lowercase().as_str() {
            "cpu" | "cpus" => json!(system_info.cpus),
            "memory" | "ram" => json!(system_info.memory),
            "storage" | "disk" => json!(system_info.storage),
            "gpu" | "gpus" => json!(system_info.gpus),
            "network" => json!(system_info.network),
            "bios" => json!(system_info.bios),
            "board" => json!(system_info.board),
            "battery" => json!(system_info.battery),
            _ => return Err(anyhow!("Unknown device type: {}", device_kind)),
        };
        
        Ok(devices)
    }

    /// 获取原始命令输出（兼容 Deepin 接口）
    pub async fn get_raw_info(&self, key: &str) -> Result<String> {
        // 这里可以实现特定命令的原始输出
        // 暂时返回占位信息
        match key {
            "lscpu" => Ok("# lscpu output\nArchitecture: x86_64\nCPU(s): 8".to_string()),
            "dmidecode" => Ok("# dmidecode output\nHandle 0x0000".to_string()),
            "lsblk" => Ok("# lsblk output\nNAME SIZE TYPE".to_string()),
            "lspci" => Ok("# lspci output\n00:00.0 Host bridge".to_string()),
            _ => Err(anyhow!("Unknown command key: {}", key)),
        }
    }

    /// 获取健康状态报告
    pub async fn get_health_report(&mut self) -> Result<Value> {
        let system_info = collect_system_info().await?;
        
        let mut report = BTreeMap::new();
        report.insert("timestamp", json!(SystemTime::now()));
        report.insert("schema_version", json!(system_info.schema_version));
        
        // CPU 健康检查
        let cpu_health = if system_info.cpus.is_empty() {
            "warning"
        } else {
            "healthy"
        };
        report.insert("cpu", json!(cpu_health));
        
        // 内存健康检查
        let memory_health = if system_info.memory.is_empty() {
            "warning"
        } else {
            "healthy"
        };
        report.insert("memory", json!(memory_health));
        
        // 存储健康检查
        let storage_health = if system_info.storage.is_empty() {
            "warning"
        } else {
            "healthy"
        };
        report.insert("storage", json!(storage_health));
        
        Ok(json!(report))
    }

    /// 获取支持的设备类型列表
    pub fn get_supported_devices() -> Value {
        json!([
            "cpu", "memory", "storage", "gpu", "network", 
            "bios", "board", "battery"
        ])
    }

    /// 获取 API 版本信息
    pub fn get_version_info() -> Value {
        json!({
            "name": "deepin-devicemanager-rust",
            "version": env!("CARGO_PKG_VERSION"),
            "schema_version": "1.0.0",
            "features": ["hardware_detection", "json_output", "caching"]
        })
    }
}

/// DBus 服务实现（可选）
#[cfg(feature = "dbus")]
pub mod dbus_service {
    use super::*;
    use zbus::{interface, ConnectionBuilder};

    #[interface(name = "com.deepin.DeviceManager")]
    struct DeviceManagerInterface {
        api: HardwareApi,
    }

    #[zbus::interface]
    impl DeviceManagerInterface {
        async fn get_info(&self, key: String) -> zbus::fdo::Result<String> {
            self.api.get_raw_info(&key).await
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
        }

        async fn refresh_info(&mut self) -> zbus::fdo::Result<()> {
            self.api.refresh_system_info().await
                .map(|_| ())
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
        }

        async fn get_system_info(&mut self) -> zbus::fdo::Result<String> {
            self.api.get_system_info().await
                .and_then(|v| serde_json::to_string(&v).map_err(|e| anyhow!(e)))
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
        }
    }

    /// 启动 DBus 服务
    pub async fn start_dbus_service() -> Result<()> {
        let interface = DeviceManagerInterface {
            api: HardwareApi::new(),
        };

        let _connection = ConnectionBuilder::session()?
            .name("com.deepin.DeviceManager")?
            .serve_at("/com/deepin/DeviceManager", interface)?
            .build()
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_api_creation() {
        let api = HardwareApi::new();
        // 测试 API 创建成功
        assert!(api.cache.is_none());
    }

    #[tokio::test]
    async fn test_version_info() {
        let version = HardwareApi::get_version_info();
        assert!(version["name"].is_string());
        assert!(version["version"].is_string());
    }

    #[tokio::test]
    async fn test_supported_devices() {
        let devices = HardwareApi::get_supported_devices();
        assert!(devices.is_array());
    }
}
