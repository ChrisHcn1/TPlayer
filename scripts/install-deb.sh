#!/usr/bin/env bash
# =============================================================================
# TPlayer —— DEB 安装脚本（Ubuntu / Debian）
# =============================================================================
#
# 只使用发行版原生工具（dpkg / apt / update-alternatives 之外的系统命令），
# 不引入任何第三方安装技术。
#
# 功能：
#   1. 定位安装包（默认 dist/linux/*.deb，可按架构筛选，也可显式指定路径）；
#   2. SHA256 完整性校验（与构建产物 SHA256SUMS 比对）；
#   3. 安装并自动解析依赖（优先 apt 本地包安装，回退 dpkg -i + apt -f install）；
#   4. 安装后验证：dpkg 状态、dpkg -V 文件校验、命令可执行、桌面入口与
#      MIME 关联已注册；
#   5. 可选冒烟测试（SMOKE_TEST=1，需图形环境/xvfb）。
#
# 用法：
#   sudo ./scripts/install-deb.sh                    # 自动找最新 deb
#   sudo ARCH=arm64 ./scripts/install-deb.sh         # 指定架构
#   sudo ./scripts/install-deb.sh dist/linux/tplayer_2.0.0_amd64.deb
#   SMOKE_TEST=1 sudo ./scripts/install-deb.sh       # 安装后试启动
# =============================================================================

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PACKAGE_NAME="${PACKAGE_NAME:-tplayer}"
BIN_NAME="${BIN_NAME:-tplayer}"
OUTPUT_DIR="${OUTPUT_DIR:-$ROOT/dist/linux}"
ARCH="${ARCH:-$(dpkg --print-architecture 2>/dev/null || echo amd64)}"
SMOKE_TEST="${SMOKE_TEST:-0}"

DEB_INPUT="${1:-}"
if [ -n "$DEB_INPUT" ] && [ ! -f "$DEB_INPUT" ]; then
  echo "错误：找不到安装包 $DEB_INPUT" >&2
  exit 1
fi

# ---- 1. 定位安装包
if [ -z "$DEB_INPUT" ]; then
  if [ ! -d "$OUTPUT_DIR" ]; then
    echo "错误：产物目录 $OUTPUT_DIR 不存在，请先执行 scripts/build-deb.sh" >&2
    exit 1
  fi
  mapfile -t candidates < <(find "$OUTPUT_DIR" -maxdepth 1 -name "${PACKAGE_NAME}_*_${ARCH}.deb" -type f | sort -V)
  if [ "${#candidates[@]}" -eq 0 ]; then
    echo "错误：$OUTPUT_DIR 下没有架构为 $ARCH 的安装包" >&2
    echo "      可用文件：" >&2
    find "$OUTPUT_DIR" -maxdepth 1 -name '*.deb' -type f -printf '        %f\n' >&2 || true
    exit 1
  fi
  DEB_INPUT="${candidates[${#candidates[@]}-1]}"   # 取版本最新
fi

if [ "$(id -u)" -ne 0 ]; then
  echo "错误：请使用 sudo 运行（安装需要 root 权限）" >&2
  exit 1
fi

echo "========================================================="
echo " TPlayer · 安装"
echo "   安装包 : $DEB_INPUT"
echo "   架构   : $ARCH"
echo "========================================================="

# ---- 2. SHA256 完整性校验
if [ -f "$OUTPUT_DIR/SHA256SUMS" ]; then
  base="$(basename "$DEB_INPUT")"
  want="$(awk -v f="$base" '$2 == f {print $1}' "$OUTPUT_DIR/SHA256SUMS")"
  if [ -n "$want" ]; then
    got="$(sha256sum "$DEB_INPUT" | awk '{print $1}')"
    if [ "$want" != "$got" ]; then
      echo "错误：SHA256 校验失败（期望 $want，实际 $got）" >&2
      exit 1
    fi
    echo "[1/4] SHA256 校验通过：$got"
  else
    echo "[1/4] SHA256SUMS 中无 $base 记录，跳过比对"
  fi
else
  echo "[1/4] 未找到 SHA256SUMS，跳过完整性比对"
fi

