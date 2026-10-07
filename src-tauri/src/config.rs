//! 应用配置：曲库目录白名单、在线模块开关、启动扫描等

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// 持久化到 config.json 的应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 曲库目录白名单（同时作为扫描根目录）
    #[serde(rename = "libraryDirs")]
    pub library_dirs: Vec<String>,
    /// 在线模块开关（默认关闭）
    #[serde(rename = "scanOnStartup")]
    pub scan_on_startup: bool,
    /// 界面语言
    pub locale: String,
    /// 在线服务配置
    pub online: OnlineConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineConfig {
    /// 在线模块是否启用
    pub enabled: bool,
    /// 是否允许上传本地曲库信息（默认 false，且即使 enabled=true 也需此为 true 才上传）
    #[serde(rename = "allowLibraryUpload")]
    pub allow_library_upload: bool,
    /// 在线服务提供商
    pub provider: String,
}

impl Default for OnlineConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            allow_library_upload: false,
            provider: "none".to_string(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            library_dirs: Vec::new(),
            scan_on_startup: true,
            locale: "zh-CN".to_string(),
            online: OnlineConfig::default(),
        }
    }
}

impl AppConfig {
    pub fn load(path: &PathBuf) -> Self {
        match std::fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, path: &PathBuf) -> AppResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::Config(format!("创建配置目录失败: {}", e)))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| AppError::Config(format!("序列化配置失败: {}", e)))?;
        std::fs::write(path, json)
            .map_err(|e| AppError::Config(format!("写入 config.json 失败: {}", e)))?;
        Ok(())
    }
}
