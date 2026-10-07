# 下载与安装

在 [Releases](https://github.com/Acture/Stepwise/releases) 页面下载最新版本。终端版与桌面版是分开的文件，可以只装其中一个；同一台电脑上的两者共用做题进度。

## 选择文件

文件名中的 `<版本>` 是版本号，例如 `0.1.0`。下文的命令都在下载文件所在的目录中运行（通常先 `cd ~/Downloads`），使用时把 `<版本>` 换成实际的版本号。

| 系统 | 桌面版 | 终端版 |
| --- | --- | --- |
| macOS，Apple 芯片 | `stepwise-desktop-<版本>-aarch64-apple-darwin.dmg` | `stepwise-<版本>-aarch64-apple-darwin.tar.gz` |
| macOS，Intel 芯片 | `stepwise-desktop-<版本>-x86_64-apple-darwin.dmg` | `stepwise-<版本>-x86_64-apple-darwin.tar.gz` |
| Windows x64 | `stepwise-desktop-<版本>-x86_64-pc-windows-msvc-setup.exe` 或 `stepwise-desktop-<版本>-x86_64-pc-windows-msvc.msi` | `stepwise-<版本>-x86_64-pc-windows-msvc.zip` |
| Linux x64 | `stepwise-desktop-<版本>-x86_64-unknown-linux-gnu` 加 `.deb`、`.rpm` 或 `.AppImage` | `stepwise-<版本>-x86_64-unknown-linux-musl.tar.gz` |

Mac 用的是哪种芯片，可在苹果菜单的「关于本机」中查看：「芯片」一栏写着 Apple M 开头的型号即 Apple 芯片，「处理器」一栏写着 Intel 即 Intel 芯片。终端版的归档里是程序 `stepwise`（Windows 为 `stepwise.exe`）、README、LICENSE 与第三方许可声明。

## 校验下载

`SHA256SUMS` 列出本版本每个文件的 SHA256。把下载的文件和它放在同一目录，macOS 运行：

```fish
shasum -a 256 -c SHA256SUMS --ignore-missing
```

Linux 运行 `sha256sum -c SHA256SUMS --ignore-missing`。Windows 在 PowerShell 中运行 `Get-FileHash .\文件名 -Algorithm SHA256`，与 `SHA256SUMS` 中同名文件的记录对照。

`SHA256SUMS` 列出的每个安装包与归档还有 GitHub 记录的构建来源证明（attestation），`SHA256SUMS` 本身没有。装有 [GitHub CLI](https://cli.github.com/) 2.68 或更高版本并已登录（`gh auth login`）时，可以核验文件出自本仓库该版本标签的发布工作流：

```fish
gh attestation verify 文件名 --repo Acture/Stepwise --signer-workflow Acture/Stepwise/.github/workflows/release.yml --source-ref refs/tags/v<版本>
```

它只说明文件的来历，不改变 macOS 或 Windows 打开程序时的安全提示。

## macOS

**桌面版**需要 macOS 11 或更高版本。打开 dmg，把 Stepwise 拖到「应用程序」文件夹，再从「应用程序」中打开。安装包经过 Apple 公证：第一次打开时，系统询问是否打开从互联网下载的 App，点「打开」即可。

**终端版**的程序同样经过 Apple 公证。在终端中解压并运行：

```fish
tar -xzf stepwise-<版本>-aarch64-apple-darwin.tar.gz
./stepwise-<版本>-aarch64-apple-darwin/stepwise --python
```

Intel 芯片把文件名中的 `aarch64` 换成 `x86_64`。从浏览器下载的程序第一次运行时，系统要联网向 Apple 核验它，离线时会拒绝运行。把 `stepwise` 移到 `PATH` 中的目录后，可以在任何位置直接输入 `stepwise`。

## Windows

**桌面版**有两种安装包，任选其一：

- `-setup.exe`，适合个人电脑：约 9 MB，安装到当前用户名下，不需要管理员权限。窗口依靠系统的 WebView2 组件显示；Windows 11 自带它，大多数 Windows 10 也已经由系统更新装好。电脑缺少 WebView2 时，安装程序会联网下载它。
- `.msi`，适合机房和离线电脑：两百多 MB，需要管理员权限，为这台电脑的所有用户安装到 Program Files。安装包内含微软的 WebView2 离线安装程序，不联网也能装好。

**终端版**：解压 zip，打开其中含有 `stepwise.exe` 的 `stepwise-<版本>-x86_64-pc-windows-msvc` 文件夹（用资源管理器「全部解压缩」时，它在同名文件夹里面一层），在空白处右键选「在终端中打开」，运行 `.\stepwise.exe --python`。

Windows 版的安装包和程序都没有代码签名：

- 打开安装包时，SmartScreen 可能显示「Windows 已保护你的电脑」。先点「更多信息」，再点随后出现的「仍要运行」。
- Windows 11 的「智能应用控制」开启时，会直接拦下没有良好信誉的未签名程序，而且不能为单个程序放行，只能在「Windows 安全中心 → 应用和浏览器控制 → 智能应用控制设置」中关闭这项功能。关闭后，整台电脑上的其他程序也不再受它保护，请自行权衡；学校或单位的电脑交给管理员决定。

## Linux

桌面版的三种包都需要 glibc 2.39 或更高版本，可用 `ldd --version` 查看。终端版是静态链接的程序，没有这项要求，也不需要下面的系统库。

deb 与 rpm 依赖系统的 WebKitGTK 4.1，用 apt 或 dnf 安装时会一并装上：

```fish
sudo apt install ./stepwise-desktop-<版本>-x86_64-unknown-linux-gnu.deb
sudo dnf install ./stepwise-desktop-<版本>-x86_64-unknown-linux-gnu.rpm
```

安装后从应用菜单打开 Stepwise，或在终端运行 `stepwise-desktop`。离线电脑上没有 WebKitGTK 4.1 时，改用 AppImage：它自带 WebKitGTK，不必另外安装，加上执行权限即可运行：

```fish
chmod +x stepwise-desktop-<版本>-x86_64-unknown-linux-gnu.AppImage
./stepwise-desktop-<版本>-x86_64-unknown-linux-gnu.AppImage
```

**终端版**：

```fish
tar -xzf stepwise-<版本>-x86_64-unknown-linux-musl.tar.gz
./stepwise-<版本>-x86_64-unknown-linux-musl/stepwise --python
```

## 数据与卸载

做题进度 `progress.json` 由终端版和桌面版共用，桌面版的外观设置 `appearance.json` 与它在同一目录。桌面版的网页引擎（WebView）另有自己的数据目录，里面没有进度。卸载时默认保留这些目录（`-setup.exe` 的卸载选项见下）：

| 系统 | 进度与外观 | WebView 数据 |
| --- | --- | --- |
| macOS | `~/Library/Application Support/io.github.acture.stepwise/` | `~/Library/WebKit/io.github.acture.stepwise/`、`~/Library/Caches/io.github.acture.stepwise/` |
| Windows | `%LOCALAPPDATA%\acture\stepwise\data\` | `%LOCALAPPDATA%\io.github.acture.stepwise\` |
| Linux | `$XDG_DATA_HOME/stepwise/`，未设置时为 `~/.local/share/stepwise/` | `$XDG_DATA_HOME/io.github.acture.stepwise/`，未设置时为 `~/.local/share/io.github.acture.stepwise/` |

卸载方法：

- macOS：把「应用程序」中的 Stepwise 移到废纸篓。终端版直接删除解压出的目录。
- Windows：在「设置 → 应用」的应用列表中卸载 Stepwise。卸载 `-setup.exe` 装的版本时，勾选 “Delete the application data” 会一并删除 WebView 数据，进度仍然保留。终端版直接删除解压出的目录。
- Linux：`sudo apt remove stepwise` 或 `sudo dnf remove stepwise`；AppImage 和终端版直接删除文件。

不再使用时，卸载后再删除上表中的目录即可清除全部数据。删除进度目录会丢失所有做题记录。
