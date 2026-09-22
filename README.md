# Stepwise

学生提出下一步，系统检查并解释原因。最终答案只是完成标志，学习发生在每次选择与反馈中。

离线 Rust TUI；学生端不需要 Python、账号或网络。首版包含三种练习：

- **Python 表达式练习**：点击子表达式 → 原位变成 `____` → 填入值或异常名 → 提交检查，按依赖和优先级计算，同优先级的独立子式任选先后。
- **命题逻辑求值**：给定命题赋值，逐步计算非、且、或、实质蕴涵、等价。
- **自然演绎**：提出公式、规则、引用行，检查经典命题逻辑的推理及假设作用域。

## 运行

开发环境需要 Rust。已编译的程序可直接运行；首次 Cargo 构建需要下载依赖。运行时必须选择 `--python` 或 `--logic`，不能同时指定；自然演绎使用 `--logic --proof …`。`--help` 和 `--version` 无需选择语言。

```fish
cargo run --locked -- --python
cargo run --locked -- --logic
cargo run --locked -- --logic --proof mp
cargo run --locked -- --logic --proof raa
```

默认直接随机出题，按 `n` 生成下一道随机题，按 `p` 回到本次运行的上一题。已有未完成进度时继续当前题；完成后再次启动会出新题。`--random` 强制开始新题，`--exercise` 可指定示例：

```fish
cargo run --locked -- --python
cargo run --locked -- --logic
cargo run --locked -- --python --exercise precedence
cargo run --locked -- --python --random --seed 42
```

随机题由语法规则组合生成，包含变量及其赋值，并控制表达式规模、数值和答案长度。Python 随机题覆盖算术、比较及布尔运算，命题逻辑随机题覆盖五种联结词。生成时检查两种求值策略均可完成；随机练习暂不出除零等异常题，相关练习仍可用 `--exercise` 选择。`r` 重做当前题，`s` 切换短路均保留同一公式。当前随机题随进度保存，下次用同一语言模式启动可恢复；`--seed` 可复现指定题目。

自定义练习：

```fish
cargo run --locked -- --python 'False and (3 / 0 > 1)'
cargo run --locked -- --python 'not (2 > 3) and True'
cargo run --locked -- --python 'x + y * z' --assign x=2 --assign y=3 --assign z=4
cargo run --locked -- --logic 'P -> (Q | ~P)' --assign P=false --assign Q=true
cargo run --locked -- --logic --proof identity --goal 'Q -> P' --premise P
```

程序直接在当前终端运行，只保留末尾几行交互区，不进入全屏或备用屏幕。正确步骤逐行写入终端历史，退出后仍保留，启动前的 shell 输出也保留。一步前后的表达式文字相同时，不重复追加同一行；学生已提交的步骤仍用于撤销和恢复。没有树面板或 Tab 切换。

点击运算符或它直接作用的值后，上行保留点击前的完整式子，下行将所选运算单元变成 `____`，等待填写。比如 `2 + (3*4)` 点击乘法后下行是 `2 + (____)`，填写正确后为 `2 + (12)`；随后点击 `(12)` 就直接去掉这一层括号，变成 `2 + 12`，不用再填写 `12`。内部还没算完时不能直接去括号；嵌套括号逐层点击，每次都可撤销、保存和恢复。这是显式的教学动作，不表示 Python 在运行时执行了一条“去括号”指令。

整个式子只剩两个已知值和一个二元运算时（如 `2 + 12`），自动进入整式 `____` 填空，上方仍保留原式。系统不会自动填写或提交答案。括号、变量、尚未求值的子式和被短路跳过的分支不触发这个快捷操作。`Enter` 检查，正确才推进；填错保留草稿，选错位置解释原因。

整个式子最后只剩负号和一个非负数时，直接把它作为最终值，例如 `-2 ** 2` 填写乘方结果 `4` 后得到 `-4`，不再要求填写一次 `-4`。只省去最后这个重复填空；中间的取负运算、显式括号、`--10` 的再次取负和布尔值转数值仍需要处理。撤销回到最后一次学生操作之前。

变量代入不受运算优先级或出现顺序限制。点击任意一处变量，同名位置一起显示填空；填写一次，所有同名变量一起代入，撤销也一起恢复。例如 `x + y * x` 可以先点右边的 `x`，填写 `2` 后变成 `2 + y * 2`。赋值显示在题目开头。`--assign` 支持 Python 的整数、有限浮点数、`True/False/None`；命题逻辑赋值接受下表的真值别名。缺少赋值、重复 CLI 赋值或多余变量名称会报错。

