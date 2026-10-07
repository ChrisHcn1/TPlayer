//! Tauri 命令层（M0 骨架）
//!
//! 分层约定：
//!   - 命令只做「参数校验 + 调用领域层 + 结构转换」，不写业务实现；
//!   - 所有命令返回 `AppResult<T>`，错误经 `AppError` 序列化为前端可读结构；
//!   - 命令名与前端服务方法一一对应（见各文件头部说明），命名与参数用 camelCase 对接口。
//!
//! M0 状态：全部命令返回 `AppError::NotImplemented`，这是刻意行为（骨架不等于实现）。

pub mod library;
pub mod lyrics;
pub mod player;
pub mod settings;
