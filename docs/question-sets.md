# 编写题集

题集使用 TOML。每道求值题写表达式和变量赋值，每道证明题写前提和结论；不用编写求解步骤或答案，程序会按规则检查学生的每次操作。

## 创建第一份题集

把下面内容保存为 `my-set.toml`。其中包含一道 Python 求值题和一道逻辑证明题：

```toml
version = 2
name = "example-set"
title = "示例题集"
description = "练习变量取值和推理规则"

[[questions]]
kind = "evaluation"
name = "literal-types"
title = "变量与类型"
language = "python"
expression = "flag + (zero == negative)"
note = "先查看题目给出的变量取值。"
[questions.bindings]
flag = "True"
zero = "0"
negative = "-0.0"

[[questions]]
kind = "proof"
name = "chain"
title = "连续推理"
premises = ["P -> Q", "Q -> R", "P"]
conclusion = "R"
```

在仓库根目录运行以下命令，先检查能否读取，再开始练习：

```fish
cargo run --locked -- --python --set my-set.toml --list
cargo run --locked -- --logic --set my-set.toml --list
cargo run --locked -- --python --set my-set.toml
cargo run --locked -- --logic --set my-set.toml --proof chain
```

使用编译好的程序时，将 `cargo run --locked --` 替换为 `./stepwise`。桌面版在题目列表中打开这个文件即可。

`--python` / `--logic` 选择题集里的对应语言。逻辑求值题与证明题可以混排；换题按文件顺序走，到最后一题停止。`--exercise NAME` 可直接选题，`--proof NAME` 额外要求它是证明题。

## 字段怎么填

| 范围 | 必填字段 | 可选字段 |
| --- | --- | --- |
| 题集 | `version = 2`、`name`、`title`、`questions` | `description` |
| 求值题 | `kind = "evaluation"`、`name`、`title`、`language`、`expression` | `bindings`、`note` |
| 证明题 | `kind = "proof"`、`name`、`title`、`conclusion` | `premises`、`note` |

求值题的 `language` 为 `"python"` 或 `"logic"`。证明题使用命题逻辑，不写 `language`；没有前提时省略 `premises` 或写 `premises = []`。每道题的 `name` 在题集内必须唯一且非空，标题也不能为空。

变量值必须是**字符串形式的语言字面量**：写 `x = "2"`，不要写 `x = 2`。`"0"`、`"0.0"`、`"-0.0"` 保留各自的数值类型和符号；`"True"` 是 Python 布尔值，`"None"` 也可使用。逻辑题可用 `"True"`、`"False"` 等[真值别名](reference.md#命题逻辑符号)。

不含变量时可以省略 `bindings`。`note` 是给学生看的可选提示，不影响判题。求值题不能写 `premises`，证明题不能写 `expression`，未知字段会报错。

想写逻辑求值题，可在求值题中设置 `language = "logic"`，再写 `expression = "P -> Q"`，并在该题的 `bindings` 中给出 `P`、`Q` 的真值。更多可运行例子见[示例题集](../questions/example.toml)。

## 修改题集与保存进度

为自己的题集取一个稳定、独有的 `name`，不要使用保留名称 `builtin`。文件改名或换目录不会改变身份；两份文件用了相同的 `name`，程序就把它们看作同一题集。

修改标题或说明不影响已有作答。修改表达式、变量值、前提或结论后，改变的题目从头开始，旧记录仍保留。加载失败不会改写已有进度。

`--set` 会替换内置题集，不能与自定义表达式、`--random`、`--seed`、`--goal`、`--premise` 或 `--equivalent` 同用。`--assign` 可以覆盖求值题的变量值，不能用于证明题。

## 排查导入错误

- **版本不支持**：当前使用 `version = 2`。升级旧版文件时，删除所有 `evaluation` 字段，再把版本改成 2；短路现在由学生在每一步自行选择。
- **字段类型错误**：检查字符串是否加引号，`premises` 是否为字符串数组，`bindings` 的值是否也为字符串。
- **题目无法解析**：按错误中的题目名定位，核对[支持的符号和运算](reference.md)。缺少变量值或给了多余变量也会报错。
- **超过大小限制**：一份题集最多 1 MiB、1024 道题；一道证明题最多 64 条前提。单个公式的限制见[规则参考](reference.md#输入范围)。

任意一题不合法都会拒绝整份文件。TOML 语法和字段错误会给出行列位置。结果为异常的求值题（如 `6 / 0`）是合法题目，学生填写异常名即可。
