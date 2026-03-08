# TPlayer
TPlayer 是一款基于 Tauri + Vue 3 开发的现代化音乐播放器，支持多种音频格式播放，提供优质的音频体验和简洁的用户界面。
TPlayer


主要功能
支持多种音频格式：MP3、FLAC、WAV、AAC、OGG、M4A、APE、DSD、DTS 等
智能音频转码：自动转码不支持的音频格式为 FLAC
均衡器控制：提供多种预设和自定义均衡器
播放列表管理：创建、编辑、删除歌单
播放模式：顺序播放、随机播放、循环播放
播放进度记忆：自动保存播放位置
可视化效果：音频频谱可视化
主题切换：支持亮色和暗色主题
技术栈
Tauri 2.x - 跨平台桌面应用框架
Vue 3 - 现代化前端框架
Rust - 高性能后端
FFmpeg - 音频解码和转码
rodio - Rust 音频播放库
开发者信息
开发者：TPlayer 开发团队

感谢 格力森酒业 智睿舒畅 忧蓝对项目的贡献

许可证：ISC

项目地址：https://github.com/ChrisHcn1/TPlayer

使用说明
1. 点击"扫描目录"按钮选择音乐文件夹

2. 在播放列表中选择要播放的歌曲

3. 使用底部播放控制栏控制播放

4. 点击设置按钮调整音频和界面选项

5. 使用均衡器调整音效

FFmpeg 配置
下载地址：https://ffmpeg.org/download.html

安装方法：

Windows：下载压缩包后解压到任意目录
macOS：使用 Homebrew 安装：brew install ffmpeg
Linux：使用包管理器安装，如 apt install ffmpeg
环境变量配置：

Windows：将FFmpeg的bin目录添加到系统环境变量PATH中
macOS/Linux：确保FFmpeg在系统PATH中，或在设置中指定FFmpeg可执行文件路径
感谢您使用 TPlayer！

© 2026 TPlayer. All rights reserved.
