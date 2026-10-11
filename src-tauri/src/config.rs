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
    /// 交叉淡化配置
    pub crossfade: CrossfadeConfig,
    /// 音量归一化 ReplayGain 配置（旧配置缺省时回落默认：开启、目标 -14 LUFS）
    #[serde(default)]
    pub replaygain: ReplayGainConfig,
    /// 目录自动监控配置（旧配置缺省时回落默认：开启）
    #[serde(default)]
    pub watch: WatchConfig,
}

/// 目录自动监控配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchConfig {
    /// 是否启用曲库目录自动监控（默认开启：放新歌自动入库、删除自动移除）
    #[serde(rename = "enabled")]
    pub enabled: bool,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// 音量归一化 ReplayGain 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayGainConfig {
    /// 是否启用（默认开启）
    #[serde(rename = "enabled")]
    pub enabled: bool,
    /// 目标综合响度（LUFS，默认 -14，与主流流媒体平台一致）
    #[serde(rename = "targetLufs")]
    pub target_lufs: f32,
}

impl Default for ReplayGainConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            target_lufs: -14.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossfadeConfig {
    /// 是否启用交叉淡化（默认开启）
    #[serde(rename = "enabled")]
    pub enabled: bool,
    /// 交叉淡化时长（毫秒，0~3000，默认 1000ms）
    pub duration_ms: u64,
}

impl Default for CrossfadeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            duration_ms: 1000,
        }
    }
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
            crossfade: CrossfadeConfig::default(),
            replaygain: ReplayGainConfig::default(),
            watch: WatchConfig::default(),
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
