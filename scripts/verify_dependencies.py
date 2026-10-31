#!/usr/bin/env python3
"""依赖声明一致性验证脚本。

用途：在 CI 或本地提交前，验证 DEPENDENCIES.yml 中声明的直接依赖
与实际构建清单（Cargo.toml / package.json）是否一致，并检查是否存在
未审核（pending/rejected/unknown）的依赖。

退出码：0 表示通过，非 0 表示发现问题。
"""

import sys
import re
from pathlib import Path

try:
    import yaml
except ImportError:
    print("错误：缺少 PyYAML，请先安装：pip install pyyaml", file=sys.stderr)
    sys.exit(1)

ROOT = Path(__file__).resolve().parents[1]
DEP_FILE = ROOT / "DEPENDENCIES.yml"
CARGO_TOML = ROOT / "src-tauri" / "Cargo.toml"
PACKAGE_JSON = ROOT / "package.json"

ERRORS = []
WARNINGS = []


def load_yaml(path: Path):
    with path.open("r", encoding="utf-8") as f:
        return yaml.safe_load(f)


def extract_cargo_deps(path: Path) -> set[str]:
    """从 Cargo.toml 提取 [dependencies]、[build-dependencies] 和
    [target.'cfg(...)'.dependencies] 中的 crate 名。

    仅提取形如 `name = "..."` 或 `name = { ... }` 的行；忽略 feature 列表
    行（如 `"Foundation",`）以及没有等号的行。
    """
    text = path.read_text(encoding="utf-8")
    crates = set()

    def _collect(lines, start_marker):
        collecting = False
        for line in lines:
            stripped = line.strip()
            if stripped == start_marker:
                collecting = True
                continue
            if collecting and stripped.startswith("[") and stripped.endswith("]"):
                collecting = False
                continue
            if collecting and stripped and not stripped.startswith("#"):
                if "=" not in stripped:
                    # feature 列表行（如 "Foundation",），非 crate 名，跳过
                    continue
                name = stripped.split("=")[0].strip().strip('"').strip("'")
                if name:
                    crates.add(name)

    lines = text.splitlines()
    _collect(lines, "[dependencies]")
    _collect(lines, "[build-dependencies]")

    # 匹配平台特定依赖段 [target.'cfg(windows)'.dependencies]
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("[target") and "dependencies" in stripped:
            # 找到该段后，继续收集直到下一个 [ 段
            idx = lines.index(line)
            for subsequent in lines[idx + 1:]:
                s = subsequent.strip()
                if s.startswith("[") and s.endswith("]"):
                    break
                if s and not s.startswith("#") and "=" in s:
                    name = s.split("=")[0].strip().strip('"').strip("'")
                    if name:
                        crates.add(name)

    return crates


def extract_npm_deps(path: Path) -> set[str]:
    """从 package.json 提取 dependencies 与 devDependencies 包名。"""
    import json

    with path.open("r", encoding="utf-8") as f:
        data = json.load(f)
    pkgs = set()
    for section in ("dependencies", "devDependencies", "peerDependencies", "optionalDependencies"):
        for name in data.get(section, {}).keys():
            pkgs.add(name)
    return pkgs


def check_review_status(data: dict):
    """检查所有依赖条目的审核状态。"""
    sections = [
        ("rust_direct_dependencies", "Rust 直接依赖"),
        ("node_direct_dependencies", "Node.js 运行时依赖"),
        ("node_dev_dependencies", "Node.js 开发依赖"),
        ("bundled_binaries", "随包二进制"),
        ("third_party_code", "第三方引用代码"),
    ]
    for key, label in sections:
        items = data.get(key, [])
        if not isinstance(items, list):
            continue
        for item in items:
            name = item.get("name", "<未知>")
            status = item.get("review_status", "missing")
            if status == "missing":
                ERRORS.append(f"[{label}] {name}: 缺少 review_status 字段")
            elif status in ("pending", "rejected"):
                ERRORS.append(f"[{label}] {name}: 审核状态为 {status}，不得进入构建")
            elif status not in ("approved",):
                WARNINGS.append(f"[{label}] {name}: 审核状态为非标准值 '{status}'")


