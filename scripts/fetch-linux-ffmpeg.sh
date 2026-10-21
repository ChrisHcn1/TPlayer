#!/usr/bin/env bash
# =============================================================================
# TPlayer —— 获取 Linux 版 FFmpeg 到 bin/linux/
# =============================================================================
#
# 背景：
#   仓库 bin/ 下只有 Windows 版 ffmpeg.exe / ffprobe.exe（随 NSIS 安装包分发），
#   Linux 侧不能使用。无损音频（APE / WavPack / TTA / DSD(DSF/DFF) / DTS / AIFF，
#   以及 ALAC 封装的 .m4a）在 Rust 引擎中经 FFmpeg 子进程解码，因此 Linux 需要
#   一个 ELF 版 ffmpeg（+ ffprobe）。
#
# 默认源：
#   ffmpeg  ← PyPI 上的 imageio-ffmpeg（内含 johnvansickle 完整静态构建，
#             含 dsd_lsbf 解码器与 dsf 解复用器，可解码 DSD / DSF）；
#   ffprobe ← npmmirror 上的 @ffmpeg-installer（x64 含 ffprobe；arm64 无静态包，
#             ffprobe 缺失时引擎自动回退 ffmpeg -i 解析 Duration）。
#
# DSD（DSDIFF / DSF）支持要点：
#   ffmpeg 主线已移除 `dff` 解复用器与 `dsd` 滤镜；现代构建改由 `dsd_lsbf` 解码器
#   在解码阶段直接把 DSD 位流转为 PCM（ffplay/ffmpeg 默认即为此行为）。因此本仓库
#   的 ffmpeg 构建只需具备 `dsf` 解复用器 + `dsd_lsbf` 解码器即可；SACD 导出 DSF
#   即走此路径。引擎侧见 src-tauri/src/player/engine.rs 的 spawn_ffmpeg_decoder
#   （DSD 不再注入 -af dsd，交由解码器转 PCM；DFF 因主线移除 dff 解复用器暂不支持）。
#
# 特性：
#   - 完全静态链接（无 glibc 依赖，各发行版可直接运行）；
#   - 解码器齐全：mp3 / flac / wav / aac / m4a(ALAC) / ogg(vorbis) / opus /
#     ape / wv / tta / dsd_lsbf(DSF) / dca(DTS) / wma / aiff；
#   - 体积：ffmpeg 约 70MB + ffprobe 约 65MB。
#
# 覆盖方式（任一）：
#   FFMPEG_TARBALL=<URL 或本地路径>   自定义 ffmpeg 归档（.tgz / .tar.xz / .whl）
#   FFPROBE_TARBALL=<URL 或本地路径>  自定义 ffprobe 归档
#   或事先把可执行文件放进 bin/linux/（脚本检测到即跳过对应下载）
#
# 依赖：curl / tar / unzip / file；ffmpeg 源需 python3（解析 PyPI 索引）；
#       ffprobe 源（x64）需 node + npm（解析版本号）。
# 用法：
#   ./scripts/fetch-linux-ffmpeg.sh            # 按本机架构下载
#   ARCH=arm64 ./scripts/fetch-linux-ffmpeg.sh # 指定架构
# =============================================================================

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="$ROOT/bin/linux"

ARCH="${ARCH:-$(dpkg --print-architecture 2>/dev/null || uname -m)}"
case "$ARCH" in
  amd64|x86_64)   NPM_ARCH="x64";   WANT_ELF="x86-64";      WHL_ARCH="x64";    BIN_ARCH="x64" ;;
  arm64|aarch64)  NPM_ARCH="arm64"; WANT_ELF="ARM aarch64";  WHL_ARCH="arm64";  BIN_ARCH="aarch64" ;;
  *) echo "错误：不支持的架构 $ARCH（支持 amd64 / arm64）" >&2; exit 1 ;;
esac

need() { command -v "$1" >/dev/null 2>&1 || { echo "错误：缺少必需命令 $1" >&2; exit 1; }; }
need curl
need tar
need unzip
need file

echo "========================================================="
echo " TPlayer · 获取 Linux 版 FFmpeg（$ARCH，完整静态构建）"
echo "   目标目录 : $OUT_DIR"
echo "========================================================="

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

# 把归档（URL 或本地路径，.tgz/.tar.xz/.whl）就地为本地文件
resolve_local() { # $1=URL 或本地路径 → 回显本地路径
  local src="$1" local_path
  case "$src" in
    http://*|https://*)
      local_path="$TMP_DIR/$(basename "${src%%\?*}")"
      curl -fsSL --retry 3 --retry-delay 3 -o "$local_path" "$src"
      ;;
    *) local_path="$src" ;;
  esac
  [ -f "$local_path" ] || { echo "错误：归档不存在：$src" >&2; exit 1; }
  printf '%s\n' "$local_path"
}

