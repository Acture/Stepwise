# 构建分发包

本页面向维护者。只需本地运行时，使用[开发指南](development.md)中的 CLI 或桌面构建命令。

## 运行打包工作流

1. 将待打包代码提交到远端分支。
2. 在 GitHub 的 **Actions → Package → Run workflow** 中选择该分支，启动 [.github/workflows/package.yml](../.github/workflows/package.yml)。
3. 等待检查、构建和产物验收结束。工作流先调用完整的 `Check`，通过后才打包。
4. 从该次运行的 **Artifacts** 下载 `stepwise`，解压到独立目录。

`stepwise` 包含通过验收的 CLI 或桌面完整平台组及 `SHA256SUMS`。CLI 与桌面分别验收，其中一组失败时，另一组仍可交付；下载前先确认汇总包里确实含有所需平台。

| 程序 | 平台与格式 |
| --- | --- |
| CLI | Linux x86_64 musl、macOS arm64 / x86_64：tar.gz；Windows x86_64：zip |
| 桌面 | macOS arm64 / x86_64：dmg；Windows x86_64：NSIS / MSI；Linux x86_64：deb / rpm / AppImage |

桌面版在 Windows 上用系统的 WebView2 组件显示界面。已装有 WebView2 的电脑上，两种安装包都跳过它；缺少时，NSIS 安装包（`-setup.exe`）联网下载安装，MSI 则用内置的微软 WebView2 离线安装程序，可在离线电脑上安装，适合机房等由管理员统一安装的场合。

Artifacts 保留七天。需要长期提供下载时，应另外发布所需产物；打包工作流不会创建 tag 或 GitHub Release。

## 校验下载

在解压后的目录中，macOS 使用：

```fish
shasum -a 256 -c SHA256SUMS
```

Linux 使用 `sha256sum -c SHA256SUMS`。Windows 可在 PowerShell 中运行 `Get-FileHash .\文件名.zip -Algorithm SHA256`，对照 `SHA256SUMS` 中相同文件的记录。校验的是归档或安装包文件，不是其内部可执行文件。

## 修改打包配置

- 平台、归档命名和构建入口在 [package.yml](../.github/workflows/package.yml)。先更新相应测试，再修改构建或验收步骤。
- CLI 的 Rust 依赖声明由 [about.toml](../about.toml) 和 [about.hbs](../about.hbs) 生成。新增许可证种类前应阅读许可证；缺少版权信息时补充校验过的来源，不用泛化 SPDX 文本代替。
- 桌面页面声明由 [licenses.ts](../src/desktop/ui/licenses.ts) 生成，安装资源映射在 [tauri.package.conf.json](../src/desktop/src-tauri/tauri.package.conf.json)。
- Windows 先按 [tauri.windows.conf.json](../src/desktop/src-tauri/tauri.windows.conf.json) 构建并打出 NSIS，再用同一个可执行文件按 [tauri.msi.conf.json](../src/desktop/src-tauri/tauri.msi.conf.json) 打出 MSI。MSI 里的 WebView2 离线安装程序在打包时从微软下载最新版，不固定版本；提取检查确认它带有微软签名，并在运行摘要中记下版本与 SHA256。
- 安装包提取、许可证、架构和动态库检查在 [scripts/package](../scripts/package/)；修改后运行 `python3 -m unittest discover -s scripts/package -v`。

验证必须检查从产物中提取的文件。新增平台或改变打包方式时，保持构建、提取检查与最终哈希针对同一份产物。发布 macOS 安装包前还需单独确认签名和公证，不能把构建成功当作已满足系统分发要求。
