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

// English version of the prompt
const PROMPT_EN: &str = r#"You are a Shell command expert. Please generate or optimize shell commands based on user needs and execution history.

Requirements:
- For first execution (no history):
  - Generate an executable shell command

- If there's execution history:
  - Analyze the previous command's execution result
  - Determine if the expected goal was achieved
  - If the goal wasn't achieved, analyze possible reasons and generate an improved command
  - Include analysis results and improvement suggestions in your response

- If you need to write code or implement functionality that shell can't directly accomplish:
  - You can use Python scripts, for example:
cat << 'EOF' > hello.py
print("Hello, World!")
# ...
EOF
cat << 'EOF' > requirements.txt
# List all packages and versions
...
EOF
python -m venv venv
source venv/bin/activate
pip install -r requirements.txt
python hello.py

- Always follow these rules:
  - Commands should be as generic and comprehensive as possible, prioritizing built-in terminal commands over third-party ones
  - Ensure all command parameters are correct and exist
  - Don't use code block markers or other formatting markers

- Termination conditions:
  - Command executes successfully and achieves the expected goal
  - Number of consecutive failures exceeds the limit
  - User manually terminates
"#;

// Chinese version of the prompt
const PROMPT_ZH: &str = r#"你是一个Shell命令专家，请根据用户的需求和历史执行结果生成或优化shell命令。

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

// UI text translations
struct UiText {
    thinking: &'static str,
    generated_command: &'static str,
    dangerous_command_warning: &'static str,
    execute_command_prompt: &'static str,
    executing_command: &'static str,
    command_success: &'static str,
    command_failure: &'static str,
    goal_achieved_prompt: &'static str,
    max_attempts_reached: &'static str,
    first_run_config: &'static str,
    config_saved: &'static str,
    base_url_prompt: &'static str,
    api_key_prompt: &'static str,
    model_prompt: &'static str,
    language_prompt: &'static str,
    provide_description: &'static str,
    config_updated: &'static str,
}

const UI_TEXT_EN: UiText = UiText {
    thinking: "🤔 Thinking...",
    generated_command: "📝 Generated command:",
    dangerous_command_warning: "⚠️  Warning: Potentially dangerous command detected, execution refused!",
    execute_command_prompt: "Do you want to execute this command?",
    executing_command: "🚀 Executing command...",
    command_success: "✅ Command executed successfully!",
    command_failure: "❌ Command execution failed:",
    goal_achieved_prompt: "Did the command achieve the expected goal?",
    max_attempts_reached: "⚠️  Maximum number of attempts reached, program terminated.",
    first_run_config: "⚙️  First run requires configuration",
    config_saved: "✅ Configuration saved",
    base_url_prompt: "Enter API base URL",
    api_key_prompt: "Enter API key",
    model_prompt: "Enter model name",
    language_prompt: "Enter language (en/zh)",
    provide_description: "Please provide an operation description",
    config_updated: "Configuration updated",
};

const UI_TEXT_ZH: UiText = UiText {
    thinking: "🤔 正在思考中...",
    generated_command: "📝 生成的命令：",
    dangerous_command_warning: "⚠️  警告：检测到潜在的危险命令，拒绝执行！",
    execute_command_prompt: "是否要执行这个命令？",
    executing_command: "🚀 正在执行命令...",
    command_success: "✅ 命令执行成功！",
    command_failure: "❌ 命令执行失败：",
    goal_achieved_prompt: "命令是否达到了预期目标？",
    max_attempts_reached: "⚠️  已达到最大尝试次数，程序终止。",
    first_run_config: "⚙️  首次运行需要进行配置",
    config_saved: "✅ 配置已保存",
    base_url_prompt: "请输入API基础URL",
    api_key_prompt: "请输入API密钥",
    model_prompt: "请输入模型名称",
    language_prompt: "请输入语言 (en/zh)",
    provide_description: "请提供操作描述",
    config_updated: "配置已更新",
};

fn get_ui_text(language: &str) -> &'static UiText {
    match language {
        "en" => &UI_TEXT_EN,
        _ => &UI_TEXT_ZH,
    }
}

fn get_prompt(language: &str) -> &'static str {
    match language {
        "en" => PROMPT_EN,
        _ => PROMPT_ZH,
    }
}

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
    language: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct ApiConfig {
    base_url: String,
    api_key: String,
    model: String,
}

fn get_system_language() -> String {
    // Try to get system language from environment variables
    let lang = env::var("LANG")
        .or_else(|_| env::var("LC_ALL"))
        .or_else(|_| env::var("LANGUAGE"))
        .unwrap_or_else(|_| String::from("en_US.UTF-8"));
    
    // Extract language code from format like "en_US.UTF-8"
    if lang.starts_with("zh") {
        "zh".to_string()
    } else {
        "en".to_string()
    }
}

fn get_config_dir() -> Result<std::path::PathBuf> {
    let home = dirs::home_dir().context("Unable to get home directory")?;
    let config_dir = home.join(".askai");
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).context("Unable to create config directory")?;
    }
    Ok(config_dir)
}

fn get_config_path() -> Result<std::path::PathBuf> {
    Ok(get_config_dir()?.join("config.toml"))
}

