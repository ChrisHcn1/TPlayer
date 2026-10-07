//! Tauri 构建脚本（M0 骨架）
//!
//! 职责：生成 Tauri 运行时所需的上下文与权限 schema（输出到 src-tauri/gen/，该目录不入库）。
//! 注意：不要在此处加入业务构建逻辑；FFmpeg 等二进制资源的注入策略见 ARCHITECTURE.md §11。

fn main() {
    tauri_build::build()
}
