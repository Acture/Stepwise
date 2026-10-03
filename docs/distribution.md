# 分发与验收

以下命令在仓库根目录执行。

本机构建 CLI：`cargo build --release --locked`，输出 `target/release/stepwise`（Windows 为 `.exe`）。[检查](../.github/workflows/check.yml)在每次推送和拉取请求上于 Linux、macOS、Windows 运行。分发只走一条管线：[打包工作流](../.github/workflows/package.yml)，只在 Actions 页面手动运行，不创建标签或 Release。它先跑完整套检查，所有打包作业都依赖同一次运行的门禁。

CLI 为四个目标各出一个归档：Linux x64（`x86_64-unknown-linux-musl`，静态链接）、macOS arm64 与 macOS x64（各只含一种架构，Intel 版在 Apple Silicon 运行器上交叉编译）、Windows x64（静态链接 C 运行时，不依赖 VCRUNTIME140.dll）。Linux 与 macOS 用 tar.gz，Windows 用 zip，每个都含程序、LICENSE、README.md、THIRD-PARTY-NOTICES.txt 和 RUST-STD-COPYRIGHT.html。

桌面在 `macos-26` 上分别构建 arm64 与 x64 的 dmg，在 `windows-2025` 上构建 NSIS 与 MSI，在 `ubuntu-24.04` 上构建 deb、rpm 与 AppImage。只上传安装包，`.app` 保留在 dmg 内，避免工作流产物丢失可执行位。macOS 仍是 ad-hoc 签名，不弹出挂载前 EULA；应用内随包携带完整许可证。11.0 是 Info.plist 声明的加载下限，不代表系统版本验收。Windows 安装包尚未签名，可能触发 SmartScreen 提示。安装后的 `licenses` 目录包含 AGPL 全文、COPYRIGHT、Rust 依赖声明、标准库声明、页面依赖声明和 WenKai OFL。CI 解包逐字节核对这些文件，在 dmg 内验证签名与架构，并从 Linux 程序和 AppImage 所有 ELF 的未定义动态符号测出 GLIBC 需求；这不是 Linux 运行时兼容性验收。

Rust 声明由 cargo-about 按 `about.toml` 与 `about.hbs` 逐目标生成，CLI 的 Linux 版另附 musl 许可证；标准库声明取自工具链。页面声明由锁定的 Vite 从构建模块生成，`src/desktop/ui/licenses.ts` 补入预打包 JS 的依赖、CSS/font 包和 Vite 注入的运行时代码，文本读取自锁定的依赖包。未审阅的许可证、缺失正文或占位版权行会使构建失败。桌面打包加载 `tauri.package.conf.json`，将 CI 生成的声明加入资源；`bun run app` 是本机开发构建，不代替这条分发管线。

CLI 的验收作业下载同次运行的确切目标归档，校验哈希后运行解包的二进制，覆盖非交互命令、退出码，以及 Unix pty 下的中文答案、进度续做、`--no-save` 和终端设置恢复。目标运行器为 ubuntu-24.04、macos-26（arm64）、macos-15-intel 与 macos-26-intel（x64）、windows-2025；Linux 归档还在 debian:11、fedora:43、alpine:3.22 中运行同一组断言。容器共用运行器内核，这只检查对发行版库的独立性。Windows 导入表检查为门禁，ConPTY 交互实验的实际结果见 [STATUS.md](../STATUS.md)。

桌面的验收作业同样按确切文件名下载同次运行的安装包，核对打包作业记下的哈希后，像学生那样安装：NSIS 静默装到 `%LOCALAPPDATA%\Stepwise`，MSI 按机器装到 `%ProgramFiles%\Stepwise`，各用一台 windows-2025；deb 由 apt 连同它依赖的 WebKitGTK 一起装上，AppImage 解包直接运行，都在 ubuntu-24.04 上。这四种由 tauri-driver 驱动装好的窗口（`src/desktop/e2e`）：找到画出的棋盘，从题目列表选题，点子式，中文答案被拒且留在空格里，用脚本派发的组字事件确认选字时的 Enter 不提交、普通 Enter 会提交，进度写成 `--progress-file` 指定的版本 3 文件，重开同一个已安装的应用回到同一题，再用 `真`、`⊥` 回答命题逻辑题，每一步截图。Windows 上整套测试经 gsudo 以 Medium 完整性运行，与学生平常的运行方式相同，msedgedriver 与机器上的 WebView2 运行时同版本。rpm（fedora:43 容器）、AppImage（没装 WebKitGTK 和 GTK 的 debian:13 容器）和 dmg（macos-26、macos-15-intel、macos-26-intel）做启动测试：等页面加载完成、进程仍在，截屏后结束；dmg 先校验镜像、架构与签名，并按应用当前的 ad-hoc 签名断言 Gatekeeper 拒绝。组字事件只测到守卫代码，不是真实输入法；启动测试只证明窗口起得来、页面加载完成，点击、判题与保存的逻辑各平台相同，由各平台都跑的 Rust 测试覆盖。

CLI 与桌面各自经验收后保留完整成功的一组产物，任一组失败不阻止另一组下载。汇总产物 `stepwise` 内的 SHA256SUMS 覆盖交付的每个归档和安装包，可在下载后运行 `shasum -a 256 -c SHA256SUMS`；所有工作流产物保留七天。实际运行与交付清单见 [STATUS.md](../STATUS.md)。尚未发布 Release；Developer ID 签名、公证、真实输入法与本机上的交互验收单独完成。
