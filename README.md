<div align="center">

# 🤖 Ask AI

_AI-powered Shell Command Generator_

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Version](https://img.shields.io/badge/version-0.1.0-green.svg)](https://github.com/blushyes/ask-ai)

[English](README.md) | [中文](README_zh.md)

</div>

## ✨ Features

- 🧠 AI-powered intelligent command generation
- 🛡️ Built-in dangerous command detection
- 🎨 Beautiful command-line interface
- 🔍 Debug mode support
- 📝 Detailed command execution results
- 🚀 Dry-run mode support

## 📦 Installation

Ensure you have the Rust toolchain installed, then run:

```bash
cargo install --path .
```

## 🔧 Configuration

On first run, the program will guide you through the configuration process. The configuration file will be saved in `.askai/config.toml` in your home directory.

You can also manually set the configuration via command line:

```bash
# Set API base URL
ask set config base_url=https://api.openai.com/v1

# Set API key
ask set config api_key=your_api_key

# Set model name
ask set config model=gpt-3.5-turbo

# Set interface language (en/zh)
ask set config language=en
```

The configuration file format is as follows:

```toml
[api]
base_url = "Your OpenAI API URL"
api_key = "Your OpenAI API Key"
model = "Model name you want to use (e.g., gpt-3.5-turbo)"

language = "Interface language (en/zh)"
```

## 🚀 Usage

```bash
# Basic usage
ask "list all files in current directory"

# Show command without execution (dry-run mode)
ask --dry-run "check system memory usage"

# Show debug information
ask -D "list all running processes"

# Hide detailed output
ask -v false "ping baidu.com"
```

## 📚 Command Line Arguments

| Parameter       | Description                          | Default |
| -------------- | ------------------------------------ | ------- |
| `<PROMPT>`     | Description of what you want to do   | Required|
| `-d, --dry-run`| Show command without execution       | false   |
| `-v, --verbose`| Show detailed output                 | true    |
| `-D, --debug`  | Show debug information               | false   |

## 🛡️ Security Features

To protect system security, the program automatically detects and refuses to execute dangerous commands such as:

- `rm -rf`
- `mkfs`
- `dd`
- `> /dev/`
- `chmod -R`
- And other potentially dangerous operations

## 🤝 Contributing

Issues and Pull Requests are welcome!

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.