同优先级且已具备操作数的独立子式可以任选先后，例如 `(2+3)*(4+5)` 可先算右边。括号各自形成局部优先级范围，不按嵌套深度给整道题排队；`(¬True) ∨ (True ∧ True ∧ (True ∧ True))` 可以先算左边的 `¬True`。包含括号的上层运算仍须等该括号处理完。优先级和原有结合关系仍然保留：`1+2+3*4` 先做乘法，`20-5-2` 不能改成先算 `5-2`。这是按 Python 运算规则进行的教学练习，不是逐条模拟 Python 的实际执行顺序。

长公式练习：

```fish
cargo run --locked -- --python --exercise long-arithmetic
cargo run --locked -- --python --exercise long-power
cargo run --locked -- --python --exercise long-python-logic
cargo run --locked -- --logic --exercise long-logic
```

例如第一题为 `(a + b * c ** 2 - d // e % f) / (g - h) + -i ** 2 + j ** -k`，内置完整赋值。题库只存表达式、赋值和题目说明；解析器保留结构和括号，求值规则实时生成下一步、答案和解释，任意支持范围内的自定义公式也走相同流程。

`↑↓` / `j k` 也可选择运算单元，`Enter` 开始填空或提交；`Esc` 取消填空。`PgUp/PgDn` 或交互区内的滚轮滚动较长的当前式和反馈；历史用终端自身的滚动功能查看（鼠标捕获期间，许多终端需要按住 Shift 再滚动）。`h` 提示，`?` 帮助，`u` 撤销，`r` 重做，`n` 下一道随机题，`p` 本次运行的上一题，`s` 切换短路，`q` 或 `Ctrl+C` 退出（填空期间字符键用于输入）。撤销和重做会在终端追加标记。青色只标记学生选中的内容，不提前指出正确节点。窗口变窄时表达式自动换行，缩放保留草稿，无固定尺寸门槛。

自然演绎也在当前终端逐行输出，直接输入 `公式 ; 规则 ; 引用行`。`Enter` 检查，`Ctrl+Z` 撤销，`Esc` 清空，`F1` 在反馈处显示规则，`↑↓`、`PgUp/PgDn` 或滚轮滚动当前反馈，`Ctrl+C` 退出。例如肯定前件练习输入 `Q ; mp ; 1,2`。

## 符号与语义

命题逻辑求值与自然演绎共用符号解析器。求值界面保留输入的符号、空格和括号，自然演绎显示统一逻辑符号：

| 联结词 | 等价写法 |
| --- | --- |
| 非 | `¬`、`~`、`∼`、`!`、`not` |
| 且 | `∧`、`&`、`&&`、`and` |
| 或 | `∨`、`\|`、`\|\|`、`or` |
| 蕴涵 | `→`、`⇒`、`⊃`、`->`、`=>` |
| 等价 | `↔`、`⇔`、`≡`、`<->`、`<=>` |

真值接受 `True/False`、`true/false`、`T/F`、`⊤/⊥`、`真/假`。命题名称使用英文字母开头的标识符；`T/F` 保留为真值。优先级为非 > 且 > 或 > 蕴涵 > 等价；蕴涵右结合。加括号可明确结构。首版不支持量词、谓词或模态逻辑。

