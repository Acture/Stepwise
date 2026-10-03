# 开发与架构

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
