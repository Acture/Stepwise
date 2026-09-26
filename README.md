# Stepwise

学生提出下一步，系统检查并解释原因。最终答案只是完成标志，学习发生在每次选择与反馈中。

离线 Rust TUI；学生端不需要 Python、账号或网络。首版包含三种练习：

- **Python 表达式练习**：点击子表达式 → 原位变成 `____` → 填入值或异常名 → 提交检查，按依赖和优先级计算，同优先级的独立子式任选先后。
- **命题逻辑求值**：给定命题赋值，逐步计算非、且、或、实质蕴涵、等价。
- **自然演绎**：提出公式、规则、引用行，检查经典命题逻辑的推理及假设作用域。

## 运行

开发环境需要 Rust。已编译的程序可直接运行；首次 Cargo 构建需要下载依赖。运行时必须选择 `--python` 或 `--logic`，不能同时指定；自然演绎属于 `--logic`，`--proof NAME` 按名称打开题集里的证明题。`--help` 和 `--version` 无需选择语言。题目来自当前题集：默认是内置题集，`--set FILE` 换成外部 TOML 题集，格式见下文「题集文件」。

```fish
cargo run --locked -- --python
cargo run --locked -- --logic
cargo run --locked -- --logic --proof mp
cargo run --locked -- --logic --proof raa
```