# 从 .tgz/.tar.xz 归档中解出二进制
extract_binary() { # $1=URL 或本地路径 $2=二进制名 $3=目标文件 [$4=归档内精确路径]
  local src="$1" name="$2" dest="$3" exact="${4:-}" local_path inner
  local_path="$(resolve_local "$src")"
  case "$local_path" in
    *.tgz|*.tar.gz)
      if [ -n "$exact" ]; then inner="$exact"; else inner="$(tar -tzf "$local_path" | grep -E "(^|/)${name}\$" | head -1)"; fi
      [ -n "$inner" ] || { echo "错误：$local_path 中未找到 $name" >&2; exit 1; }
      tar -xzf "$local_path" "$inner" -O > "$dest"
      ;;
    *.tar.xz|*.txz)
      if [ -n "$exact" ]; then inner="$exact"; else inner="$(tar -tJf "$local_path" | grep -E "(^|/)${name}\$" | head -1)"; fi
      [ -n "$inner" ] || { echo "错误：$local_path 中未找到 $name" >&2; exit 1; }
      tar -xJf "$local_path" "$inner" -O > "$dest"
      ;;
    *) echo "错误：不支持的归档格式：$local_path（支持 .tgz / .tar.xz）" >&2; exit 1 ;;
  esac
  [ -s "$dest" ] || { echo "错误：解压 $name 失败" >&2; exit 1; }
  chmod 0755 "$dest"
}

# 从 imageio-ffmpeg 的 PyPI wheel（zip）解出 ffmpeg-linux-<BIN_ARCH> 二进制
extract_ffmpeg_from_wheel() { # $1=wheel 本地路径 $2=目标文件
  local whl="$1" dest="$2" inner
  inner="$(unzip -l "$whl" | awk '/imageio_ffmpeg\/binaries\/ffmpeg-linux-'"$BIN_ARCH"'-/{print $4; exit}')"
  [ -n "$inner" ] || { echo "错误：$whl 中未找到 ffmpeg-linux-$BIN_ARCH 二进制" >&2; exit 1; }
  unzip -p "$whl" "$inner" > "$dest"
  [ -s "$dest" ] || { echo "错误：解压 ffmpeg 失败" >&2; exit 1; }
  chmod 0755 "$dest"
}

# 从 PyPI 选取对应架构最新 manylinux wheel 的下载地址
imageio_ffmpeg_whl_url() {
  curl -fsSL --retry 3 "https://pypi.org/simple/imageio-ffmpeg/" \
    | python3 -c "
import sys,re
txt=sys.stdin.read()
urls=re.findall(r'https://files.pythonhosted.org/packages/[^\"]+\.whl', txt)
def ver(u):
    m=re.search(r'imageio_ffmpeg-([0-9.]+)-',u); return m.group(1) if m else '0'
arch='$WHL_ARCH'
cands=[u for u in urls if 'manylinux' in u and ('x86_64' in u if arch=='x64' else 'aarch64' in u)]
print(sorted(cands, key=ver, reverse=True)[0])
"
}

