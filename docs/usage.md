# 开始做题

下文在仓库根目录用 `cargo run --locked --` 启动。如果使用已编译的程序，把它替换为 `./stepwise`；Windows 使用 `stepwise.exe`。源码构建步骤见[开发指南](development.md)。

## 完成第一次练习

启动一道简单的 Python 题，暂不保存进度：

```fish
cargo run --locked -- --python '2 + (3 * 4)' --no-save
```

1. 点击 `*`，或用方向键选中乘法后按 `Enter`。上行保留 `2 + (3 * 4)`，下行显示 `2 + (____)`。
2. 填入 `12`，按 `Enter`。正确后下行变为 `2 + (12)`。如果填错，按反馈修改后重新提交。
3. 点击 `(12)`，直接去掉这一层括号，不必再输入 `12`。
4. 剩下 `2 + 12` 时，整式自动变成填空。输入结果，按 `Enter` 完成。

点错位置时，程序会解释为什么这一步还不能算。它不会替你选择正确的子式。`h` 可以请求提示，`u` 可以撤销上一步。

终端版在当前终端内逐行追加历史，不占用全屏。当前式会随窗口宽度换行；原式保留在填空上方，已经接受的步骤可以通过终端的滚动历史回看。

## 随机题、指定题和自定义题

启动时必须选择 `--python` 或 `--logic`，两者不能同时使用。默认随机出题；有未完成记录时继续上次的题目。`--random` 强制出新题，配合 `--seed` 可以复现一道随机题。

```fish
cargo run --locked -- --python
cargo run --locked -- --logic --random --seed 42
cargo run --locked -- --python --list
cargo run --locked -- --python --exercise long-arithmetic
```

`--list` 列出当前语言的内置题目，`--exercise NAME` 打开其中一题。自然演绎属于逻辑练习，可用 `--logic --proof raa` 打开。导入外部题集见[编写题集](question-sets.md)。

给出自己的公式时，在命令行中用引号括起整个式子；变量通过重复的 `--assign` 赋值：

```fish
cargo run --locked -- --python 'x + y * x' --assign x=2 --assign y=3
cargo run --locked -- --logic 'P -> (Q | ~P)' --assign P=false --assign Q=true
cargo run --locked -- --logic --goal 'Q -> P' --premise P
```

点击任意一处 `x`，填写一次即可代入所有同名位置，不受运算优先级限制。缺少变量赋值、重复赋值或提供了多余变量时，程序会报错。

随机练习中，`n` 生成下一题，`p` 回到本次运行的上一题，`r` 重做当前公式。用 `--set` 打开题集时，前后换题遵循文件顺序，到最后一题就停止。

## 快捷键

求值练习中，没有打开填空时：

| 按键 | 动作 |
| --- | --- |
| 点击、`↑↓`、`j k` | 选择子表达式 |
| `Enter` | 打开填空；填写时提交答案 |
| `Esc` | 取消当前填空 |
| `h` / `?` | 提示 / 帮助 |
| `u` / `r` | 撤销 / 重做当前题 |
| `n` / `p` | 下一题 / 上一题 |
| `PgUp/PgDn`、滚轮 | 滚动当前交互区 |
| `q` / `Ctrl+C` | 退出 |

填空打开时，字母键属于答案输入。查看终端原生历史时，某些终端需要按住 Shift 再滚动。

自然演绎中，直接输入 `公式 ; 规则 ; 引用行`，引用方法见[自然演绎规则](reference.md#自然演绎)。所有字母均用于输入，控制键如下：

| 按键 | 动作 |
| --- | --- |
| `Enter` | 检查当前行 |
| `Ctrl+Z` | 撤销上一行 |
| `Esc` | 清空正在输入的行 |
| `F1` | 显示规则 |
| `Ctrl+N` / `Ctrl+P` | 下一题 / 上一题 |
| `Ctrl+C` | 退出 |

## 保存和恢复

默认进度文件 `progress.json` 保存在：

| 系统 | 目录 |
| --- | --- |
| macOS | `~/Library/Application Support/io.github.acture.stepwise/` |
| Windows | `%LOCALAPPDATA%\acture\stepwise\data\` |
| Linux | `$XDG_DATA_HOME/stepwise/`，未设置时为 `~/.local/share/stepwise/` |

退出后重新启动同一种语言，会恢复尚未完成的练习。终端和桌面版共用这份进度；同一份文件请只交给一个正在运行的实例。

```fish
cargo run --locked -- --python --progress-file ./my-progress.json
cargo run --locked -- --python --no-save
```

`--progress-file` 更换保存位置，`--no-save` 同时禁用读取和保存，两者不能同用。读取失败或格式不受支持时，原文件不会被覆盖。升级导致教学规则变化时，旧求值记录仍保留，但新规则下会重新开始。

题目以异常结束也算完成。若要改走另一条路线，在当前题按 `u` 撤销；已经换题则先用 `p` 返回。随机练习只能返回本次运行出过的题。

## 常见疑问

- **结果正确，为什么不接受？** 每次只能提交一个允许的步骤，不能跨过尚未完成的子式。Python 答案还区分类型：`False`、`0`、`0.0` 不能互换。
- **为什么不自动去括号？** 去掉一层已算完的括号是单独的点击动作，可撤销，无需输入。负数幂底数的必要括号会保留，以免改变式子含义。
- **可以先算右边吗？** 同优先级的独立子式可以任选先后，原来的结合关系仍须保留。这是教学练习，不是严格重演 Python 执行顺序。
- **短路必须使用吗？** 不必。每一步可选择短路，也可以继续计算会被跳过的操作数；详细区别见[符号与规则](reference.md#短路与额外计算)。
