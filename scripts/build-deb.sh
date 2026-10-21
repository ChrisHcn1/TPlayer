#!/usr/bin/env bash
# =============================================================================
# TPlayer —— Ubuntu / Debian 原生 DEB 打包脚本
# =============================================================================
#
# 目标平台：Ubuntu 26.10（及任何基于 Debian 的发行版，amd64 / arm64）
#
# 打包方式说明（"严格使用原生打包方式"）：
#   本脚本只使用发行版原生工具链完成打包，不引入任何第三方打包技术
#   （不使用 electron-builder / appimagetool / Tauri 自带 bundler 等）：
#     1. 前端产物      —— npm run build（Vite）
#     2. 原生二进制    —— cargo build --release（rustc）
#     3. 安装包        —— dpkg-deb --build --root-owner-group（dpkg 原生工具）
#     4. 元数据        —— 手写 Debian Policy 控制文件 / maintainer scripts /
#                         desktop entry / AppStream metainfo / MIME / man page
#   安装与卸载同样只依赖 dpkg / apt。
#
# 可参数化项（环境变量）：
#   ARCH=amd64|arm64     目标 CPU 架构（默认取本机架构；决定 dpkg Architecture
#                        与 Rust target：x86_64-unknown-linux-gnu /
#                        aarch64-unknown-linux-gnu）
#   PREFIX=/usr          安装路径前缀（/usr | /usr/local | /opt/tplayer …）
#   VERSION=2.0.0        版本号（默认读 package.json）
#   PROFILE=release      cargo profile
#   RUST_TARGET=...      显式指定 Rust target（覆盖 ARCH 推导，用于交叉编译）
#   CARGO_WRAPPER=cross  交叉编译时使用的 cargo 封装（如 cross）
#   OUTPUT_DIR=dist/linux 产物输出目录
#   SKIP_FRONTEND=1      跳过前端构建（已构建过）
#   SKIP_BUILD=1         跳过 cargo 构建（复用已有二进制）
#   BUNDLE_FFMPEG=1|0    是否把 ffmpeg/ffprobe 打进安装包
#                        （FFMPEG_DIR 或 bin/linux 提供 Linux 版 ffmpeg/ffprobe
#                         时默认开启；关闭时改由系统 ffmpeg 提供，Depends 自动加 ffmpeg）
#   FFMPEG_DIR=...       存放 Linux ffmpeg / ffprobe 可执行文件的目录
#                        （默认取仓库 bin/linux，可用 scripts/fetch-linux-ffmpeg.sh 获取）
#   PACKAGE_NAME=tplayer        DEB 包名（Linux 侧统一为 TPlayer，移除 Next 字样）
#   BIN_NAME=tplayer            安装后的命令名（/usr/bin/<BIN_NAME>）
#   CARGO_BIN_NAME=tplayer-next cargo 产出的二进制名（Cargo 包名，与 Windows 构建
#                              共用同一 Cargo.toml，不随 Linux 侧改名而改动；
#                              脚本负责把该产物安装为 <BIN_NAME>）
#   MAINTAINER="..."     维护者字段
#   HOMEPAGE=...         主页字段
#   SECTION=sound        Debian section
#   WM_CLASS=...         桌面文件 StartupWMClass（需与窗口实际 WM_CLASS 一致）
#   RUN_LINTIAN=1|0     是否运行 lintian（默认可用则运行，结果仅告警不阻断）
#
# 用法示例：
#   # 纯净 Ubuntu 26.10 amd64，随包分发 ffmpeg（推荐：安装即可播全部格式）
#   FFMPEG_DIR=/opt/ffmpeg-linux-x64 ARCH=amd64 ./scripts/build-deb.sh
#
#   # arm64，使用系统 ffmpeg
#   ARCH=arm64 BUNDLE_FFMPEG=0 ./scripts/build-deb.sh
#
#   # 交叉编译 arm64（在 amd64 主机上，需先配置交叉工具链或使用 cross）
#   ARCH=arm64 CARGO_WRAPPER=cross ./scripts/build-deb.sh
#
# 依赖（构建机，Ubuntu）：
#   dpkg-deb file gzip find sed coreutils
#   nodejs npm rustc cargo
#   （Tauri Linux 构建还需：libgtk-3-dev libwebkit2gtk-4.1-dev \
#      libayatana-appindicator3-dev librsvg2-dev patchelf）
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------- 基础工具检查
need() { command -v "$1" >/dev/null 2>&1 || { echo "错误：缺少必需命令 $1" >&2; exit 1; }; }
for tool in dpkg-deb file gzip find sed du awk; do need "$tool"; done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PKG_TEMPLATE_DIR="$ROOT/packaging/deb"
[ -d "$PKG_TEMPLATE_DIR" ] || { echo "错误：缺少打包模板目录 $PKG_TEMPLATE_DIR" >&2; exit 1; }