# 经 npmmirror npm pack 取包，返回本地 .tgz 路径
npm_pack() { # $1=包名 $2=tarball 文件名前缀
  need node; need npm
  ( cd "$TMP_DIR" && npm pack "$1" --registry https://registry.npmmirror.com >/dev/null 2>&1 )
  ls "$TMP_DIR"/$2*.tgz | head -1
}

mkdir -p "$OUT_DIR"

# ---- 1. ffmpeg（完整构建，含 dsd_lsbf 解码器 + dsf 解复用器）----------------
# 已存在且具备 dsf 解复用器 + dsd_lsbf 解码器才跳过；否则重新获取完整构建。
if [ -f "$OUT_DIR/ffmpeg" ] && \
   "$OUT_DIR/ffmpeg" -hide_banner -demuxers 2>/dev/null | grep -qw dsf && \
   "$OUT_DIR/ffmpeg" -hide_banner -decoders 2>/dev/null | grep -qw dsd_lsbf; then
  echo "[1/3] ffmpeg 已存在且为完整构建（含 dsf/dsd_lsbf），跳过（$OUT_DIR/ffmpeg）"
elif [ -n "${FFMPEG_TARBALL:-}" ]; then
  echo "[1/3] 安装自定义 ffmpeg 归档"
  extract_binary "$FFMPEG_TARBALL" "ffmpeg" "$OUT_DIR/ffmpeg"
elif [ -f "$OUT_DIR/ffmpeg" ]; then
  echo "[1/3] 现有 ffmpeg 为精简构建（缺 dsf/dsd_lsbf），重新获取完整构建…"
  need python3
  whl="$(resolve_local "$(imageio_ffmpeg_whl_url)")"
  extract_ffmpeg_from_wheel "$whl" "$OUT_DIR/ffmpeg"
else
  echo "[1/3] 经 PyPI(imageio-ffmpeg) 获取完整静态 ffmpeg …"
  need python3
  whl="$(resolve_local "$(imageio_ffmpeg_whl_url)")"
  extract_ffmpeg_from_wheel "$whl" "$OUT_DIR/ffmpeg"
fi

# ---- 2. ffprobe --------------------------------------------------------------
if [ -f "$OUT_DIR/ffprobe" ]; then
  echo "[2/3] ffprobe 已存在，跳过（$OUT_DIR/ffprobe）"
elif [ -n "${FFPROBE_TARBALL:-}" ]; then
  echo "[2/3] 安装自定义 ffprobe 归档"
  extract_binary "$FFPROBE_TARBALL" "ffprobe" "$OUT_DIR/ffprobe"
elif [ "$NPM_ARCH" = "x64" ]; then
  echo "[2/3] 经 npmmirror 获取 ffprobe-static …"
  extract_binary "$(npm_pack "ffprobe-static" "ffprobe-static-")" "ffprobe" "$OUT_DIR/ffprobe" "package/bin/linux/$NPM_ARCH/ffprobe"
else
  echo "[2/3] arm64 无 ffprobe 静态包，跳过（引擎回退 ffmpeg -i 探测时长）"
fi

# ---- 3. 校验 -----------------------------------------------------------------
echo "[3/3] 校验二进制（ELF / 架构 / 可执行 / DFF 能力）…"
[ -f "$OUT_DIR/ffmpeg" ] || { echo "错误：缺少 $OUT_DIR/ffmpeg" >&2; exit 1; }
for tool in ffmpeg ffprobe; do
  [ -f "$OUT_DIR/$tool" ] || continue
  desc="$(file -b "$OUT_DIR/$tool")"
  case "$desc" in
    *ELF*) ;;
    *) echo "错误：$OUT_DIR/$tool 不是 ELF 可执行文件：$desc" >&2; exit 1 ;;
  esac
  case "$desc" in
    *"$WANT_ELF"*) ;;
    *) echo "错误：$OUT_DIR/$tool 架构不符（$desc，期望 $ARCH）" >&2; exit 1 ;;
  esac
  "$OUT_DIR/$tool" -hide_banner -version >/dev/null 2>&1 || {
    echo "错误：$OUT_DIR/$tool 无法执行" >&2; exit 1; }
  echo "       $tool: $desc"
done
[ -f "$OUT_DIR/ffprobe" ] || echo "       ffprobe: 未安装（时长探测回退 ffmpeg -i）"

# DSD 关键能力强校验：dsf 解复用器（SACD 导出 DSF 走此路径）+ dsd_lsbf 解码器
# （现代 ffmpeg 已移除 `dff` 解复用器与 `dsd` 滤镜，由解码器直接输出 PCM）。
"$OUT_DIR/ffmpeg" -hide_banner -demuxers 2>/dev/null | grep -qw dsf \
  || { echo "错误：ffmpeg 缺少 dsf 解复用器，SACD/DSF 无法播放（请使用完整构建）" >&2; exit 1; }
"$OUT_DIR/ffmpeg" -hide_banner -decoders 2>/dev/null | grep -qw dsd_lsbf \
  || { echo "错误：ffmpeg 缺少 dsd_lsbf 解码器，DSD 无法转 PCM（请使用完整构建）" >&2; exit 1; }
echo "       ✓ dsf 解复用器 / dsd_lsbf 解码器 就绪（SACD/DSF 可播放；DFF 因主线移除 dff 解复用器暂不支持）"

DECS="$("$OUT_DIR/ffmpeg" -hide_banner -decoders 2>/dev/null || true)"
missing=0
for codec in ape wavpack tta dsd_lsbf dca aiff alac opus flac; do
  if printf '%s\n' "$DECS" | grep -q " $codec "; then
    echo "       ✓ $codec 解码器"
  else
    echo "       ! 未内置 $codec 解码器（对应格式将无法解码）" >&2
    missing=1
  fi
done
[ "$missing" -eq 0 ] || echo "       提示：部分解码器缺失，播放对应格式会失败（见上）"

echo ""
echo "完成：$OUT_DIR/ffmpeg$([ -f "$OUT_DIR/ffprobe" ] && echo " + $OUT_DIR/ffprobe")"
echo "  - 开发运行：直接启动即可（引擎自动沿祖先目录查找 bin/linux）"
echo "  - DEB 打包：scripts/build-deb.sh 会随包分发该目录（BUNDLE_FFMPEG=1）"
