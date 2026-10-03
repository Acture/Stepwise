# 构建分发包

在 GitHub Actions 中手动运行 Package 工作流，选择待打包的分支。检查通过后，工作流构建 CLI 归档与桌面安装包；下载该次运行的 stepwise 产物，并用 SHA256SUMS 核对下载的文件。

构建入口见 [.github/workflows/package.yml](../.github/workflows/package.yml)。修改打包脚本后，运行 python3 -m unittest discover -s scripts/package -v。打包不会自动发布 GitHub Release。
