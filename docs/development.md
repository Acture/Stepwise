# 开发指南

本指南从普通源码克隆开始，先运行 CLI，再按需安装桌面开发环境。

## 克隆并运行 CLI

安装 Rust stable（含 Cargo、rustfmt、Clippy）和 Git，然后运行：

```fish
git clone https://github.com/Acture/Stepwise.git
cd Stepwise
cargo run --locked -- --python
cargo run --locked -- --logic
```

无需递归初始化 submodule。首次构建会下载依赖，程序运行本身离线。`Cargo.toml` 的 `rust-version` 声明最低版本，CI 使用 stable。

修改后先跑默认包的检查；这些命令不会编译 Tauri：

```fish
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

只想运行一组测试，可指定测试目标，例如 `cargo test --locked --test choice`。要编译发行版 CLI，运行 `cargo build --locked --release`，产物位于根目录的 `target/release/`。

## 运行桌面版

除 Rust 外，还需要 Bun 和平台依赖：macOS 的 Xcode Command Line Tools、Windows 的 MSVC C++ 构建工具与 WebView2、Linux 的 GTK 3 与 WebKitGTK 4.1 开发库。Linux CI 安装的包名可查 [test.yml](../.github/workflows/test.yml) 中的桌面作业。

在仓库根目录进入页面目录，安装锁定依赖并启动带热更新的窗口：

```fish
cd src/desktop/ui
bun install --frozen-lockfile
bun run desktop
```

`bun run desktop` 同时启动开发页面和 Tauri 壳；`bun run dev` 只有页面服务器，不提供桌面命令接口。要独立检查界面布局，可运行 `bun run gallery`，打开生成的 `dist-gallery/gallery.html`，其中的屏幕由固定数据绘制。

macOS 本地构建 `.app`：在同一目录运行 `bun run app`，结果位于仓库根目录的 `target/release/bundle/macos/Stepwise.app`。跨平台分发使用[打包工作流](distribution.md)。

## 从哪里修改

| 要修改的内容 | 入口 |
| --- | --- |
| Python 解析、运算、解释或出题语法 | [src/python](../src/python/) |
| 命题逻辑、符号别名、自然演绎 | [src/logic](../src/logic/) |
| 可选步骤、答案校验、历史重放 | [src/core](../src/core/) |
| 换题、草稿、撤销与共享应用状态 | [src/app](../src/app/) |
| 终端按键与绘制 | [src/tui](../src/tui/) |
| 桌面命令和视图适配 | [src/desktop/mod.rs](../src/desktop/mod.rs) |
| 窗口、文件读取、进度保存、外观 | [src/desktop/src-tauri](../src/desktop/src-tauri/) |
| 页面组件与皮肤 | [src/desktop/ui/src](../src/desktop/ui/src/) |
| 题集加载或内置题目 | [src/exercises.rs](../src/exercises.rs)、[questions](../questions/) |
| 随机种子与采样流程 | [src/generate.rs](../src/generate.rs) |
| 进度格式与持久化 | [src/progress.rs](../src/progress.rs) |

两个界面调用同一套 `app` / `core`。修改判题应从语言模块或 core 入手，不在终端或页面重新实现规则。页面只呈现 `View` 并发送 `Command`；文件读写属于 CLI 或 Tauri 壳。

新增题目通常只需编辑 `questions/builtin.toml`；更适合单独分享给学生的题集，按[题集指南](question-sets.md)保存成外部文件即可。

## 按改动选择测试

Python 运算语义改动需与 CPython 对照；终端输入或渲染改动需通过 pty 驱动真实终端。两组测试需要开发机安装 `python3`，默认测试不运行它们：

```fish
cargo test --locked --test python_oracle -- --ignored --nocapture
cargo test --locked --test terminal -- --ignored --nocapture
```

修改证明规则时，在 `tests/proof_questions.rs` 的 `applied` 中加入验证用例，同时更新程序内规则文本和[公开规则表](reference.md#自然演绎)。完整证明放在测试中，用户文档只说明如何写一行。

修改题集格式时，更新[题集示例](question-sets.md#创建第一份题集)与 `tests/question_set.rs`；文档中的 TOML 会通过真实加载器测试。修改允许的步骤集合、随机题身份或文件格式时，检查对应的版本常量和恢复测试，避免旧进度被错误重放。

桌面修改先在 `src/desktop/ui` 运行：

```fish
bun run check
bun run lint
bun run build
```

再回到仓库根目录运行：

```fish
cargo fmt --all --check
cargo clippy --locked -p stepwise-desktop --all-targets -- -D warnings
cargo test --locked -p stepwise-desktop
cargo check --locked -p stepwise-desktop --features tauri/custom-protocol
```

最后一条检查内嵌页面，因此必须在页面构建之后运行。Rust 协议类型变动后，运行 `cargo test --locked --lib desktop` 和桌面包测试，重新生成 `src/desktop/ui/src/lib/protocol/` 下的 TypeScript 声明，再提交生成差异；不要直接编辑声明文件。

修改打包脚本时运行 `python3 -m unittest discover -s .github/scripts -v`。

## 提交前

使用项目现有的制表符缩进与显式类型。检查 `git diff`，提交相关代码、测试及文档；运行产物留在被忽略的构建目录。CI 会在多个系统上重复检查，并验证 Rust 与页面的协议声明没有漂移。