def check_unknown_license(data: dict):
    """检查是否存在未知许可证。"""
    sections = [
        "rust_direct_dependencies",
        "node_direct_dependencies",
        "node_dev_dependencies",
        "bundled_binaries",
        "third_party_code",
    ]
    for key in sections:
        items = data.get(key, [])
        if not isinstance(items, list):
            continue
        for item in items:
            name = item.get("name", "<未知>")
            lic = item.get("license", "")
            if "unknown" in lic.lower() or "LicenseRef-unknown" in lic:
                ERRORS.append(f"[{key}] {name}: 许可证为未知 ({lic})，禁止进入生产构建")


def check_cargo_consistency(data: dict, cargo_crates: set[str]):
    """验证 Rust 直接依赖与 Cargo.toml 是否一致。"""
    declared = set()
    for item in data.get("rust_direct_dependencies", []):
        declared.add(item.get("name", ""))
    missing_in_declared = cargo_crates - declared
    if missing_in_declared:
        for crate in sorted(missing_in_declared):
            ERRORS.append(f"[Rust] {crate}: 在 Cargo.toml 中存在但未在 DEPENDENCIES.yml 声明")
    extra_in_declared = declared - cargo_crates
    if extra_in_declared:
        for crate in sorted(extra_in_declared):
            WARNINGS.append(f"[Rust] {crate}: 在 DEPENDENCIES.yml 声明但 Cargo.toml 中未找到")


def check_npm_consistency(data: dict, npm_pkgs: set[str]):
    """验证 Node.js 依赖与 package.json 是否一致。"""
    declared = set()
    for key in ("node_direct_dependencies", "node_dev_dependencies"):
        for item in data.get(key, []):
            declared.add(item.get("name", ""))
    missing_in_declared = npm_pkgs - declared
    if missing_in_declared:
        for pkg in sorted(missing_in_declared):
            ERRORS.append(f"[Node] {pkg}: 在 package.json 中存在但未在 DEPENDENCIES.yml 声明")
    extra_in_declared = declared - npm_pkgs
    if extra_in_declared:
        for pkg in sorted(extra_in_declared):
            WARNINGS.append(f"[Node] {pkg}: 在 DEPENDENCIES.yml 声明但 package.json 中未找到")


def main():
    if not DEP_FILE.exists():
        print(f"错误：未找到 {DEP_FILE}", file=sys.stderr)
        sys.exit(1)

    data = load_yaml(DEP_FILE)

    check_review_status(data)
    check_unknown_license(data)

    if CARGO_TOML.exists():
        cargo_crates = extract_cargo_deps(CARGO_TOML)
        check_cargo_consistency(data, cargo_crates)
    else:
        WARNINGS.append("未找到 src-tauri/Cargo.toml，跳过 Rust 依赖一致性检查")

    if PACKAGE_JSON.exists():
        npm_pkgs = extract_npm_deps(PACKAGE_JSON)
        check_npm_consistency(data, npm_pkgs)
    else:
        WARNINGS.append("未找到 package.json，跳过 Node.js 依赖一致性检查")

    # 输出结果
    if WARNINGS:
        print("=== 警告 ===")
        for w in WARNINGS:
            print(f"  WARN: {w}")

    if ERRORS:
        print("\n=== 错误 ===")
        for e in ERRORS:
            print(f"  ERR : {e}")
        print(f"\n共 {len(ERRORS)} 个错误，{len(WARNINGS)} 个警告。依赖合规检查未通过。", file=sys.stderr)
        sys.exit(1)

    print(f"依赖合规检查通过（{len(WARNINGS)} 个警告）。")
    sys.exit(0)


if __name__ == "__main__":
    main()
