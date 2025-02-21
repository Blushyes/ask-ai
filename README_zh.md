<div align="center">

# 🤖 Ask AI

_让 AI 帮你生成最适合的 Shell 命令_

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Version](https://img.shields.io/badge/version-0.1.0-green.svg)](https://github.com/blushyes/ask-ai)

</div>

## ✨ 功能特点

- 🧠 基于 AI 的智能命令生成
- 🛡️ 内置危险命令检测
- 🎨 美观的命令行界面
- 🔍 支持调试模式
- 📝 详细的命令执行结果
- 🚀 支持 dry-run 模式

## 📦 安装

确保你的系统已安装 Rust 工具链，然后执行：

```bash
cargo install --path .
```

## 🔧 配置

首次运行时，程序会自动引导你完成配置。配置文件将保存在用户主目录的 `.askai/config.toml` 中。

你也可以通过命令行手动设置配置：

```bash
# 设置API基础URL
ask set config base_url=https://api.openai.com/v1

# 设置API密钥
ask set config api_key=your_api_key

# 设置模型名称
ask set config model=gpt-3.5-turbo
```

配置文件格式如下：

```toml
[api]
base_url = "你的OpenAI API地址"
api_key = "你的OpenAI API密钥"
model = "你要使用的模型名称（如：gpt-3.5-turbo）"
```

## 🚀 使用方法

```bash
# 基本使用
ask "查看当前目录下的所有文件"

# 只显示命令而不执行（dry-run模式）
ask --dry-run "查看系统内存使用情况"

# 显示调试信息
ask -D "列出所有正在运行的进程"

# 不显示详细输出
ask -v false "ping baidu.com"
```

## 📚 命令行参数

| 参数            | 描述               | 默认值 |
| --------------- | ------------------ | ------ |
| `<PROMPT>`      | 你想执行的操作描述 | 必填   |
| `-d, --dry-run` | 只显示命令而不执行 | false  |
| `-v, --verbose` | 显示详细输出       | true   |
| `-D, --debug`   | 显示调试信息       | false  |

## 🛡️ 安全特性

为了保护系统安全，程序会自动检测并拒绝执行以下危险命令：

- `rm -rf`
- `mkfs`
- `dd`
- `> /dev/`
- `chmod -R`
- 以及其他潜在的危险操作

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

## 📄 开源协议

本项目采用 MIT 协议开源，详见[LICENSE](LICENSE)文件。