fn load_config() -> Result<Config> {
    let config_path = get_config_path()?;
    if !config_path.exists() {
        // Get system language as default
        let default_language = get_system_language();
        // Get UI text based on system language
        let ui_text = get_ui_text(&default_language);
        
        println!("{}", style(ui_text.first_run_config).blue().bold());
        println!();

        let base_url = dialoguer::Input::<String>::with_theme(&ColorfulTheme::default())
            .with_prompt(ui_text.base_url_prompt)
            .default(String::from("https://api.openai.com/v1"))
            .interact()?;

        let api_key = dialoguer::Password::with_theme(&ColorfulTheme::default())
            .with_prompt(ui_text.api_key_prompt)
            .interact()?;

        let model = dialoguer::Input::<String>::with_theme(&ColorfulTheme::default())
            .with_prompt(ui_text.model_prompt)
            .default(String::from("gpt-3.5-turbo"))
            .interact()?;
            
        let language = dialoguer::Input::<String>::with_theme(&ColorfulTheme::default())
            .with_prompt(ui_text.language_prompt)
            .default(default_language)
            .interact()?;

        let config = Config {
            api: ApiConfig {
                base_url,
                api_key,
                model,
            },
            language,
        };

        save_config(&config)?;
        println!("{}", style(ui_text.config_saved).green().bold());
        return Ok(config);
    }
    let config_str = fs::read_to_string(&config_path).context("Unable to read config file")?;
    
    // 尝试解析配置文件，如果失败可能是旧版本配置缺少language字段
    match toml::from_str::<Config>(&config_str) {
        Ok(config) => Ok(config),
        Err(_) => {
            // 尝试解析为不包含language字段的旧版本配置
            #[derive(serde::Deserialize)]
            struct OldConfig {
                api: ApiConfig,
            }
            
            let old_config: OldConfig = toml::from_str(&config_str).context("Unable to parse config file")?;
            
            // 获取系统默认语言
            let default_language = get_system_language();
            // 获取对应语言的UI文本
            let ui_text = get_ui_text(&default_language);
            
            // 提示用户选择语言
            println!("{}", style("需要设置语言偏好").blue().bold());
            let language = dialoguer::Input::<String>::with_theme(&ColorfulTheme::default())
                .with_prompt(ui_text.language_prompt)
                .default(default_language)
                .interact()?;
            
            // 创建新的配置并保存
            let config = Config {
                api: old_config.api,
                language,
            };
            
            save_config(&config)?;
            println!("{}", style(ui_text.config_saved).green().bold());
            
            Ok(config)
        }
    }
}

fn save_config(config: &Config) -> Result<()> {
    let config_path = get_config_path()?;
    let config_str = toml::to_string_pretty(config).context("Unable to serialize config")?;
    fs::write(&config_path, config_str).context("Unable to save config file")?;
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
            language: String::from("en"),
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
            "language" => config.language = value.to_string(),
            _ => return Err(anyhow::anyhow!("未知的配置项: {}", key)),
        },
        _ => return Err(anyhow::anyhow!("未知的配置类型: {}", config_type)),
    }

    save_config(&config)?;
    println!("{}", style("Configuration updated").green().bold());
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
    let full_prompt = format!("{}
{}", get_prompt(&config.language), system_info);
    let user_prompt = match history {
        Some(h) => format!(
            "用户的问题为：{}
上一次执行的命令是：{}
执行结果是：{}
执行是否成功：{}
这是第{}次尝试。
请根据上述信息分析执行结果，判断是否达到预期目标，如果没有达到目标，分析原因并生成改进的命令。",
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
    let config = load_config()?;
    let ui_text = get_ui_text(&config.language);

    let mut attempt = 1;
    while attempt <= max_attempts {
        term.write_line(&format!("{}", style(ui_text.thinking).blue()))?;
        let command = get_ai_response(prompt.as_str(), history.as_ref(), cli.debug).await?;

        term.write_line("")?;
        term.write_line(&format!("{}", style(ui_text.generated_command).blue().bold()))?;
        term.write_line(&format!("{}", style(&command).cyan()))?;
        term.write_line("")?;

        if is_dangerous_command(&command) {
            term.write_line(&format!(
                "{}",
                style(ui_text.dangerous_command_warning)
                    .red()
                    .bold()
            ))?;
            return Ok(());
        }

        if !cli.dry_run {
            if Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt(ui_text.execute_command_prompt)
                .default(false)
                .interact()?
            {
                term.write_line("")?;
                term.write_line(&format!("{}", style(ui_text.executing_command).yellow()))?;

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
                    term.write_line(&format!("{}", style(ui_text.command_success).green()))?;
                } else {
                    term.write_line(&format!("{}", style(ui_text.command_failure).red()))?;
                }

                if !output_text.is_empty() {
                    term.write_line("")?;
                    term.write_line(&output_text)?;
                }

                if success {
                    if !Confirm::with_theme(&ColorfulTheme::default())
                        .with_prompt(ui_text.goal_achieved_prompt)
                        .default(true)
                        .interact()?
                    {
                        history = Some(ExecutionHistory {
                            command,
                            output: output_text,
                            success,
                            attempt,
                        });
                        attempt += 1;
                        continue;
                    }
                    return Ok(());
                }

                history = Some(ExecutionHistory {
                    command,
                    output: output_text,
                    success,
                    attempt,
                });
                attempt += 1;
                continue;
            }
            return Ok(());
        }

        return Ok(());
    }

    term.write_line(&format!(
        "{}",
        style(ui_text.max_attempts_reached).red().bold()
    ))?;
    Ok(())
}