Python 模式按 [Python 表达式规则](https://docs.python.org/3/reference/expressions.html) 解析，保留大小写、优先级和类型区别；逻辑符号别名不会改变 Python 语法。支持已赋值的变量、整数、有限浮点数、`True/False/None`，`+ - * / // % **`、一元正负号、`not/and/or`、单个 `== != < <= > >=` 比较。暂不支持字符串、调用、赋值语句、容器、链式比较、位运算或复数。中间的取负运算需要单独处理，最终仅剩负号和非负数时直接完成；`False`、`0`、`0.0` 不是同一个替换答案。浮点答案严格比较，包括负零，不使用隐藏误差阈值。

负数作为幂的底数时仍显示必要的括号，例如 `(-2) ** 2`；去分组这一步完成后，保护负数含义的括号不再是一条待做步骤，反馈会说明原因。否则显示为 `-2 ** 2` 会改变式子的含义。

**短路可选**：Python 默认开启，命题逻辑默认关闭；界面按 `s` 或 CLI 使用 `--evaluation short-circuit` / `--evaluation eager`。标题只显示 `Python 运算练习` 或 `命题逻辑`，切换时通过反馈提示短路状态。开启时，可能被跳过的分支须等左侧条件确定后才能计算；变量是题目给定的值，仍可统一代入，这不会执行分支中的运算。关闭时所有操作数都要求值，同优先级的独立运算可以任选先后。Python 的 `and/or` 仍返回对应操作数；关闭短路属于教学变体。短路不影响自然演绎。

```fish
cargo run --locked -- --python --trace 'False and (3 / 0 > 1)'
cargo run --locked -- --python --trace --evaluation eager 'False and (3 / 0 > 1)'
cargo run --locked -- --logic 'P -> Q' --equivalent '~P | Q'
```

第一条一次返回 `False`；第二条先执行 `3 / 0` 并以 `ZeroDivisionError` 结束。`--trace` 是独立的演示入口，不读写练习进度。`--equivalent` 使用 [boolean_expression](https://docs.rs/boolean_expression) 的 BDD 检查所有赋值，不是只检查当前赋值。

## 自然演绎

支持 `assume`、`mp`、`copy`、`and-intro`、`and-left/right`、`or-left/right`、`not-elim`、`bottom-elim`、`imp-intro`、`not-intro`、`raa`、`iff-intro`、`iff-left/right`。这是基础规则子集，尚未实现析取消去或自动证明搜索。

`raa` 是经典逻辑的反证规则：在假设 `¬A` 下推出 `⊥`，才能解除该假设得到 `A`。引入规则引用当前最内层假设行和子证明末行；禁止跨层解除、引用未来行或引用已关闭子证明中的行。在未解除的假设里写出目标不算完成。BDD 等价检查不能替代规则检查。

反证练习给定第 1 行 `¬¬P`，目标 `P`，依次输入：

```text
~P ; assume
False ; not-elim ; 1,2
P ; raa ; 2,3
```

也可检查保存的 JSON 步骤文件：

```fish
cargo run --locked -- --logic --proof raa --check-proof examples/raa.json
```

## 进度、范围与开发

默认在系统本地应用数据目录的 Stepwise 目录保存 `progress.json`。`--progress-file PATH` 指定位置，`--no-save` 禁用读取和保存。正确步骤、撤销和换题后原子写入；重启通过重放步骤恢复历史。不同表达式、赋值（包括类型、浮点负零）、模式、教学规则版本分别保存；自然演绎按前提与目标隔离。统一代入、同优先级任选及最终负号完成的新规则使用独立进度，旧记录保留，不重放旧版多出来的填写；自然演绎进度不变。损坏或无法重放的记录明确报错，不会静默覆盖。首版同一个进度文件只供一个程序实例使用。

教学公式最多 2048 字节，Python 教学树（包括分组）最多 128 节点、32 层，整数结果最多 4096 个二进制位；逻辑公式最多 128 个词法单元、32 层解析嵌套，BDD 等价检查最多 12 个命题。证明最多 256 行、16 层假设。超出支持范围不冒充 Python 错误或学生算错。

架构：`src/core` 提供与语言无关的教学机制：稳定节点 ID、树替换与来源映射、可选步骤检查、按类型的反馈、历史与重放；`src/python` 提供 Python 解析、运算规则、类型语义、优先级、短路规则、解释与出题语法；`src/logic` 提供符号解析、命题语义、BDD 检查、自然演绎与出题语法；`src/generate.rs` 提供共用的种子协议、采样流程与可完成性检查；`src/app` 是与界面无关的应用层，负责题目来源与换题、当前会话的选择与草稿、撤销/重做/策略切换、历史归档与进度快照，不依赖任何终端或窗口库；`src/tui` 只做适配：把终端事件映射为应用层操作并绘制其状态；`assets/exercises.json` 内嵌题库；`src/progress.rs` 保存并重放进度。core 只通过 `Language` 与 `Op` 两个小接口调用语言模块，不内联任何一种语言的规则。Ratatui + Crossterm 提供终端界面，rustpython-parser 只负责 Python 解析。

```fish
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo test --locked --test python_oracle -- --ignored --nocapture
ruff check tests/python_oracle.py
ruff format --check tests/python_oracle.py
ty check tests/python_oracle.py
```

CPython 对照测试需要开发机器上有 `python3`；程序运行本身不需要它。当前验证证据见 [STATUS.md](STATUS.md)。

本机构建分发文件：`cargo build --release --locked`，输出 `target/release/stepwise`（Windows 为 `.exe`）。[CI 配置](.github/workflows/check.yml) 在 Linux、macOS、Windows 检查；手动触发时额外构建各运行器原生架构的压缩包。尚未运行远程 CI，也未发布预编译包；macOS 签名、公证、Intel 包和 Linux 可移植性需要单独验收。
