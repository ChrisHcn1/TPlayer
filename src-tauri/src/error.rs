//! 错误类型与统一返回契约（M0 骨架）
//!
//! 契约（与前端 src/types/api.ts 的 AppError / AppResult 一致）：
//!   Rust: `Result<T, AppError>`  ──序列化──▶  { code, message, detail }
//!   前端: services/ipc.ts 的 `callCommandSafe` 把失败收敛为 `{ ok: false, error }`。
//! 约定：新增错误一律在此枚举中登记，禁止用 `String` 或 `anyhow` 穿透到命令层。

use serde::ser::{SerializeStruct, Serializer};
use serde::Serialize;
use thiserror::Error;

/// 后端统一错误类型
#[derive(Debug, Error)]
pub enum AppError {
    /// M0 骨架：功能尚未实现（刻意显式失败，避免返回假数据）
    #[error("功能尚未实现：{0}")]
    NotImplemented(&'static str),

    /// 路径不在音乐库白名单内（安全边界，见 security.rs）
    #[error("路径不在音乐库白名单内：{0}")]
    PathNotAllowed(String),

    #[error("播放器错误：{0}")]
    Player(String),

    #[error("曲库错误：{0}")]
    Library(String),

    #[error("歌词错误：{0}")]
    Lyrics(String),

    #[error("更新检查失败：{0}")]
    Update(String),

    #[error("在线模块未开启，已拒绝该请求：{0}")]
    OnlineDisabled(String),

    #[error("配置错误：{0}")]
    Config(String),

    #[error("I/O 错误：{0}")]
    Io(#[from] std::io::Error),
}

impl AppError {
    /// 便捷构造：M0 骨架中的统一未实现错误
    pub fn not_implemented(what: &'static str) -> Self {
        Self::NotImplemented(what)
    }

    /// 稳定的错误码，供前端按码分支处理（不要依赖 message 文案）
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotImplemented(_) => "NOT_IMPLEMENTED",
            Self::PathNotAllowed(_) => "PATH_NOT_ALLOWED",
            Self::Player(_) => "PLAYER_ERROR",
            Self::Library(_) => "LIBRARY_ERROR",
            Self::Lyrics(_) => "LYRICS_ERROR",
            Self::Update(_) => "UPDATE_ERROR",
            Self::OnlineDisabled(_) => "ONLINE_DISABLED",
            Self::Config(_) => "CONFIG_ERROR",
            Self::Io(_) => "IO_ERROR",
        }
    }

    /// 附加细节（可为空）；不含敏感信息，仅用于诊断
    pub fn detail(&self) -> Option<String> {
        match self {
            Self::Io(error) => Some(error.to_string()),
            _ => None,
        }
    }
}

/// 按 { code, message, detail } 序列化，直接对应前端 AppError
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("AppError", 3)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", &self.to_string())?;
        state.serialize_field("detail", &self.detail())?;
        state.end()
    }
}

/// 后端命令统一返回类型
pub type AppResult<T> = Result<T, AppError>;
