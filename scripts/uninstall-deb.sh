#!/usr/bin/env bash
# =============================================================================
# TPlayer —— DEB 卸载脚本（Ubuntu / Debian）
# =============================================================================
#
# 只使用发行版原生工具（dpkg / apt），不引入任何第三方卸载技术。
#
# 功能：
#   1. 卸载前记录包内全部文件清单（用于卸载后残留校验）；
#   2. 请求运行中实例退出（prerm 已做，这里双保险）；
#   3. purge 卸载（删除包文件 + 系统级缓存），或 remove（保留配置）；
#   4. 卸载后校验：无残留包文件、包状态为已清除、命令与桌面入口消失；
#   5. --purge-data：连用户数据一并删除（~/.local/share/com.tplayer 等），
#      删除前需交互确认。
#
# 用法：
#   sudo ./scripts/uninstall-deb.sh                      # purge（保留用户数据）
#   sudo ./scripts/uninstall-deb.sh --purge-data         # 连用户数据一并清除
#   sudo ./scripts/uninstall-deb.sh --remove             # 仅 remove，保留缓存
#   ./scripts/uninstall-deb.sh --help
# =============================================================================

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PACKAGE_NAME="${PACKAGE_NAME:-tplayer}"
BIN_NAME="${BIN_NAME:-tplayer}"
APP_ID="com.tplayer"                      # tauri.conf.json identifier
DESKTOP_ID="com.tplayer.desktop"

PURGE_DATA=0
MODE="purge"

usage() {
  sed -n '2,26p' "$0" | sed 's/^# \{0,1\}//'
  exit "${1:-0}"
}

while [ $# -gt 0 ]; do
  case "$1" in
    --purge-data) PURGE_DATA=1; shift ;;
    --remove)     MODE="remove"; shift ;;
    -h|--help)    usage 0 ;;
    *) echo "未知参数：$1" >&2; usage 1 ;;
  esac
done

if [ "$(id -u)" -ne 0 ]; then
  echo "错误：请使用 sudo 运行（卸载需要 root 权限）" >&2
  exit 1
fi

echo "========================================================="
echo " TPlayer · 卸载"
echo "   模式     : $MODE$([ "$PURGE_DATA" -eq 1 ] && echo ' + 清除用户数据')"
echo "   包名     : $PACKAGE_NAME"
echo "========================================================="

# ---- 1. 安装状态检查 + 记录文件清单
if ! dpkg-query -W -f='${Status}' "$PACKAGE_NAME" 2>/dev/null | grep -q "install ok installed"; then
  echo "包 $PACKAGE_NAME 未安装（或已被移除），无需卸载。"
  # 未安装：仍提供空的文件清单占位，供下方「卸载后校验」遍历
  # （set -u 下 FILE_LIST 不得缺省）
  FILE_LIST="$(mktemp)"
  : > "$FILE_LIST"
else
  FILE_LIST="$(mktemp)"
  dpkg -L "$PACKAGE_NAME" > "$FILE_LIST"
  echo "[1/4] 已记录包内文件清单（$(wc -l < "$FILE_LIST") 条）"

  # ---- 2. 停止运行中的实例
  echo "[2/4] 停止运行中的实例…"
  if command -v pkill >/dev/null 2>&1; then
    pkill -x "$BIN_NAME" 2>/dev/null || true
    i=0
    while [ "$i" -lt 30 ]; do
      pgrep -x "$BIN_NAME" >/dev/null 2>&1 || break
      sleep 0.1 2>/dev/null || sleep 1
      i=$((i + 1))
    done
    pkill -9 -x "$BIN_NAME" 2>/dev/null || true
  fi

  # ---- 3. 卸载
  echo "[3/4] 卸载（$MODE）…"
  if command -v apt-get >/dev/null 2>&1; then
    if [ "$MODE" = "purge" ]; then
      DEBIAN_FRONTEND=noninteractive apt-get purge -y "$PACKAGE_NAME"
    else
      DEBIAN_FRONTEND=noninteractive apt-get remove -y "$PACKAGE_NAME"
    fi
  else
    if [ "$MODE" = "purge" ]; then
      dpkg --purge "$PACKAGE_NAME"
    else
      dpkg --remove "$PACKAGE_NAME"
    fi
  fi
