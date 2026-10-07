# 构建分发包

本页面向维护者。只需本地运行时，使用[开发指南](development.md)中的 CLI 或桌面构建命令；下载发布版见[下载与安装](install.md)。

## 运行打包工作流

CLI 与桌面各有一个打包工作流：[package-cli.yml](../.github/workflows/package-cli.yml)（**Package CLI**）和 [package-desktop.yml](../.github/workflows/package-desktop.yml)（**Package Desktop**），互不依赖，只改了一边就只跑那一边。

1. 确认仓库已配置 [macOS 签名凭据](#macos-签名与公证)。缺少时，macOS 的作业在构建前失败。
2. 将待打包代码提交到远端分支。
3. 在 GitHub 的 **Actions** 中选择 **Package CLI** 或 **Package Desktop**，点 **Run workflow** 并选择该分支。
4. 等待测试与构建结束。工作流先跑这一边的测试（[test.yml](../.github/workflows/test.yml)），通过后才打包；打包作业会拆开自己打出的文件逐项检查。
5. 从该次运行的 **Artifacts** 下载 `stepwise-cli` 或 `stepwise-desktop`，解压到独立目录。

只有每个平台都通过时，运行才产出这份汇总包及其 `SHA256SUMS`；某个平台失败时，其他平台的单独产物仍保留在该次运行中。工作流不安装软件、不启动程序，只拆开打出的文件检查。格式与静态检查在 [lint.yml](../.github/workflows/lint.yml)，与测试一样只在 PR 上运行，并且只检查 PR 改到的部分（由 [changes.py](../.github/scripts/changes.py) 判断）；打包不等它。

| 程序 | 平台与格式 |
| --- | --- |
| CLI | Linux x86_64 musl、macOS arm64 / x86_64：tar.gz；Windows x86_64：zip |
| 桌面 | macOS arm64 / x86_64：dmg；Windows x86_64：NSIS / MSI；Linux x86_64：deb / rpm / AppImage |

桌面版在 Windows 上用系统的 WebView2 组件显示界面。已装有 WebView2 的电脑上，两种安装包都跳过它；缺少时，NSIS 安装包（`-setup.exe`）联网下载安装，MSI 则用内置的微软 WebView2 离线安装程序，可在离线电脑上安装，适合机房等由管理员统一安装的场合。

Artifacts 保留七天。手动运行不会创建 tag 或 GitHub Release；对外发布见[发布版本](#发布版本)。

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

每次 Package CLI 提交两项公证（两个 CLI），每次 Package Desktop 提交四项（两个 `.app`、两个 dmg）；苹果建议每天不超过 75 次。签名身份与公证密钥只放在作业的临时钥匙串和临时文件中，作业结束时删除。本地 `bun run app` 仍按 [tauri.macos.conf.json](../src/desktop/src-tauri/tauri.macos.conf.json) 做 ad-hoc 签名，只供本机使用。

## 发布版本

推送形如 `v0.1.0` 的标签后，[release.yml](../.github/workflows/release.yml)（**Release**）在标签指向的提交上跑完上面两个打包工作流，再把它们检查过的全部文件放进一个草稿 Release。草稿由维护者亲自发布。

1. 把根目录 [Cargo.toml](../Cargo.toml) 与 [src/desktop/src-tauri/Cargo.toml](../src/desktop/src-tauri/Cargo.toml) 的 `version` 改成同一个新版本号，运行 `cargo check` 更新 `Cargo.lock`，经 PR 合入 master。版本号只用 `主.次.修订` 三段数字：MSI 不接受带字母的预发布号，而 deb 与 rpm 会把 `0.1.0-1` 这类纯数字后缀排在 `0.1.0` 之后。
2. 为 master 上的这个提交打标签并推送。标签一经推送就是公开的，推送前确认这个提交就是要发布的版本：

   ```fish
   git fetch origin
   git log -1 origin/master
   git tag v0.1.0 origin/master
   git push origin v0.1.0
   ```

3. 等待 Release 运行结束。它依次：
   - 检查标签是 `v主.次.修订`，且等于两个 crate 的版本，否则在构建前失败；
   - 跑 Package CLI 与 Package Desktop：测试、构建、签名公证与提取检查；
   - 确认该标签还没有任何 Release（包括草稿），然后建一个草稿；
   - 把 CLI 归档、桌面安装包和合并后的 `SHA256SUMS` 上传到草稿，再从草稿下载回来，校验文件集合与每个文件的 SHA256；
   - 为 `SHA256SUMS` 列出的每个文件生成 GitHub 的构建来源证明（由 GitHub 保存，不在 Release 的文件中），核验命令见[下载与安装](install.md#校验下载)。
4. 在 Apple 芯片的 Mac 上，用浏览器从草稿下载 arm64 的 dmg 与 CLI 归档（这样文件带有隔离属性，与学生下载的一样），安装并打开桌面版，在终端第一次运行 CLI。
5. 没有问题后，在 GitHub 的 **Releases** 中编辑草稿，按需修改说明，然后发布。

每次 Release 提交六项公证。权限按作业分配：测试与构建只能读仓库，能写草稿的只有建草稿与上传两个作业，能签构建来源证明的只有最后一个作业。

某个作业因网络或苹果服务等临时原因失败时，在同一次运行中只重跑失败的作业：上传作业（`upload`）重跑时覆盖已上传的同名文件，证明作业（`attest`）重跑时为同样的摘要再签一次。草稿一旦建成，建草稿的作业（`draft`）就会拒绝这个标签；要重跑它或整次运行，先删除草稿（`gh release delete v0.1.0 --yes`，标签保留）。Release 运行期间，不要在同一标签上手动启动打包工作流。草稿中的文件验收不通过时，删除草稿，修复后升到下一个修订号重新打标签，不复用已推送的版本号。

## 校验下载

在解压后的目录中，macOS 使用：

```fish
shasum -a 256 -c SHA256SUMS
```

Linux 使用 `sha256sum -c SHA256SUMS`。Windows 可在 PowerShell 中运行 `Get-FileHash .\文件名.zip -Algorithm SHA256`，对照 `SHA256SUMS` 中相同文件的记录。校验的是归档或安装包文件，不是其内部可执行文件。

## 修改打包配置

- 平台、归档命名和构建入口在 [package-cli.yml](../.github/workflows/package-cli.yml) 与 [package-desktop.yml](../.github/workflows/package-desktop.yml)。先更新相应测试，再修改构建或检查步骤。
- CLI 的 Rust 依赖声明由 [about.toml](../about.toml) 和 [about.hbs](../about.hbs) 生成。新增许可证种类前应阅读许可证；缺少版权信息时补充校验过的来源，不用泛化 SPDX 文本代替。
- 桌面页面声明由 [licenses.ts](../src/desktop/ui/licenses.ts) 生成，安装资源映射在 [tauri.package.conf.json](../src/desktop/src-tauri/tauri.package.conf.json)。
- Windows 先按 [tauri.windows.conf.json](../src/desktop/src-tauri/tauri.windows.conf.json) 构建并打出 NSIS，再用同一个可执行文件按 [tauri.msi.conf.json](../src/desktop/src-tauri/tauri.msi.conf.json) 打出 MSI。MSI 里的 WebView2 离线安装程序在打包时从微软下载最新版，不固定版本；提取检查确认它带有微软签名，并在运行摘要中记下版本与 SHA256。
- macOS 签名与公证在 [apple.py](../.github/scripts/apple.py)，票据检查在 [verify.py](../.github/scripts/verify.py)。
- [release.yml](../.github/workflows/release.yml) 按名称下载两个打包工作流的汇总 Artifact（`stepwise-cli`、`stepwise-desktop`），改动这两个名称时一并修改它。产物文件名（CLI 的在 package-cli.yml，桌面的在 verify.py）改变时，同步修改[下载与安装](install.md)中的文件表与命令。发布说明的模板是 [release-notes.md](../.github/release-notes.md)。
- 安装包提取、许可证、架构和动态库检查在 [.github/scripts](../.github/scripts/)；修改后运行 `python3 -m unittest discover -s .github/scripts -v`。

验证必须检查从产物中提取的文件。新增平台或改变打包方式时，保持构建、提取检查与最终哈希针对同一份产物。
