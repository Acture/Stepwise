Stepwise {{VERSION}}：终端版与桌面版。

按系统选择文件，安装时的系统提示、数据位置与卸载方法见[下载与安装](https://github.com/Acture/Stepwise/blob/{{TAG}}/docs/install.md)。macOS 版经 Apple 公证，可直接打开；Windows 版没有代码签名，SmartScreen 若显示「Windows 已保护你的电脑」，先点「更多信息」，再点「仍要运行」。

## 校验下载

把下载的文件和 `SHA256SUMS` 放在同一目录。第一行是 macOS 的命令，Linux 用 `sha256sum -c SHA256SUMS --ignore-missing`，Windows 见[下载与安装](https://github.com/Acture/Stepwise/blob/{{TAG}}/docs/install.md#校验下载)；第二行核验构建来源，需要 GitHub CLI 2.68 或更高版本：

```fish
shasum -a 256 -c SHA256SUMS --ignore-missing
gh attestation verify 文件名 --repo Acture/Stepwise --signer-workflow Acture/Stepwise/.github/workflows/release.yml --source-ref refs/tags/{{TAG}}
```

## 源码与许可

这些文件由标签 [{{TAG}}](https://github.com/Acture/Stepwise/tree/{{TAG}}) 的源码构建，下方 Source code 是同一提交的归档。Stepwise 按 [AGPL-3.0-only](https://github.com/Acture/Stepwise/blob/{{TAG}}/LICENSE) 发布；各安装包与归档附带字体和第三方组件的许可声明。