# ---- 3. 安装（自动解析依赖）
echo "[2/4] 安装（apt 解析依赖）…"
if command -v apt-get >/dev/null 2>&1; then
  # apt 可直接安装本地 deb 并自动补齐依赖（apt >= 1.1）
  DEBIAN_FRONTEND=noninteractive apt-get install -y "$DEB_INPUT" || {
    echo "       apt 安装失败，回退 dpkg -i + apt-get -f install …"
    dpkg -i "$DEB_INPUT" || true
    DEBIAN_FRONTEND=noninteractive apt-get -f install -y
  }
else
  dpkg -i "$DEB_INPUT"
fi

# ---- 4. 安装后验证
echo "[3/4] 安装后验证…"
fail=0

dpkg-query -W -f='${Status}' "$PACKAGE_NAME" 2>/dev/null | grep -q "install ok installed" || {
  echo "  ✗ dpkg 状态不是 install ok installed" >&2; fail=1; }

# dpkg -V：按包内 md5sums 逐文件校验
if command -v dpkg >/dev/null 2>&1; then
  if dpkg -V "$PACKAGE_NAME" >/dev/null 2>&1; then
    echo "  ✓ 文件完整性校验通过（dpkg -V，md5sums）"
  else
    echo "  ✗ 文件完整性校验失败（dpkg -V）" >&2
    fail=1
  fi
fi

if command -v "$BIN_NAME" >/dev/null 2>&1; then
  echo "  ✓ 命令可执行：$(command -v "$BIN_NAME")"
else
  echo "  ✗ 命令 $BIN_NAME 不在 PATH 中" >&2
  fail=1
fi

DESKTOP_ID="com.tplayer.desktop"
if [ -f "/usr/share/applications/$DESKTOP_ID" ]; then
  echo "  ✓ 桌面入口已注册：/usr/share/applications/$DESKTOP_ID"
  if command -v desktop-file-validate >/dev/null 2>&1; then
    if desktop-file-validate "/usr/share/applications/$DESKTOP_ID"; then
      echo "  ✓ desktop 文件通过 desktop-file-validate"
    else
      echo "  ! desktop 文件存在告警（见上）" >&2
    fi
  fi
else
  echo "  ✗ 桌面入口缺失" >&2
  fail=1
fi

if command -v xdg-mime >/dev/null 2>&1; then
  default="$(xdg-mime query default audio/x-flac 2>/dev/null || true)"
  if [ -n "$default" ]; then
    echo "  ✓ 音频文件默认打开方式：$default"
  else
    echo "  ! 未查询到默认打开方式（可在文件管理器「打开方式」中选择 TPlayer）"
  fi
fi

if [ "$fail" -ne 0 ]; then
  echo "" >&2
  echo "安装后验证未全部通过（见上）。" >&2
  exit 1
fi

# ---- 5. 可选冒烟测试
if [ "$SMOKE_TEST" = "1" ]; then
  echo "[4/4] 冒烟测试（启动 5s 后关闭）…"
  run_cmd="$BIN_NAME"
  if [ -z "${DISPLAY:-}" ] && command -v xvfb-run >/dev/null 2>&1; then
    run_cmd="xvfb-run -a $BIN_NAME"
  fi
  if [ -z "${DISPLAY:-}" ] && ! command -v xvfb-run >/dev/null 2>&1; then
    echo "  ! 无图形环境且未安装 xvfb，跳过启动冒烟测试"
  else
    timeout 5 $run_cmd >/dev/null 2>&1 &
    pid=$!
    sleep 3
    if kill -0 "$pid" 2>/dev/null; then
      echo "  ✓ 进程成功启动（pid $pid），冒烟测试通过"
      kill "$pid" 2>/dev/null || true
    else
      # 进程已退出：timeout 回收（124）属正常，其它退出码视为启动失败
      rc=0
      wait "$pid" 2>/dev/null || rc=$?
      if [ "$rc" -eq 124 ]; then
        echo "  ✓ 进程启动并持续运行（由 timeout 正常回收）"
      else
        echo "  ✗ 进程启动后异常退出（退出码 $rc）" >&2
        exit 1
      fi
    fi
  fi
else
  echo "[4/4] 跳过冒烟测试（SMOKE_TEST=1 可启用）"
fi

echo ""
echo "安装完成：执行 '$BIN_NAME' 或从应用菜单启动。"
