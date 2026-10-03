# Stepwise

选择下一步算哪里，填入结果，立即得到反馈。Stepwise 提供 Python 表达式、命题逻辑求值和自然演绎练习，可在终端或桌面窗口中离线使用，不需要 Python、账号或网络。

点击一个子表达式后，它会变成 `____`，上方保留原式。答对后继续下一步，答错时保留输入并解释原因。变量可以一次全部代入；括号内算完后再点一下去掉括号，无需重复填写。

## 开始练习

如果已有编译好的程序，在终端选择一种语言启动：

```fish
./stepwise --python
./stepwise --logic
```

Windows 使用 `stepwise.exe`。默认随机出题；有未完成进度时继续上次的题目。终端内点击或用方向键选择，`Enter` 提交，`u` 撤销，`h` 提示，`n` 换题，`Ctrl+C` 退出。填写答案期间，字符键用于输入。桌面版打开窗口后选择语言即可开始。

从源码运行需要 Rust stable：

```fish
git clone https://github.com/Acture/Stepwise.git
cd Stepwise
cargo run --locked -- --python
```

首次构建需要下载依赖。桌面版还需要 Bun 和平台的 Tauri 构建依赖，步骤见开发指南。

## 文档

- [开始做题](https://github.com/Acture/Stepwise/blob/master/docs/usage.md)：第一次练习、快捷键、自定义公式与保存进度。
- [使用桌面版](https://github.com/Acture/Stepwise/blob/master/docs/desktop.md)：选题、导入题集、调整外观。
- [编写题集](https://github.com/Acture/Stepwise/blob/master/docs/question-sets.md)：创建和加载 TOML 题集。
- [符号与规则](https://github.com/Acture/Stepwise/blob/master/docs/reference.md)：支持的运算、短路选择与自然演绎写法。
- [开发指南](https://github.com/Acture/Stepwise/blob/master/docs/development.md)：构建、调试、模块入口与测试。
- [构建分发包](https://github.com/Acture/Stepwise/blob/master/docs/distribution.md)：打包、下载产物与校验。

## 许可

[AGPL-3.0-only](https://github.com/Acture/Stepwise/blob/master/LICENSE)。字体和第三方组件使用各自的许可证，分发包附带相应声明。