fi

# ---- 4. 卸载后校验
echo "[4/4] 卸载后校验…"
fail=0

# 4.1 包状态
status="$(dpkg-query -W -f='${Status}' "$PACKAGE_NAME" 2>/dev/null || true)"
if [ -z "$status" ]; then
  echo "  ✓ 包已从 dpkg 数据库移除"
elif printf '%s' "$status" | grep -qE "not-installed|config-files"; then
  if [ "$MODE" = "purge" ]; then
    echo "  ! 包状态：$status（预期完全清除）" >&2
    fail=1
  else
    echo "  ✓ 包状态：$status（remove 模式保留配置文件属正常）"
  fi
else
  echo "  ✗ 包仍处于：$status" >&2
  fail=1
fi

# 4.2 包内文件残留（仅检查仍存在的普通文件；目录允许被其它包共享）
leftover=0
while IFS= read -r f; do
  [ -n "$f" ] || continue
  if [ -f "$f" ] || [ -L "$f" ]; then
    echo "  ✗ 残留文件：$f" >&2
    leftover=1
  fi
done < "$FILE_LIST"
if [ "$leftover" -eq 0 ]; then
  echo "  ✓ 无包内文件残留"
else
  fail=1
fi
rm -f "$FILE_LIST"

# 4.3 命令与桌面入口
if command -v "$BIN_NAME" >/dev/null 2>&1; then
  echo "  ✗ 命令仍可执行：$(command -v "$BIN_NAME")" >&2
  fail=1
else
  echo "  ✓ 命令已移除"
fi
if [ -e "/usr/share/applications/$DESKTOP_ID" ] || [ -L "/usr/share/applications/$DESKTOP_ID" ]; then
  echo "  ✗ 桌面入口仍存在" >&2
  fail=1
else
  echo "  ✓ 桌面入口已移除"
fi
for d in /usr/share/icons/hicolor/*/apps/tplayer.png; do
  [ -e "$d" ] || continue
  echo "  ✗ 图标残留：$d" >&2
  fail=1
done
if [ "$fail" -eq 0 ]; then
  echo "  ✓ 图标已移除"
fi

# ---- 5. 可选：清除用户数据
if [ "$PURGE_DATA" -eq 1 ]; then
  targets=(
    "${HOME}/.local/share/${APP_ID}"
    "${HOME}/.config/${APP_ID}"
    "${HOME}/.cache/${APP_ID}"
    "${XDG_DATA_HOME:-$HOME/.local/share}/${APP_ID}"
    "${XDG_CONFIG_HOME:-$HOME/.config}/${APP_ID}"
    "${XDG_CACHE_HOME:-$HOME/.cache}/${APP_ID}"
  )
  existing=()
  for t in "${targets[@]}"; do
    if [ -e "$t" ]; then existing+=("$t"); fi
  done
  if [ "${#existing[@]}" -eq 0 ]; then
    echo ""
    echo "未发现用户数据目录（${APP_ID}），无需清除。"
  else
    echo ""
    echo "即将删除以下用户数据（曲库、设置、播放进度、歌词偏移、封面缓存）："
    for t in ${existing[@]+"${existing[@]}"}; do echo "    $t"; done
    printf "确认删除？输入 yes 继续: "
    read -r answer
    if [ "$answer" = "yes" ]; then
      for t in ${existing[@]+"${existing[@]}"}; do
        if rm -rf "$t"; then echo "  已删除 $t"; fi
      done
    else
      echo "  已保留用户数据。"
    fi
  fi
else
  echo ""
  echo "用户数据（曲库/设置/播放进度/封面缓存）已保留："
  echo "    ~/.local/share/${APP_ID}"
  echo "如需一并清除，请执行：sudo $0 --purge-data"
fi

if [ "$fail" -ne 0 ]; then
  echo ""
  echo "卸载校验未全部通过（见上）。" >&2
  exit 1
fi

echo ""
echo "卸载完成。"
