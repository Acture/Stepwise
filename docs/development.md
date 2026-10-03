# 开发与架构

以下命令在仓库根目录执行。构建与测试不需要初始化私有 `notes/`。

架构：`src/core` 提供与语言无关的教学机制：稳定节点 ID、树替换与来源映射、可选步骤检查（学生此刻可以提交的全部步骤，以及提示和 `--trace` 所用的短路参考顺序）、按类型的反馈、历史与重放；`src/python` 提供 Python 解析、运算规则、类型语义、优先级、短路规则、解释与出题语法；`src/logic` 提供符号解析、命题语义、BDD 检查、自然演绎与出题语法；`src/generate.rs` 提供共用的种子协议、采样流程与可完成性检查；`src/app` 是与界面无关的应用层，负责题目来源与换题、当前会话的选择与草稿、撤销、重新开始、历史归档与进度快照，不依赖任何终端或窗口库；`src/tui` 只做适配：把终端事件映射为应用层操作并绘制其状态；`src/exercises.rs` 定义题集格式并解析 TOML，内置题集 `questions/builtin.toml` 与 `--set` 导入的文件（例如 `questions/example.toml`）走同一条加载路径；`src/progress.rs` 保存并重放进度。core 只通过 `Language` 与 `Op` 两个小接口调用语言模块，不内联任何一种语言的规则。Ratatui + Crossterm 提供终端界面，rustpython-parser 只负责 Python 解析。

## 原生检查

```fish
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo test --locked --test python_oracle -- --ignored --nocapture
ruff check tests/python_oracle.py
ruff format --check tests/python_oracle.py
ty check tests/python_oracle.py
```

CPython 对照测试需要开发机器上有 `python3`；程序运行本身不需要它。当前验证证据见 [STATUS.md](../STATUS.md)。

## 桌面源码与检查

桌面相关源码统一放在 `src/desktop/`，职责仍分开：

| 路径 | 职责 |
| --- | --- |
| `src/desktop/*.rs` | 共享的 `Desk / Command / View` 适配层，将窗口操作交给 `src/app`，不读写文件、不依赖窗口库 |
| `src/desktop/src-tauri/` | Tauri 壳，拥有窗口、文件读写、进度持久化和外观偏好 |
| `src/desktop/ui/` | Svelte 页面，呈现 `View` 并发送 `Command` |
| `src/desktop/e2e/` | 安装后应用的 WebDriver 验收 |
| `src/desktop/icon.svg` | 图标源文件 |

Cargo workspace 共享根目录的 `Cargo.lock` 和 `target/`。根目录默认只构建 `stepwise`，CLI 不编译 Tauri；窗口壳的包名是 `stepwise-desktop`。

先在 `src/desktop/ui/` 安装并检查页面：

```fish
bun install --frozen-lockfile
bun run check
bun run lint
bun run build
```

在 `src/desktop/e2e/` 检查安装包验收套件：

```fish
bun install --frozen-lockfile
bun run check
bun run lint
```

然后回到仓库根目录，检查窗口壳及其嵌入页面的构建：

```fish
cargo fmt --all --check
cargo clippy --locked -p stepwise-desktop --all-targets -- -D warnings
cargo test --locked -p stepwise-desktop
cargo check --locked -p stepwise-desktop --features tauri/custom-protocol
python3 -m unittest discover -s scripts/package -v
```

Rust 测试将类型声明生成到 `src/desktop/ui/src/lib/protocol/`；修改 Rust 类型后重新运行测试，不直接编辑生成文件。CI 检查生成结果是否与提交一致。安装包验收需要已打包的应用及平台驱动，入口与证据边界见 [分发与验收](distribution.md)。
