use anyhow::{Context, Result};
use clap::Parser;
use console::{style, Term};
use dialoguer::{theme::ColorfulTheme, Confirm};
use regex::Regex;
use reqwest::Client;
use serde_json::{json, Value};
use std::process::Command;
use std::{env, fs};
use toml;

#[derive(Debug)]
struct ExecutionHistory {
    command: String,
    output: String,
    success: bool,
    attempt: u32,
}

#[derive(Parser)]
#[command(author, version, about = "AI驱动的shell命令助手")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// 你想执行的操作描述
    #[arg(index = 1)]
    prompt: Option<String>,

    /// 只显示命令而不执行
    #[arg(short, long)]
    dry_run: bool,

    /// 显示详细输出
    #[arg(short, long, default_value = "true")]
    verbose: bool,

    /// 显示调试信息
    #[arg(short = 'D', long)]
    debug: bool,
}

#[derive(Parser)]
enum Commands {
    /// 设置配置项
    #[command(name = "set")]
    Set {
        /// 配置类型 (config)
        #[arg(index = 1)]
        config_type: String,

        /// 配置项 (key=value)
        #[arg(index = 2)]
        config_value: String,
    },
}

const DANGEROUS_COMMANDS: [&str; 6] = [
    "rm -rf",
    "mkfs",
    "dd",
    "> /dev/",
    "chmod -R",
    ":(){ :|:& };:",
];

fn get_system_info() -> String {
    let os = if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else {
        "Unknown OS"
    };

    let shell = env::var("SHELL").unwrap_or_else(|_| String::from("Unknown"));
    let term = env::var("TERM").unwrap_or_else(|_| String::from("Unknown"));
    let user = env::var("USER").unwrap_or_else(|_| String::from("Unknown"));
    let pwd = env::var("PWD").unwrap_or_else(|_| String::from("Unknown"));

    format!("当前系统环境信息：\n- 操作系统: {}\n- Shell类型: {}\n- 终端类型: {}\n- 当前用户: {}\n- 当前目录: {}\n", 
        os, shell, term, user, pwd)
}

const PROMPT: &str = r#"你是一个Shell命令专家，请根据用户的需求和历史执行结果生成或优化shell命令。

要求：
- 如果是首次执行（没有历史记录）：
  - 生成一个可执行的shell命令

- 如果有历史执行记录：
  - 分析上一次命令的执行结果
  - 判断是否达到了预期目标
  - 如果未达到目标，分析可能的原因并生成改进的命令
  - 在响应中包含分析结果和改进建议

- 如果需要写代码或实现shell无法直接完成的功能：
  - 可以使用python脚本方式，例如：
cat << 'EOF' > hello.py
print("Hello, World!")
# ...
EOF
cat << 'EOF' > requirements.txt
# 列出所有的包和版本
...
EOF
python -m venv venv
source venv/bin/activate
pip install -r requirements.txt
python hello.py

- 不管什么时候，你必须遵守的：
  - 命令应该尽可能通用和全面，优先使用终端自带的非第三方语句
  - 确保命令的所有参数都是正确且存在的
  - 不要使用代码块标记或其他格式标记

- 终止条件：
  - 命令执行成功且达到预期目标
  - 连续失败次数超过限制
  - 用户手动终止
"#;

fn is_dangerous_command(command: &str) -> bool {
    DANGEROUS_COMMANDS
        .iter()
        .any(|dangerous| command.to_lowercase().contains(dangerous))
}

