//! 应用配置：曲库目录白名单、在线模块开关、启动扫描等

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// 持久化到 config.json 的应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 曲库目录白名单（同时作为扫描根目录）
    ///
    /// 注意：**刻意不加 `#[serde(default)]`**。`settings_save` 为整体替换语义，
    /// 若缺字段回落为空 Vec，会把用户已配置的曲库目录静默清空；此处宁可反序列化
    /// 报错（前端会记录日志），也不接受"缺字段 = 清空白名单"。
    #[serde(rename = "libraryDirs")]
    pub library_dirs: Vec<String>,
    /// 在线模块开关（默认关闭）
    #[serde(rename = "scanOnStartup")]
    pub scan_on_startup: bool,
    /// 界面语言
    pub locale: String,
    /// 在线服务配置
    #[serde(default)]
    pub online: OnlineConfig,
    /// 交叉淡化配置
    #[serde(default)]
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
#[serde(default)]
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
///
/// `#[serde(default)]`：容忍前端只发 `{ "enabled": true }`（历史 bug 根因——
/// 缺 `targetLufs` 曾使整个 `settings_save` 反序列化失败，进而曲库目录等设置无法落盘）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
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
#[serde(default)]
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
#[serde(default)]
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 前端 `settings_save` 实际发送的形状：replaygain 只有 enabled、缺 targetLufs。
    /// 修复前该载荷会因 `missing field targetLufs` 整体反序列化失败，
    /// 使曲库白名单等所有设置无法落盘（表现为"已添加目录却报未配置曲库目录"）。
    #[test]
    fn deserialize_frontend_payload_without_target_lufs() {
        let json = r#"{
            "locale": "zh-CN",
            "libraryDirs": ["E:\\KwDownload"],
            "online": { "enabled": false, "allowLibraryUpload": false, "provider": "none" },
            "scanOnStartup": true,
            "crossfade": { "enabled": true, "duration_ms": 1000 },
            "replaygain": { "enabled": true },
            "watch": { "enabled": true }
        }"#;

        let cfg: AppConfig = serde_json::from_str(json).expect("前端载荷应可反序列化");
        assert_eq!(cfg.library_dirs, vec!["E:\\KwDownload".to_string()]);
        assert!(cfg.replaygain.enabled);
        assert_eq!(cfg.replaygain.target_lufs, -14.0);
    }

    /// 旧版本配置缺失 replaygain / watch 两个可选段：整体仍可解析并回落默认。
    #[test]
    fn deserialize_legacy_payload_missing_optional_sections() {
        let json = r#"{
            "locale": "zh-CN",
            "libraryDirs": [],
            "online": { "enabled": false, "allowLibraryUpload": false, "provider": "none" },
            "scanOnStartup": true,
            "crossfade": { "enabled": true, "duration_ms": 1000 }
        }"#;

        let cfg: AppConfig = serde_json::from_str(json).expect("旧配置应可解析");
        assert!(cfg.replaygain.enabled);
        assert_eq!(cfg.replaygain.target_lufs, -14.0);
        assert!(cfg.watch.enabled);
    }

    /// 边界：libraryDirs 缺失必须报错。
    /// `settings_save` 是整体替换，若缺字段静默回落为空会清空用户曲库白名单。
    #[test]
    fn deserialize_missing_library_dirs_is_rejected() {
        let json = r#"{
            "locale": "zh-CN",
            "online": { "enabled": false, "allowLibraryUpload": false, "provider": "none" },
            "scanOnStartup": true,
            "crossfade": { "enabled": true, "duration_ms": 1000 }
        }"#;

        assert!(
            serde_json::from_str::<AppConfig>(json).is_err(),
            "缺少 libraryDirs 应被拒绝，避免静默清空白名单"
        );
    }

    /// 落盘 → 读取往返：曲库白名单与嵌套配置保持不变。
    #[test]
    fn save_then_load_roundtrip_preserves_config() {
        let dir = std::env::temp_dir().join(format!("tplayer-cfg-test-{}", std::process::id()));
        let path = dir.join("config.json");

        let mut cfg = AppConfig::default();
        cfg.library_dirs = vec!["E:\\KwDownload".to_string(), "\\\\nas\\music".to_string()];
        cfg.crossfade.enabled = false;
        cfg.crossfade.duration_ms = 500;
        cfg.save(&path).expect("保存应成功");

        let loaded = AppConfig::load(&path);
        assert_eq!(loaded.library_dirs, cfg.library_dirs);
        assert!(!loaded.crossfade.enabled);
        assert_eq!(loaded.crossfade.duration_ms, 500);
        assert_eq!(loaded.replaygain.target_lufs, -14.0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 边界：损坏 / 非法 JSON 回落默认，不 panic。
    #[test]
    fn load_corrupted_file_falls_back_to_default() {
        let dir = std::env::temp_dir().join(format!("tplayer-cfg-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, "{ not valid json").unwrap();

        let loaded = AppConfig::load(&path);
        assert!(loaded.library_dirs.is_empty());
        assert!(loaded.scan_on_startup);
        assert_eq!(loaded.replaygain.target_lufs, -14.0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 边界：文件不存在时回落默认（首次启动）。
    #[test]
    fn load_missing_file_falls_back_to_default() {
        let path = std::env::temp_dir().join(format!("tplayer-cfg-none-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let loaded = AppConfig::load(&path);
        assert!(loaded.library_dirs.is_empty());
        assert!(loaded.watch.enabled);
    }
}
