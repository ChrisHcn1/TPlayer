# FFmpeg 内置说明

本程序支持内置 FFmpeg 工具，无需用户单独安装。

## 放置文件

请将以下文件放置在 `src-tauri/bin/` 目录中：

- `ffmpeg.exe` - FFmpeg 主程序，用于音频转码
- `ffprobe.exe` - FFprobe 工具，用于获取音频信息
- `ffplay.exe` - FFplay 播放器（可选）

## 获取 FFmpeg

您可以从以下地址获取 FFmpeg：

- 官方网站：https://ffmpeg.org/download.html
- Windows 构建版本：https://www.gyan.dev/ffmpeg/builds/
- GitHub Releases：https://github.com/BtbN/FFmpeg-Builds/releases

推荐使用 `ffmpeg-git-full.7z` 或 `ffmpeg-release-full.7z` 版本。

## 使用说明

1. 下载并解压 FFmpeg 压缩包
2. 从 `bin` 目录中提取 `ffmpeg.exe`、`ffprobe.exe` 和 `ffplay.exe`
3. 将这些文件复制到 `src-tauri/bin/` 目录
4. 运行 `npm run tauri build` 进行打包

程序会自动优先使用内置的 FFmpeg 工具，如果不存在则使用系统安装的版本。

## 注意事项

- 请确保下载的 FFmpeg 版本与您的系统架构匹配（x64 或 x86）
- 内置的 FFmpeg 工具会随程序一起打包，无需用户单独安装
- 如果用户已安装 FFmpeg，程序会优先使用用户安装的版本（通过环境变量或 PATH）