fn clean_command_output(command: &str) -> String {
    let re = Regex::new(r"```(?:shell|bash)?\s*\n?([\s\S]*?)```").unwrap();
    if let Some(captures) = re.captures(command) {
        captures.get(1).unwrap().as_str().trim().to_string()
    } else {
        command.trim().to_string()
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
struct Config {
    api: ApiConfig,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct ApiConfig {
    base_url: String,
    api_key: String,
    model: String,
}

fn get_config_dir() -> Result<std::path::PathBuf> {
    let home = dirs::home_dir().context("无法获取用户主目录")?;
    let config_dir = home.join(".askai");
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).context("无法创建配置目录")?;
    }
    Ok(config_dir)
}

fn get_config_path() -> Result<std::path::PathBuf> {
    Ok(get_config_dir()?.join("config.toml"))
}

fn load_config() -> Result<Config> {
    let config_path = get_config_path()?;
    if !config_path.exists() {
        println!("{}", style("⚙️  首次运行需要进行配置").blue().bold());
        println!();

        let base_url = dialoguer::Input::<String>::with_theme(&ColorfulTheme::default())
            .with_prompt("请输入API基础URL")
            .default(String::from("https://api.openai.com/v1"))
            .interact()?;

        let api_key = dialoguer::Password::with_theme(&ColorfulTheme::default())
            .with_prompt("请输入API密钥")
            .interact()?;

        let model = dialoguer::Input::<String>::with_theme(&ColorfulTheme::default())
            .with_prompt("请输入模型名称")
            .default(String::from("gpt-3.5-turbo"))
            .interact()?;

        let config = Config {
            api: ApiConfig {
                base_url,
                api_key,
                model,
            },
        };

        save_config(&config)?;
        println!("{}", style("✅ 配置已保存").green().bold());
        return Ok(config);
    }
    let config_str = fs::read_to_string(&config_path).context("无法读取配置文件")?;
    let config: Config = toml::from_str(&config_str).context("无法解析配置文件")?;
    Ok(config)
}

fn save_config(config: &Config) -> Result<()> {
    let config_path = get_config_path()?;
    let config_str = toml::to_string_pretty(config).context("无法序列化配置")?;
    fs::write(&config_path, config_str).context("无法保存配置文件")?;
    Ok(())
}

fn set_config(config_type: &str, config_value: &str) -> Result<()> {
    let mut config = if let Ok(existing_config) = load_config() {
        existing_config
    } else {
        Config {
            api: ApiConfig {
                base_url: String::from("https://api.openai.com/v1"),
                api_key: String::new(),
                model: String::from("gpt-3.5-turbo"),
            },
        }
    };

    let parts: Vec<&str> = config_value.split('=').collect();
    if parts.len() != 2 {
        return Err(anyhow::anyhow!("配置格式错误，应为 key=value"));
    }

    let key = parts[0];
    let value = parts[1];

    match config_type {
        "config" => match key {
            "base_url" => config.api.base_url = value.to_string(),
            "api_key" => config.api.api_key = value.to_string(),
            "model" => config.api.model = value.to_string(),
            _ => return Err(anyhow::anyhow!("未知的配置项: {}", key)),
        },
        _ => return Err(anyhow::anyhow!("未知的配置类型: {}", config_type)),
    }

    save_config(&config)?;
    println!("配置已更新");
    Ok(())
}

async fn get_ai_response(
    prompt: &str,
    history: Option<&ExecutionHistory>,
    debug: bool,
) -> Result<String> {
    let client = Client::new();
    let config = load_config()?;
    let base_url = &config.api.base_url;
    let api_key = &config.api.api_key;
    let model = &config.api.model;

    let system_info = get_system_info();
    let full_prompt = format!("{}\n{}", PROMPT, system_info);
    let user_prompt = match history {
        Some(h) => format!(
            "用户的问题为：{}\n上一次执行的命令是：{}\n执行结果是：{}\n执行是否成功：{}\n这是第{}次尝试。\n请根据上述信息分析执行结果，判断是否达到预期目标，如果没有达到目标，分析原因并生成改进的命令。",
            prompt, h.command, h.output, h.success, h.attempt
        ),
        None => format!(
            "现在，用户的问题为：{}，请你根据用户的问题生成对应的shell命令来实现用户的需求。",
            prompt
        ),
    };

    if debug {
        println!("{}", style("🔍 调试信息：").blue().bold());
        println!("{}", style("系统提示：").blue());
        println!("{}", full_prompt);
        println!("{}", style("用户提示：").blue());
        println!("{}", user_prompt);
        println!();
    }

    let response = client
        .post(&format!("{}/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&json!({
            "model": model,
            "messages": [
                {
                    "role": "system",
                    "content": full_prompt,
                },
                {
                    "role": "user",
                    "content": user_prompt,
                }
            ]
        }))
        .send()
        .await
        .context("Failed to send request")?;

    let response_json: Value = response.json().await.context("Failed to parse response")?;
    let command = response_json["choices"][0]["message"]["content"]
        .as_str()
        .context("Failed to get command from response")?;

    Ok(clean_command_output(command))
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(Commands::Set { config_type, config_value }) = cli.command {
        return set_config(&config_type, &config_value);
    }

    let prompt = cli.prompt.ok_or_else(|| anyhow::anyhow!("请提供操作描述"))?;
    let term = Term::stdout();
    let mut history: Option<ExecutionHistory> = None;
    let max_attempts = 3;

    let mut attempt = 1;
    while attempt <= max_attempts {
        term.write_line(&format!("{}", style("🤔 正在思考中...").blue()))?;
        let command = get_ai_response(prompt.as_str(), history.as_ref(), cli.debug).await?;

        term.write_line("")?;
        term.write_line(&format!("{}", style("📝 生成的命令：").blue().bold()))?;
        term.write_line(&format!("{}", style(&command).cyan()))?;
        term.write_line("")?;

        if is_dangerous_command(&command) {
            term.write_line(&format!(
                "{}",
                style("⚠️  警告：检测到潜在的危险命令，拒绝执行！")
                    .red()
                    .bold()
            ))?;
            return Ok(());
        }

        if !cli.dry_run {
            if Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt("是否要执行这个命令？")
                .default(false)
                .interact()?
            {
                term.write_line("")?;
                term.write_line(&format!("{}", style("🚀 正在执行命令...").yellow()))?;

                #[cfg(target_os = "windows")]
                let output = Command::new("cmd")
                    .args(["/C", &command])
                    .output()
                    .context("Failed to execute command")?;

                #[cfg(not(target_os = "windows"))]
                let output = Command::new("sh")
                    .arg("-c")
                    .arg(&command)
                    .output()
                    .context("Failed to execute command")?;

                let success = output.status.success();
                let output_text = if success {
                    String::from_utf8_lossy(&output.stdout).to_string()
                } else {
                    String::from_utf8_lossy(&output.stderr).to_string()
                };

                if success {
                    term.write_line(&format!("{}", style("✅ 命令执行成功！").green().bold()))?;
                    if cli.verbose && !output_text.is_empty() {
                        term.write_line("")?;
                        term.write_line(&output_text)?;
                    }
                } else {
                    term.write_line(&format!(
                        "{} {}",
                        style("❌ 命令执行失败：").red().bold(),
                        style(&output_text).red()
                    ))?;
                }

                history = Some(ExecutionHistory {
                    command: command.clone(),
                    output: output_text,
                    success,
                    attempt,
                });

                if success {
                    if Confirm::with_theme(&ColorfulTheme::default())
                        .with_prompt("命令是否达到了预期目标？")
                        .default(true)
                        .interact()?
                    {
                        break;
                    }
                }
            } else {
                break;
            }
        } else {
            break;
        }

        attempt += 1;
        if attempt > max_attempts {
            term.write_line(&format!(
                "{}",
                style("⚠️  已达到最大尝试次数，程序终止。").yellow().bold()
            ))?;
        }
        term.write_line("")?;
    }

    Ok(())
}