默认直接随机出题，按 `n` 生成下一道随机题，按 `p` 回到本次运行的上一题。已有未完成进度时继续当前题，没写完的证明题也一样；完成后再次启动会出新题。`--random` 强制开始新题，`--exercise` 按名称从当前题集选题，`--list` 列出当前题集里该语言的题目：

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
cargo run --locked -- --logic --goal 'Q -> P' --premise P
```

`--goal` 写出一道不属于任何题集的证明题，前提用 `--premise` 逐条给出，没有前提也可以；它自己就是题目来源，不与 `--proof`、`--exercise` 或 `--set` 同用。

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

例如第一题为 `(a + b * c ** 2 - d // e % f) / (g - h) + -i ** 2 + j ** -k`，内置完整赋值。题集只存表达式、赋值和题目说明；解析器保留结构和括号，求值规则实时生成下一步、答案和解释，任意支持范围内的自定义公式也走相同流程。

`↑↓` / `j k` 也可选择运算单元，`Enter` 开始填空或提交；`Esc` 取消填空。`PgUp/PgDn` 或交互区内的滚轮滚动较长的当前式和反馈；历史用终端自身的滚动功能查看（鼠标捕获期间，许多终端需要按住 Shift 再滚动）。`h` 提示，`?` 帮助，`u` 撤销，`r` 重做，`n` 下一题（随机练习出新题，`--set` 时走到题集的下一题），`p` 上一题（随机练习限本次运行已出过的题），`s` 切换短路，`q` 或 `Ctrl+C` 退出（填空期间字符键用于输入）。撤销和重做会在终端追加标记。青色只标记学生选中的内容，不提前指出正确节点。窗口变窄时表达式自动换行，缩放保留草稿，无固定尺寸门槛。

自然演绎也在当前终端逐行输出，直接输入 `公式 ; 规则 ; 引用行`。`Enter` 检查，`Ctrl+Z` 撤销，`Esc` 清空，`F1` 在反馈处显示规则，`↑↓`、`PgUp/PgDn` 或滚轮滚动当前反馈，`Ctrl+N` 下一题、`Ctrl+P` 上一题（证明里每个字母都是输入，所以换题用控制键），`Ctrl+C` 退出。一行怎么写见下文「自然演绎」。

## 题集文件

题集是一份带版本号的 TOML 文件：一个稳定的题集名称，加上若干道题的题面、语言、表达式或公式和变量取值。内置题集本身就是这个格式（`questions/builtin.toml`），和导入的文件共用同一条加载路径、同一套检查，没有第二个格式，也没有兼容读法。`--set FILE` 用文件里的题集取代内置题集，此后 `--list`、`--trace`、`--exercise`、`--assign`、`--evaluation` 都作用于这份题集；`--python` / `--logic` 在题集内部筛选语言。`--set` 不能与自定义表达式、`--random`、`--seed`、`--goal`、`--premise`、`--equivalent` 同用：这些各自指定另一个题目来源，程序直接拒绝，不会收下再丢掉。`--proof NAME` 不在其中：它和 `--exercise` 走同一次查找，在当前加载的题集里按名称找题，只是要求找到的是证明题；`--check-proof` 检查的也是这道题。`--assign` 只给求值题赋值，落在证明题上会被拒绝，不会收下再丢掉；`--evaluation` 是整段练习的求值策略，从证明题开始时作用于其后的求值题。

`questions/example.toml` 是一份可直接运行的完整示例，字段如下：

```toml
version = 1                    # 只认 1；写别的版本会被拒绝，而不是猜着读
name = "example-set"           # 题集的稳定身份，进度按它保存
title = "示例题集"
description = "可选的一句话说明"

[[questions]]
kind = "evaluation"            # 必填并写明，不按出现了哪些字段推断
name = "literal-types"         # 题集内唯一，非空；就是 --exercise 里敲的那个词
title = "赋值写的是源码字面量"
language = "python"            # 或 "logic"；求值题必填
expression = "flag + (zero == negative)"
note = "可选：给人看的说明文字"
evaluation = "eager"           # 可选，"short-circuit" 或 "eager"
[questions.bindings]           # 可选；值是该语言的源码字面量，写成字符串
flag = "True"
zero = "0"
negative = "-0.0"

[[questions]]
kind = "proof"
name = "chain"
title = "连续两次肯定前件"
premises = ["P -> Q", "Q -> R", "P"]
conclusion = "R"               # 要推出的公式
note = "可选：给人看的说明文字"
```

`kind` 决定这道题有哪些字段，另一类的字段会被拒绝而不是忽略：求值题写 `premises`、证明题写 `evaluation` 都报未知字段。短路是求值策略，对证明没有意义。

格式里除了 `title` 和 `note`，每个字段都能被机器检查：版本号、名称唯一性、表达式、公式、取值都有对错。`note` 没有——一句话是不是说中了这道题，只有读的人知道，所以它是可选的：不写照样是一道完整的题，下一步、答案和解释本来就由规则从 `expression` 算出来。真正的目标是 `conclusion`（证明题要推出的公式），那个是会被检查的；说明文字不占用这个名字。

取值一律写成字符串，内容是该语言的源码字面量，交给该语言的解析器读，和学生填写答案走同一条规则。换成 TOML 自己的数字和布尔值就没有这条路了：一个 TOML 标量不属于任何一种语言，而这里要教的恰好是语言之间、类型之间的区别。`"0"` 是整数零，`"0.0"` 是浮点零，两者不是同一个答案；`"-0.0"` 与 `"0.0"` 相等却不是同一个浮点数；`"True"` 是布尔真，不是 `1`，也不是 TOML 的 `true`；TOML 的布尔值只有 `true` / `false`，既写不出 `None`，也写不出命题逻辑接受的 `T`、`真` 等真值别名。

`evaluation` 只决定求值题的开场策略，学生仍可按 `s` 切换，两种策略各存各的进度；不写就用语言的默认值（Python 短路，命题逻辑不短路）。

题集只写题面和条件，不写解法。下一步能选哪里、填什么值、为什么、证明的每一行合不合规则，都由程序里的运算与推理规则从这里的表达式和公式现算出来；文件里没有、也不该有一份逐步答案。

身份与进度：认的是题集 `name`，不是文件路径。文件改名、换目录、复制到别处都不影响已有记录，改 `name` 才算另一套题；反过来，两份文件写同一个 `name` 就是同一套题，这个名字是全局的。`builtin` 留给内置题集，导入的文件不能用。进度指针记的是「哪个题集的哪道题」，所以两套题集里都叫 `q1` 的题目不会互相打开对方的进度。每道题的答题记录按内容保存（教学规则版本、策略、语言、赋值、表达式），改标题或题面文字不影响记录，改表达式或赋值则是另一道题：它从头开始，旧记录仍留在进度文件里。题集加载失败时程序在解析进度文件路径之前就退出，不会碰已有进度。

加载时逐项检查，任何一项不通过就拒绝整份文件，并指出文件、出错的题目和字段（题集自身的字段不带题目）：TOML 语法错误、未知字段和写错类型的字段由 toml 自己报出，带行号和列号；版本号不是 1、题集名称或标题为空、题集没有题目、题目名称为空或重复、题目标题为空、取值不是该语言的字面量、表达式或公式解析不了，各报一句话。题集最多 1024 道题、1 MiB；证明题最多 64 条前提；表达式、公式和证明自身的规模上限见下文。以异常结束的题目（例如 `6 / 0`）是合法题目，照常加载。

带 `--set` 时，练习按文件顺序走这份题集中当前语言的全部题目，求值题和证明题排在同一条路上：求值题里按 `n` / `p`，证明题里按 `Ctrl+N` / `Ctrl+P` 前后换题，两种题各自的进度都跟着保存，回到一道题时从离开的地方接着做。最后一题再往后只提示「已经是本题集的最后一题。」，不会转而生成随机题。启动时从进度指向的那道题往后找第一道还没做完的题，证明题以在所有假设之外得到结论为完成；整套都做完了就停在最后一题。进度只记一个指针，所以在两套题集之间来回切换会丢掉离开那套时的位置——再回去时是从头查找未完成的题，而不是把已完成的第一题当新题递回来。不带 `--set` 的行为不变：默认随机出题，`n` 仍生成新的随机题。

```fish
cargo run --locked -- --python --set questions/example.toml --list
cargo run --locked -- --logic --set questions/example.toml --list
cargo run --locked -- --python --set questions/example.toml
cargo run --locked -- --logic --set questions/example.toml
cargo run --locked -- --logic --set questions/example.toml --exercise chain
cargo run --locked -- --python --set questions/example.toml --exercise eager-division --trace
```

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

证明题只给前提和结论。内置的三道 `mp`（肯定前件）、`raa`（反证法）、`identity`（蕴涵引入）和其他题目一样写在 `questions/builtin.toml` 里，`--logic --list` 列出它们，`--proof NAME` 或 `--exercise NAME` 打开。题集里只有题目，没有解法：推导的每一行都由规则现场检查。

每行写 `公式 ; 规则 ; 引用行`，用分号隔开三段：

- **公式**：这一行得到的命题，符号见上文「符号与语义」。`⊥`、`False` 都表示矛盾。
- **规则**：下表的规则名。
- **引用行**：逗号分隔的行号。前提依次是第 1 至第 n 行，之后每接受一行编号加一；`assume` 不引用任何行，第三段连同前面的分号都可以省略。

例如第 3 行是 `A → B`、第 5 行是 `A`，写 `B ; mp ; 3,5` 得到 `B`。`mp`、`not-elim`、`iff-intro` 的两条引用不分先后；`and-intro` 按引用的先后组成 `A ∧ B`。`A ; assume` 打开一个子证明，其后的行缩进显示，属于这个假设；`imp-intro`、`not-intro`、`raa` 引用该假设行和子证明的最后一行，把它关闭。比如第 4 行假设 `A`、子证明最后一行（第 7 行）得到 `C`，写 `A → C ; imp-intro ; 4,7`。

| 规则 | 引用 | 得到 |
| --- | --- | --- |
| `assume` | 无 | 任意公式，作为新假设 |
| `mp` | `A → B` 和 `A` | `B` |
| `and-intro` | `A` 和 `B` | `A ∧ B` |
| `and-left` / `and-right` | `A ∧ B` | `A` / `B` |
| `or-left` / `or-right` | `A` / `B` | `A ∨ B` |
| `not-elim` | `A` 和 `¬A` | `⊥` |
| `bottom-elim` | `⊥` | 任意公式 |
| `imp-intro` | 假设行 `A`，子证明末行 `B` | `A → B` |
| `not-intro` | 假设行 `A`，子证明末行 `⊥` | `¬A` |
| `raa` | 假设行 `¬A`，子证明末行 `⊥` | `A` |
| `iff-intro` | `A → B` 和 `B → A` | `A ↔ B` |
| `iff-left` / `iff-right` | `A ↔ B` | `A → B` / `B → A` |
| `copy` | 任意一行 | 同一公式 |

练习中按 `F1` 在反馈处显示这些写法和规则。结论必须出现在所有假设之外，才算证明完成。

`--check-proof FILE` 不进入练习、不读写进度，只检查一份写好的证明：文件是 JSON 字符串数组，每个元素是上面格式的一行。它按行输出检查结果，遇到不合规则的一行就停下并报错；各行都合规则但还没在所有假设之外得到结论，同样报错退出。要检查的证明题用 `--proof NAME` 指定（`--set` 时查那份题集），或用 `--goal` / `--premise` 写出：

```fish
cargo run --locked -- --logic --proof raa --check-proof 我的证明.json
cargo run --locked -- --logic --goal 'Q -> P' --premise P --check-proof 我的证明.json
```

## 进度、范围与开发

默认在系统本地应用数据目录的 Stepwise 目录保存 `progress.json`。`--progress-file PATH` 指定位置，`--no-save` 禁用读取和保存。正确步骤、撤销和换题后原子写入；重启通过重放步骤恢复历史。不同表达式、赋值（包括类型、浮点负零）、模式、教学规则版本分别保存；自然演绎按解析后的前提与结论隔离：内置证明题搬进题集时前提与结论的原文一字未改，此前保存的证明进度照常重放。进度指针同时记下题集名称与题目名称，题集按名称认身份而不按文件路径；证明题也会移动这个指针。证明题没有求值策略可存：从证明题恢复时，其后的求值题按 `--evaluation` 或该语言的默认策略打开。统一代入、同优先级任选及最终负号完成的新规则使用独立进度，旧记录保留，不重放旧版多出来的填写；自然演绎进度不变。损坏或无法重放的记录明确报错，不会静默覆盖。题集没有改变进度文件的格式，只是在指针旁边多记了题集名称。题集之前写下的进度文件照常读取，答题记录一条不丢；只是那个指针没写题集，等于「手上这道题不属于任何题集」——本版打不开它，于是直接出一道新题，旧记录留在文件里，用 `--exercise` 选回那道题仍能接着做。首版同一个进度文件只供一个程序实例使用。

教学公式最多 2048 字节，Python 教学树（包括分组）最多 128 节点、32 层，整数结果最多 4096 个二进制位；逻辑公式最多 128 个词法单元、32 层解析嵌套，BDD 等价检查最多 12 个命题。证明最多 256 行、16 层假设。超出支持范围不冒充 Python 错误或学生算错。

架构：`src/core` 提供与语言无关的教学机制：稳定节点 ID、树替换与来源映射、可选步骤检查、按类型的反馈、历史与重放；`src/python` 提供 Python 解析、运算规则、类型语义、优先级、短路规则、解释与出题语法；`src/logic` 提供符号解析、命题语义、BDD 检查、自然演绎与出题语法；`src/generate.rs` 提供共用的种子协议、采样流程与可完成性检查；`src/app` 是与界面无关的应用层，负责题目来源与换题、当前会话的选择与草稿、撤销、重新开始、策略切换、历史归档与进度快照，不依赖任何终端或窗口库；`src/tui` 只做适配：把终端事件映射为应用层操作并绘制其状态；`src/exercises.rs` 定义题集格式并解析 TOML，内置题集 `questions/builtin.toml` 与 `--set` 导入的文件（例如 `questions/example.toml`）走同一条加载路径；`src/progress.rs` 保存并重放进度。core 只通过 `Language` 与 `Op` 两个小接口调用语言模块，不内联任何一种语言的规则。Ratatui + Crossterm 提供终端界面，rustpython-parser 只负责 Python 解析。

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
