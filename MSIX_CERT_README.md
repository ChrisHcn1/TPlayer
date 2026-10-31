# MSIX 打包签名证书要求

MSIX 包必须由受信任的证书签名才能安装。分为两种场景：

| 场景 | 证书要求 | 适用 |
|------|---------|------|
| Microsoft Store 上架 | 由 Microsoft Store 签名（提交后由 Store 重签），本地打包可用测试证书 | 商业发布 |
| 企业内部分发 / 侧载 | 需企业可信根证书签名，客户端需信任该根证书 | 内部部署 |
| 本地测试 / 开发 | 自签名测试证书，安装前需手动信任 | 开发验证 |

## 1. 生成自签名测试证书（开发/测试用）

使用 PowerShell（管理员）：

```powershell
# 创建证书目录
mkdir e:/TPlayerNext/build

# 生成自签名证书（有效期 2 年）
$cert = New-SelfSignedCertificate `
    -Subject "CN=TPlayer Team" `
    -KeyExportPolicy Exportable `
    -KeySpec Signature `
    -KeyLength 2048 `
    -CertStoreLocation "Cert:\CurrentUser\My" `
    -NotAfter (Get-Date).AddYears(2)

# 导出为 PFX（需设置密码）
$pwd = ConvertTo-SecureString -String "你的证书密码" -Force -AsPlainText
Export-PfxCertificate -Cert $cert -FilePath "e:/TPlayerNext/build/TPlayerNext.pfx" -Password $pwd
```

将生成的 `TPlayerNext.pfx` 放到 `e:/TPlayerNext/build/`（与 `tauri.conf.json` 中的 `certificateFile` 路径一致）。

## 2. 打包时提供证书密码

```powershell
$env:MSIX_CERT_PASSWORD = "你的证书密码"
npm run build:win:msix
```

或：

```powershell
$env:MSIX_CERT_PASSWORD = "你的证书密码"
powershell -File scripts/build-win.ps1 -Target msix
```

## 3. 安装自签名 MSIX（测试环境）

自签名证书不受系统信任，安装前需将证书导入“受信任的根证书颁发机构”：

```powershell
# 导入证书到受信任根
Import-PfxCertificate -FilePath "e:/TPlayerNext/build/TPlayerNext.pfx" `
    -CertStoreLocation Cert:\LocalMachine\Root `
    -Password (ConvertTo-SecureString -String "你的证书密码" -Force -AsPlainText)

# 安装 MSIX
Add-AppxPackage "e:/TPlayerNext/src-tauri/target/release/bundle/msix/TPlayer.Next_2.0.3_x64.msix"
```

## 4. Microsoft Store 上架

- 提交 MSIX 到 Microsoft Store 时，Store 会使用其自身证书重新签名，本地 PFX 仅用于生成提交包；
- 需确保 `publisher`（`CN=TPlayer Team`）与开发者中心账户名称一致；
- 参考 `tauri.microsoftstore.conf.json` 中的 MSI 配置，MSIX 与 MSI 可并存。

## 5. 证书文件安全

- `TPlayerNext.pfx` 是私钥文件，**不要提交到版本控制**；
- `build/` 目录已应为 `.gitignore` 覆盖（如未覆盖请自行添加）；
- 密码通过环境变量 `MSIX_CERT_PASSWORD` 传入，不要写死在脚本中。

## 6. tauri.conf.json 中的 MSIX 配置（已就位）

```json
"msix": {
    "displayName": "TPlayer Next",
    "publisherDisplayName": "TPlayer Team",
    "publisher": "CN=TPlayer Team",
    "certificateFile": "../build/TPlayerNext.pfx",
    "certificatePassword": "${MSIX_CERT_PASSWORD}",
    "capabilities": ["internetClient"],
    "optional": true
}
```

- `certificateFile` 相对 `src-tauri/` 目录，故指向 `../build/TPlayerNext.pfx`（即仓库根 `build/`）；
- `certificatePassword` 使用 Tauri 环境变量替换语法，运行时从 `MSIX_CERT_PASSWORD` 读取；
- `optional: true` 表示 MSIX 为可选 target，缺少证书时不会阻断 NSIS 打包。
