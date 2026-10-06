# 构建分发包

本页面向维护者。只需本地运行时，使用[开发指南](development.md)中的 CLI 或桌面构建命令。

## 运行打包工作流

1. 确认仓库已配置 [macOS 签名凭据](#macos-签名与公证)。缺少时，macOS 的 CLI 与桌面作业在构建前失败。
2. 将待打包代码提交到远端分支。
3. 在 GitHub 的 **Actions → Package → Run workflow** 中选择该分支，启动 [.github/workflows/package.yml](../.github/workflows/package.yml)。
4. 等待检查与构建结束。工作流先调用完整的 `Check`，通过后才打包；打包作业会拆开自己打出的文件逐项检查。
5. 从该次运行的 **Artifacts** 下载 `stepwise`，解压到独立目录。

`stepwise` 包含打包作业全部通过的 CLI 或桌面完整平台组及 `SHA256SUMS`。CLI 与桌面分别汇总，其中一组失败时，另一组仍可交付；下载前先确认汇总包里确实含有所需平台。工作流只检查打出的文件本身，不在任何系统上安装或运行它们。

| 程序 | 平台与格式 |
| --- | --- |
| CLI | Linux x86_64 musl、macOS arm64 / x86_64：tar.gz；Windows x86_64：zip |
| 桌面 | macOS arm64 / x86_64：dmg；Windows x86_64：NSIS / MSI；Linux x86_64：deb / rpm / AppImage |

桌面版在 Windows 上用系统的 WebView2 组件显示界面。已装有 WebView2 的电脑上，两种安装包都跳过它；缺少时，NSIS 安装包（`-setup.exe`）联网下载安装，MSI 则用内置的微软 WebView2 离线安装程序，可在离线电脑上安装，适合机房等由管理员统一安装的场合。

Artifacts 保留七天。需要长期提供下载时，应另外发布所需产物；打包工作流不会创建 tag 或 GitHub Release。

## macOS 签名与公证

macOS 的 CLI 与 dmg 都用 Developer ID 签名并经苹果公证。学生从浏览器下载的 dmg 可直接打开，不必绕过 Gatekeeper；CLI 在终端中运行，首次运行需联网供 Gatekeeper 核验。工作流没有退回 ad-hoc 签名的路径：在仓库 **Settings → Secrets and variables → Actions** 中缺任何一项，或格式不对，macOS 作业都会在构建前失败。

| Secret | 内容 |
| --- | --- |
| `APPLE_CERTIFICATE` | Developer ID Application 证书连同私钥导出的 `.p12`，base64 编码 |
| `APPLE_CERTIFICATE_PASSWORD` | 导出 `.p12` 时设的密码；导出时不设密码，就不添加这一项 |
| `APPLE_SIGNING_IDENTITY` | 证书的完整名称 `Developer ID Application: 名称 (团队 ID)`，或其 SHA-1 |
| `APPLE_API_ISSUER` | App Store Connect API 的 Issuer ID |
| `APPLE_API_KEY` | 该 API 密钥的 Key ID（10 位） |
| `APPLE_API_PRIVATE_KEY` | 下载的 `AuthKey_<Key ID>.p8` 全文 |

API 密钥在 App Store Connect 的 **用户和访问 → 集成** 中创建，须是团队密钥（Team key），权限选 Developer：Tauri 提交公证时总带 Issuer ID，个人密钥会被拒绝。证书名称可在本机用 `security find-identity -v -p codesigning` 查到。在仓库目录中用 GitHub CLI 设置：

```fish
base64 -i DeveloperID.p12 | gh secret set APPLE_CERTIFICATE
gh secret set APPLE_CERTIFICATE_PASSWORD  # 仅当 .p12 设了密码
gh secret set APPLE_SIGNING_IDENTITY --body 'Developer ID Application: 名称 (团队 ID)'
gh secret set APPLE_API_ISSUER --body 'Issuer ID'
gh secret set APPLE_API_KEY --body 'Key ID'
gh secret set APPLE_API_PRIVATE_KEY < AuthKey_KEYID.p8
```

各作业的做法：

- CLI 二进制在打包前以 hardened runtime 和安全时间戳签名，压成 zip 提交公证。单独的可执行文件装订不了票据，tar.gz 里是同一份签名字节。从浏览器下载、解压出来的副本带隔离属性，首次在终端运行时 Gatekeeper 联网核验，离线时会被拒绝。
- 桌面：`tauri build` 用同一身份签名 `.app` 和 dmg，并公证 `.app`、装订票据；工作流随后公证 dmg 并装订，然后才提取检查和计算哈希。
- 公证结果必须为 Accepted。打包作业检查交付的 dmg 及其中的 `.app` 都装订了票据，然后才计算哈希。

一次运行提交六项公证（两个 `.app`、两个 dmg、两个 CLI），苹果建议每天不超过 75 次。签名身份与公证密钥只放在作业的临时钥匙串和临时文件中，作业结束时删除。本地 `bun run app` 仍按 [tauri.macos.conf.json](../src/desktop/src-tauri/tauri.macos.conf.json) 做 ad-hoc 签名，只供本机使用。

## 校验下载

在解压后的目录中，macOS 使用：

```fish
shasum -a 256 -c SHA256SUMS
```

Linux 使用 `sha256sum -c SHA256SUMS`。Windows 可在 PowerShell 中运行 `Get-FileHash .\文件名.zip -Algorithm SHA256`，对照 `SHA256SUMS` 中相同文件的记录。校验的是归档或安装包文件，不是其内部可执行文件。

## 修改打包配置

- 平台、归档命名和构建入口在 [package.yml](../.github/workflows/package.yml)。先更新相应测试，再修改构建或检查步骤。
- CLI 的 Rust 依赖声明由 [about.toml](../about.toml) 和 [about.hbs](../about.hbs) 生成。新增许可证种类前应阅读许可证；缺少版权信息时补充校验过的来源，不用泛化 SPDX 文本代替。
- 桌面页面声明由 [licenses.ts](../src/desktop/ui/licenses.ts) 生成，安装资源映射在 [tauri.package.conf.json](../src/desktop/src-tauri/tauri.package.conf.json)。
- Windows 先按 [tauri.windows.conf.json](../src/desktop/src-tauri/tauri.windows.conf.json) 构建并打出 NSIS，再用同一个可执行文件按 [tauri.msi.conf.json](../src/desktop/src-tauri/tauri.msi.conf.json) 打出 MSI。MSI 里的 WebView2 离线安装程序在打包时从微软下载最新版，不固定版本；提取检查确认它带有微软签名，并在运行摘要中记下版本与 SHA256。
- macOS 签名与公证在 [apple.py](../.github/scripts/apple.py)，票据检查在 [verify.py](../.github/scripts/verify.py)。
- 安装包提取、许可证、架构和动态库检查在 [.github/scripts](../.github/scripts/)；修改后运行 `python3 -m unittest discover -s .github/scripts -v`。

验证必须检查从产物中提取的文件。新增平台或改变打包方式时，保持构建、提取检查与最终哈希针对同一份产物。