# ---------------------------------------------------------------- 参数解析
ARCH="${ARCH:-$(dpkg --print-architecture 2>/dev/null || echo amd64)}"
case "$ARCH" in
  amd64|x86_64)   ARCH="amd64";        DEFAULT_TARGET="x86_64-unknown-linux-gnu" ;;
  arm64|aarch64)  ARCH="arm64";        DEFAULT_TARGET="aarch64-unknown-linux-gnu" ;;
  *) echo "错误：不支持的架构 $ARCH（支持 amd64 / arm64）" >&2; exit 1 ;;
esac

PREFIX="${PREFIX:-/usr}"                  # 先取默认，避免 set -u 下未设置时报未绑定
PREFIX="${PREFIX%/}"                      # 去掉末尾斜杠
case "$PREFIX" in
  /*) ;;
  *) echo "错误：PREFIX 必须是绝对路径（当前：$PREFIX）" >&2; exit 1 ;;
esac

VERSION="${VERSION:-}"
if [ -z "$VERSION" ]; then
  need node
  VERSION="$(node -p "require('./package.json').version")"
fi
# Debian 版本号只允许小写字母、数字与 . + - ~
DEB_VERSION="$(printf '%s' "$VERSION" | tr '[:upper:]' '[:lower:]')"

# 版本同步校验（发版元数据一致性）：package.json / tauri.conf.json / Cargo.toml
# 三处版本号必须一致，否则安装包版本与自动升级声明会脱节。校验失败即中止，
# 避免带着不一致的版本元数据发版。（SKIP_VERSION_CHECK=1 可跳过）
if [ -z "${SKIP_VERSION_CHECK:-}" ]; then
  V_CONF=""
  if command -v node >/dev/null 2>&1; then
    V_CONF="$(node -p "const c=require('./src-tauri/tauri.conf.json'); c.version" 2>/dev/null || true)"
  fi
  V_CARGO="$(sed -n 's/^version = "\(.*\)"/\1/p' src-tauri/Cargo.toml | head -1)"
  if [ -n "$V_CONF" ] && [ "$V_CONF" != "$VERSION" ]; then
    echo "错误：版本不一致——package.json=$VERSION，tauri.conf.json=$V_CONF" >&2
    exit 1
  fi
  if [ -n "$V_CARGO" ] && [ "$V_CARGO" != "$VERSION" ]; then
    echo "错误：版本不一致——package.json=$VERSION，Cargo.toml=$V_CARGO" >&2
    exit 1
  fi
  echo "       版本同步校验通过：$VERSION（package.json / tauri.conf.json / Cargo.toml）"
fi

PACKAGE_NAME="${PACKAGE_NAME:-tplayer}"
BIN_NAME="${BIN_NAME:-tplayer}"
# Cargo 包名产物名：与 Windows 构建共用同一 Cargo.toml（包名 tplayer-next），
# Linux 侧不改动共享配置，仅在安装时把该产物落为 <BIN_NAME>（tplayer）。
CARGO_BIN_NAME="${CARGO_BIN_NAME:-tplayer-next}"
MAINTAINER="${MAINTAINER:-TPlayer Contributors <noreply@tplayer.local>}"
HOMEPAGE="${HOMEPAGE:-https://github.com/ChrisHcn1/TPlayer}"
SECTION="${SECTION:-sound}"
# 桌面文件 StartupWMClass 默认取 Tauri 标识符（Linux 下 GTK 以标识符作为 app id /
# WM_CLASS 的依据）；模板中另附 TPlayer / tplayer-next 候选，命中任一即可关联。
WM_CLASS="${WM_CLASS:-com.tplayer}"
PROFILE="${PROFILE:-release}"
RUST_TARGET="${RUST_TARGET:-$DEFAULT_TARGET}"
CARGO_WRAPPER="${CARGO_WRAPPER:-}"
OUTPUT_DIR="${OUTPUT_DIR:-$ROOT/dist/linux}"
SKIP_FRONTEND="${SKIP_FRONTEND:-0}"
SKIP_BUILD="${SKIP_BUILD:-0}"
RUN_LINTIAN="${RUN_LINTIAN:-auto}"
RELEASE_DATE="${RELEASE_DATE:-$(date +%Y-%m-%d)}"

# 旧包名（Linux 侧历史包名 tplayer-next）：新包 tplayer 通过
# Conflicts/Replaces/Provides 接管，保证曾装过旧包的用户平滑升级。
LEGACY_PACKAGE=""
[ "$PACKAGE_NAME" != "tplayer-next" ] && LEGACY_PACKAGE="tplayer-next"

FFMPEG_DIR="${FFMPEG_DIR:-}"
# Linux 版 ffmpeg 随仓库分发位：bin/linux/（ffmpeg + ffprobe 两个 ELF 二进制，
# 获取方式见 scripts/fetch-linux-ffmpeg.sh）。目录存在即默认随包分发——
# 安装后无需系统 ffmpeg 即可解码 APE/DSD/WavPack/TTA/TAK/AIFF/DTS/ALAC 等
# 无损格式（Rust 引擎经 /usr/lib/tplayer/bin 与祖先目录查找该目录）。
if [ -z "$FFMPEG_DIR" ] && [ -d "$ROOT/bin/linux" ]; then
  FFMPEG_DIR="$ROOT/bin/linux"
fi
if [ -z "${BUNDLE_FFMPEG:-}" ]; then
  if [ -n "$FFMPEG_DIR" ]; then BUNDLE_FFMPEG=1; else BUNDLE_FFMPEG=0; fi
fi

# 安装后的内部资源目录（Rust 侧 resolve_bin_tool 会探测该路径）
LIB_DIR="$PREFIX/lib/tplayer"
BIN_LIB_DIR="$LIB_DIR/bin"

echo "========================================================="
echo " TPlayer · Debian 原生打包"
echo "   架构        : $ARCH  (rust target: $RUST_TARGET)"
echo "   安装前缀    : $PREFIX"
echo "   版本        : $DEB_VERSION"
echo "   内置 ffmpeg : $BUNDLE_FFMPEG${FFMPEG_DIR:+ ($FFMPEG_DIR)}"
echo "   输出目录    : $OUTPUT_DIR"
echo "========================================================="

if [ "$PREFIX" != "/usr" ]; then
  echo "注意：PREFIX=$PREFIX 非 /usr。桌面入口 / 图标 / MIME 会装到该前缀下，"
  echo "      而多数桌面环境只扫描 /usr/share——需自行设置 XDG_DATA_DIRS"
  echo "      才能让菜单项与文件关联生效。推荐使用默认 /usr。"
fi

# sed 替换值转义（避免值中的 / & \ 与分隔符破坏表达式）
sed_escape() { printf '%s' "$1" | sed -e 's/[\/&\\]/\\&/g'; }

# 渲染模板：@XXX@ 占位符 → 实际值。
# 使用 \001 作分隔符，匹配模板中的 @占位符@ 形式，避免 Depends 等字段里的 "|" 冲突。
render() { # $1=模板文件 $2=输出文件
  local tpl="$1" out="$2"
  local d=$'\001'
  local args=()
  _sub() { args+=(-e "s${d}@$1@${d}$2${d}g"); }
  _sub PACKAGE       "$(sed_escape "$PACKAGE_NAME")"
  _sub VERSION       "$(sed_escape "$DEB_VERSION")"
  _sub ARCHITECTURE  "$(sed_escape "$ARCH")"
  _sub DEPENDS       "$(sed_escape "$DEPENDS")"
  _sub RECOMMENDS    "$(sed_escape "$RECOMMENDS")"
  _sub MAINTAINER    "$(sed_escape "$MAINTAINER")"
  _sub HOMEPAGE      "$(sed_escape "$HOMEPAGE")"
  _sub SECTION       "$(sed_escape "$SECTION")"
  _sub PREFIX        "$(sed_escape "$PREFIX")"
  _sub BIN_NAME      "$(sed_escape "$BIN_NAME")"
  _sub WM_CLASS      "$(sed_escape "$WM_CLASS")"
  _sub RELEASE_DATE  "$(sed_escape "$RELEASE_DATE")"
  _sub LIB_DIR       "$(sed_escape "$LIB_DIR")"
  _sub LEGACY_PACKAGE "$(sed_escape "$LEGACY_PACKAGE")"
  sed "${args[@]}" "$tpl" > "$out"
}

# ---------------------------------------------------------------- 1. 前端构建
if [ "$SKIP_FRONTEND" != "1" ]; then
  need npm
  echo "[1/6] 构建前端产物 (npm run build)…"
  # npm ci 需要 lock 与 package.json 完全同步；不同步时回退 npm install
  npm ci --no-audit --no-fund || npm install --no-audit --no-fund
  npm run build
else
  echo "[1/6] 跳过前端构建 (SKIP_FRONTEND=1)"
  [ -d "$ROOT/dist" ] || { echo "错误：dist/ 不存在，无法打包" >&2; exit 1; }
fi

# ---------------------------------------------------------------- 2. 原生二进制
if [ "$SKIP_BUILD" != "1" ]; then
  need cargo
  echo "[2/6] 编译原生二进制 (cargo build --profile $PROFILE)…"
  cargo_bin=cargo
  if [ -n "$CARGO_WRAPPER" ]; then
    cargo_bin="$CARGO_WRAPPER cargo"
  fi
  # 关键：本脚本绕开 `tauri build`（改用原生 dpkg-deb），但必须显式开启
  # tauri 的 `custom-protocol` 特性，否则 release 二进制不会内嵌 frontendDist
  # （../dist），运行时会回退到 devUrl（http://localhost:3100 的 Vite 开发服务器）
  # 导致 “Could not connect to localhost: Connection refused”。开启后前端资源
  # 在编译期由 tauri-build 直接打进二进制，无需另行拷贝 dist/ 进包。
  TAURI_FEATURES="${TAURI_FEATURES:-tauri/custom-protocol}"
  # shellcheck disable=SC2086
  $cargo_bin build --profile "$PROFILE" --target "$RUST_TARGET" \
    --features "$TAURI_FEATURES" \
    --manifest-path "$ROOT/src-tauri/Cargo.toml"
else
  echo "[2/6] 跳过编译 (SKIP_BUILD=1)"
fi

# 定位二进制：按 Cargo 产物名（CARGO_BIN_NAME）查找，优先显式 target，其次宿主 target；
# 兼容 <BIN_NAME> 直命名的历史布局（旧脚本默认二者同名）
BINARY=""
for name in "$CARGO_BIN_NAME" "$BIN_NAME"; do
  for cand in \
    "$ROOT/src-tauri/target/$RUST_TARGET/$PROFILE/$name" \
    "$ROOT/src-tauri/target/$RUST_TARGET/release/$name" \
    "$ROOT/src-tauri/target/$PROFILE/$name" \
    "$ROOT/src-tauri/target/release/$name"
  do
    if [ -f "$cand" ]; then BINARY="$cand"; break 2; fi
  done
done
[ -n "$BINARY" ] || { echo "错误：未找到二进制 $CARGO_BIN_NAME（请先构建）" >&2; exit 1; }

# ---------------------------------------------------------------- 3. 二进制校验
echo "[3/6] 校验二进制（ELF / 架构匹配）…"
BIN_DESC="$(file -b "$BINARY")"
case "$BIN_DESC" in
  *ELF*) ;;
  *) echo "错误：$BINARY 不是 ELF 可执行文件：$BIN_DESC" >&2; exit 1 ;;
esac
case "$ARCH" in
  amd64) WANT_ELF="x86-64" ;;
  arm64) WANT_ELF="ARM aarch64" ;;
esac
case "$BIN_DESC" in
  *"$WANT_ELF"*) ;;
  *) echo "错误：二进制架构与目标不符（$BIN_DESC，期望 $ARCH/$WANT_ELF）" >&2
     echo "      交叉编译请配置对应交叉工具链，或使用 CARGO_WRAPPER=cross" >&2
     exit 1 ;;
esac
echo "       $BINARY"
echo "       $BIN_DESC"

# ---------------------------------------------------------------- 4. 组装包树
echo "[4/6] 组装安装包目录树…"
STAGE="$ROOT/build/deb-stage"
PKG_ROOT="$STAGE/$PACKAGE_NAME"
MAN_TMP_DIR="$STAGE/man-tmp"
rm -rf "$STAGE"
mkdir -p "$PKG_ROOT" "$MAN_TMP_DIR"

install -D -m 0755 "$BINARY" "$PKG_ROOT$PREFIX/bin/$BIN_NAME"

# ---- 图标（hicolor）：优先精确尺寸源，缺省尺寸用 ImageMagick 缩放生成
ICON_SRC_DIR="$ROOT/src-tauri/icons"
HICOLOR="$PKG_ROOT$PREFIX/share/icons/hicolor"
mkdir -p "$HICOLOR"
icon_count=0
install_icon_by_source() { # $1=尺寸 $2=源文件
  local size="$1" src="$2"
  [ -f "$src" ] || return 0
  install -D -m 0644 "$src" "$HICOLOR/$size/apps/tplayer.png"
  icon_count=$((icon_count + 1))
}
generate_icon() { # $1=尺寸：由最大源图缩放
  local size="$1" w h out
  w="${size%x*}"; h="${size#*x}"
  out="$HICOLOR/$size/apps/tplayer.png"
  if [ -f "$out" ]; then return 0; fi
  if command -v magick >/dev/null 2>&1; then
    if magick "$ICON_SRC_DIR/icon.png" -resize "${w}x${h}" "$out" 2>/dev/null; then
      icon_count=$((icon_count + 1))
    fi
  elif command -v convert >/dev/null 2>&1; then
    if convert "$ICON_SRC_DIR/icon.png" -resize "${w}x${h}" "$out" 2>/dev/null; then
      icon_count=$((icon_count + 1))
    fi
  fi
}
if [ -d "$ICON_SRC_DIR" ]; then
  install_icon_by_source "32x32"   "$ICON_SRC_DIR/32x32.png"
  install_icon_by_source "48x48"   "$ICON_SRC_DIR/48x48.png"
  install_icon_by_source "64x64"   "$ICON_SRC_DIR/64x64.png"
  install_icon_by_source "128x128" "$ICON_SRC_DIR/128x128.png"
  install_icon_by_source "256x256" "$ICON_SRC_DIR/128x128@2x.png"
  install_icon_by_source "512x512" "$ICON_SRC_DIR/icon.png"
  for size in 16x16 24x24 48x48 96x96 256x256 512x512; do
    generate_icon "$size"
  done
fi
[ "$icon_count" -gt 0 ] || { echo "错误：未能安装任何图标（$ICON_SRC_DIR）" >&2; exit 1; }
echo "       已安装 $icon_count 个尺寸的 hicolor 图标"

# ---- 桌面入口 / AppStream / MIME / man / doc
# 关键：含 @占位符@ 的模板必须先 render 再安装。此前直接 install 原始模板，导致
# .desktop 里 `Exec=@BIN_NAME@` / `StartupWMClass=@WM_CLASS@` 原样落地——任务栏无法
# 按 WM_CLASS 把窗口关联到本 .desktop，图标回退系统默认（托盘图标独立设置故不受影响）；
# metainfo / copyright / man 页同样带占位符，一并渲染。
RENDER_TMP="$(mktemp -d)"
render "$PKG_TEMPLATE_DIR/com.tplayer.desktop" "$RENDER_TMP/com.tplayer.desktop"
render "$PKG_TEMPLATE_DIR/com.tplayer.metainfo.xml" "$RENDER_TMP/com.tplayer.metainfo.xml"
render "$PKG_TEMPLATE_DIR/copyright" "$RENDER_TMP/copyright"
render "$PKG_TEMPLATE_DIR/tplayer.1" "$RENDER_TMP/tplayer.1"
install -D -m 0644 "$RENDER_TMP/com.tplayer.desktop" \
  "$PKG_ROOT$PREFIX/share/applications/com.tplayer.desktop"
install -D -m 0644 "$RENDER_TMP/com.tplayer.metainfo.xml" \
  "$PKG_ROOT$PREFIX/share/metainfo/com.tplayer.metainfo.xml"
install -D -m 0644 "$PKG_TEMPLATE_DIR/tplayer.xml" \
  "$PKG_ROOT$PREFIX/share/mime/packages/tplayer.xml"
install -D -m 0644 "$RENDER_TMP/tplayer.1" "$MAN_TMP_DIR/$BIN_NAME.1"
gzip -9 -n "$MAN_TMP_DIR/$BIN_NAME.1"
install -D -m 0644 "$MAN_TMP_DIR/$BIN_NAME.1.gz" \
  "$PKG_ROOT$PREFIX/share/man/man1/$BIN_NAME.1.gz"
rm -rf "$MAN_TMP_DIR"
install -D -m 0644 "$RENDER_TMP/copyright" \
  "$PKG_ROOT$PREFIX/share/doc/$PACKAGE_NAME/copyright"
rm -rf "$RENDER_TMP"

mkdir -p "$PKG_ROOT$PREFIX/share/doc/$PACKAGE_NAME"
{
  echo "$PACKAGE_NAME ($DEB_VERSION) unstable; urgency=medium"
  echo ""
  echo "  * 原生 DEB 打包（dpkg-deb）；支持 amd64/arm64，安装前缀可参数化。"
  echo ""
  echo " -- $MAINTAINER  $RELEASE_DATE"
} | gzip -9 -n > "$PKG_ROOT$PREFIX/share/doc/$PACKAGE_NAME/changelog.Debian.gz"

# ---- 可选：随包分发 ffmpeg / ffprobe
if [ "$BUNDLE_FFMPEG" = "1" ]; then
  if [ -z "$FFMPEG_DIR" ] || [ ! -d "$FFMPEG_DIR" ]; then
    echo "错误：BUNDLE_FFMPEG=1 需要 FFMPEG_DIR 指向含 Linux ffmpeg/ffprobe 的目录" >&2
    exit 1
  fi
  for tool in ffmpeg ffprobe; do
    if [ ! -f "$FFMPEG_DIR/$tool" ]; then
      echo "错误：$FFMPEG_DIR/$tool 不存在" >&2
      exit 1
    fi
    ff_desc="$(file -b "$FFMPEG_DIR/$tool")"
    case "$ff_desc" in
      *ELF*) ;;
      *) echo "错误：$FFMPEG_DIR/$tool 不是 Linux ELF 可执行文件：$ff_desc" >&2
         echo "      bin/ 目录下的 ffmpeg.exe 是 Windows 程序，不能用于 Linux 包" >&2
         exit 1 ;;
    esac
    case "$ff_desc" in
      *"$WANT_ELF"*) ;;
      *) echo "错误：$FFMPEG_DIR/$tool 架构与目标不符（$ff_desc，期望 $ARCH）" >&2; exit 1 ;;
    esac
    install -D -m 0755 "$FFMPEG_DIR/$tool" "$PKG_ROOT$BIN_LIB_DIR/$tool"
  done
  echo "       已内置 ffmpeg/ffprobe → $BIN_LIB_DIR"
  RECOMMENDS_FFMPEG=""
else
  echo "       未内置 ffmpeg：依赖系统 ffmpeg（Recommends）"
  RECOMMENDS_FFMPEG="ffmpeg | ffmpeg-free"
fi

# ---- 运行时依赖
# ALSA 插件层：cpal 在 Linux 经 ALSA 出声；纯 PipeWire 系统（未装
# pipewire-alsa 兼容层）上会没有任何可用的 PCM 设备，表现为「完全无声」。
# 以 Recommends 形式兜底（默认桌面环境本就安装，不构成硬依赖）。
DEFAULT_DEPENDS="libc6 (>= 2.35), libgtk-3-0 (>= 3.24), libwebkit2gtk-4.1-0 (>= 2.40), libayatana-appindicator3-1 | libappindicator3-1, librsvg2-2, libasound2, libx11-6"
DEPENDS="${DEPENDS:-$DEFAULT_DEPENDS}"
if [ "$BUNDLE_FFMPEG" != "1" ] && ! printf '%s' "$DEPENDS" | grep -q 'ffmpeg'; then
  DEPENDS="$DEPENDS, ffmpeg | ffmpeg-free"
fi

# Recommends 组装：ALSA 插件层兜底（纯 PipeWire 系统无声问题）+ 未随包分发时的系统 ffmpeg
RECOMMENDS="pipewire-alsa | libasound2-plugins"
[ -n "$RECOMMENDS_FFMPEG" ] && RECOMMENDS="$RECOMMENDS, $RECOMMENDS_FFMPEG"

# ---------------------------------------------------------------- 5. DEBIAN 控制
echo "[5/6] 生成 DEBIAN 控制文件…"
DEBIAN="$PKG_ROOT/DEBIAN"
mkdir -p "$DEBIAN"

# 占位符替换：render() 已在脚本头部定义（\001 分隔符 + 值转义）
render "$PKG_TEMPLATE_DIR/control" "$DEBIAN/control"
# control 不允许注释行（dpkg-deb 严格 RFC822 解析）：剔除模板中以 # 开头的说明行
sed -i '/^[[:space:]]*#/d' "$DEBIAN/control"
# 空字段整行移除（空字段会触发 lintian 告警）：
#   Recommends 为空、或 LEGACY_PACKAGE 为空（未启用旧包接管）时删除对应行
if [ -z "$RECOMMENDS" ]; then
  sed -i '/^Recommends:[[:space:]]*$/d' "$DEBIAN/control"
fi
if [ -z "$LEGACY_PACKAGE" ]; then
  sed -i '/^\(Provides\|Conflicts\|Replaces\):[[:space:]]*$/d' "$DEBIAN/control"
fi
for script in preinst postinst prerm postrm; do
  render "$PKG_TEMPLATE_DIR/$script" "$DEBIAN/$script"
  chmod 0755 "$DEBIAN/$script"
done
chmod 0644 "$DEBIAN/control"

# 模板中的 @INSTALLED_SIZE@ 需在 md5sums 之前回填（control 里已渲染）
INSTALLED_SIZE="$(du -k -s "$PKG_ROOT" --exclude=DEBIAN | awk '{print $1}')"
sed -i "s|@INSTALLED_SIZE@|$INSTALLED_SIZE|g" "$DEBIAN/control"

# md5sums：Debian 标准的包内文件校验清单（不含 DEBIAN 自身）
( cd "$PKG_ROOT" && find . -path ./DEBIAN -prune -o -type f -print0 \
    | sed -z 's|^\./||' | LC_ALL=C sort -z | xargs -0 -r md5sum ) \
    > "$DEBIAN/md5sums"
chmod 0644 "$DEBIAN/md5sums"
MD5_COUNT="$(wc -l < "$DEBIAN/md5sums")"

# triggers：仅标准前缀 /usr 下有效（dpkg 只管理系统目录的触发器激活）
if [ "$PREFIX" = "/usr" ]; then
  install -m 0644 "$PKG_TEMPLATE_DIR/triggers" "$DEBIAN/triggers"
else
  echo "       提示：PREFIX=$PREFIX 非 /usr，跳过 triggers（安装后需手动刷新桌面/图标缓存）"
fi

# 权限规范化：目录 0755，普通文件 0644，bin 下可执行 0755
find "$PKG_ROOT" -type d -exec chmod 0755 {} +
find "$PKG_ROOT" -type f -exec chmod 0644 {} +
chmod 0755 "$PKG_ROOT$PREFIX/bin/$BIN_NAME"
if [ -d "$PKG_ROOT$BIN_LIB_DIR" ]; then
  find "$PKG_ROOT$BIN_LIB_DIR" -type f -exec chmod 0755 {} +
fi
for script in preinst postinst prerm postrm; do chmod 0755 "$DEBIAN/$script"; done

# ---------------------------------------------------------------- 6. 构建 + 校验
echo "[6/6] 构建 DEB 并校验…"
mkdir -p "$OUTPUT_DIR"
DEB_FILE="$OUTPUT_DIR/${PACKAGE_NAME}_${DEB_VERSION}_${ARCH}.deb"
rm -f "$DEB_FILE"

# --root-owner-group：无需 fakeroot 即以 root:root 归属打包（dpkg >= 1.19.3）
dpkg-deb --root-owner-group --build "$PKG_ROOT" "$DEB_FILE"

# --- 校验 1：元数据可读且关键字段齐全
INFO="$(dpkg-deb --info "$DEB_FILE")"
for field in "Package: $PACKAGE_NAME" "Version: $DEB_VERSION" "Architecture: $ARCH" "Maintainer:"; do
  printf '%s\n' "$INFO" | grep -q -- "$field" || { echo "错误：control 缺少字段 $field" >&2; exit 1; }
done
printf '%s\n' "$INFO" | grep -q "Depends:" || { echo "错误：control 缺少 Depends" >&2; exit 1; }

# --- 校验 2：内容清单与 md5sums 条数一致（防止文件遗漏/多余）
CONTENT_COUNT="$(dpkg-deb --contents "$DEB_FILE" | awk '$1 !~ /^d/ && $NF != "./"' | wc -l)"
[ "$CONTENT_COUNT" -eq "$MD5_COUNT" ] || {
  echo "错误：内容条目($CONTENT_COUNT) 与 md5sums($MD5_COUNT) 不一致" >&2; exit 1; }

# --- 校验 3：关键路径确实在包内
CONTENTS="$(dpkg-deb --contents "$DEB_FILE")"
for required in \
  "$PREFIX/bin/$BIN_NAME" \
  "$PREFIX/share/applications/com.tplayer.desktop" \
  "$PREFIX/share/icons/hicolor" \
  "$PREFIX/share/doc/$PACKAGE_NAME/copyright"
do
  printf '%s\n' "$CONTENTS" | grep -q "$required" || {
    echo "错误：安装包缺少 $required" >&2; exit 1; }
done

# --- 校验 4：md5sums 自身可校验（解包后逐条比对）
VERIFY_DIR="$STAGE/verify"
mkdir -p "$VERIFY_DIR"
dpkg-deb --fsys-tarfile "$DEB_FILE" | tar -xf - -C "$VERIFY_DIR"
( cd "$VERIFY_DIR" && md5sum -c "$DEBIAN/md5sums" --quiet ) || {
  echo "错误：md5sums 校验失败" >&2; exit 1; }
# 二进制架构在解包后再确认一次（与 dpkg Architecture 声明一致）
VERIFY_DESC="$(file -b "$VERIFY_DIR$PREFIX/bin/$BIN_NAME")"
case "$VERIFY_DESC" in *"$WANT_ELF"*) ;; *) echo "错误：包内二进制架构不符：$VERIFY_DESC" >&2; exit 1 ;; esac

# --- 校验 5：lintian（Debian 包检查器；可用则运行，问题仅告警）
if [ "$RUN_LINTIAN" = "auto" ]; then
  if command -v lintian >/dev/null 2>&1; then RUN_LINTIAN=1; else RUN_LINTIAN=0; fi
fi
if [ "$RUN_LINTIAN" = "1" ] && command -v lintian >/dev/null 2>&1; then
  echo "       lintian 检查（仅告警）…"
  lintian --no-tag-display-limit "$DEB_FILE" || echo "       提示：lintian 报告了上述问题（不阻断构建）"
fi

# --- 产物摘要 + SHA256 完整性清单
( cd "$OUTPUT_DIR" && sha256sum "$(basename "$DEB_FILE")" > SHA256SUMS )

echo ""
echo "========================================================="
echo " 构建完成"
echo "   安装包     : $DEB_FILE"
echo "   大小       : $(du -h "$DEB_FILE" | awk '{print $1}')"
echo "   架构       : $ARCH    安装前缀: $PREFIX"
echo "   依赖       : $DEPENDS"
echo "   Installed  : ${INSTALLED_SIZE} KiB / $MD5_COUNT 个文件"
echo "   校验清单   : $OUTPUT_DIR/SHA256SUMS"
echo ""
echo " 安装 / 卸载："
echo "   sudo ./scripts/install-deb.sh            # 安装（自动解析依赖）"
echo "   sudo ./scripts/uninstall-deb.sh          # 卸载（purge，保留用户数据）"
echo "   sudo ./scripts/uninstall-deb.sh --purge-data   # 连用户数据一并清除"
echo "========================================================="
