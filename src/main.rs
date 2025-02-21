use anyhow::{Context, Result};
use clap::Parser;
use console::{style, Term};
use dialoguer::{theme::ColorfulTheme, Confirm};
use dotenv::dotenv;
use regex::Regex;
use reqwest::Client;
use serde_json::{json, Value};
use std::env;
use std::process::Command;

#[derive(Parser)]
#[command(author, version, about = "AI驱动的shell命令助手")]
struct Cli {
    /// 你想执行的操作描述
    #[arg(index = 1)]
    prompt: String,

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

const PROMPT: &str = r#"你是一个Shell命令专家，请根据用户的需求生成对应的shell命令。

要求：
- 只需要输出可执行的shell命令，不需要任何解释
- 生成的命令应该尽可能通用和全面，确保能够显示完整的信息。只返回命令本身，不要有其他解释。对于网络相关的查询，优先使用 lsof 或 netstat 等更通用的命令。
- 不要使用代码块标记（```）或其他格式标记
- 如果用户需要写代码，或者实现什么shell做不到的功能，可以类似如下方式用python写脚本写入py文件后执行py文件（假设用户安装了python环境）：
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
- 一个命令能完成用户的需求，不要拆分成多步
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

async fn get_ai_response(prompt: &str, debug: bool) -> Result<String> {
    let client = Client::new();
    let base_url = env::var("OPENAI_BASE_URL").context("OPENAI_BASE_URL not set")?;
    let api_key = env::var("OPENAI_API_KEY").context("OPENAI_API_KEY not set")?;
    let model = env::var("OPENAI_MODEL").context("OPENAI_MODEL not set")?;

    let system_info = get_system_info();
    let full_prompt = format!("{}\n{}", PROMPT, system_info);
    let user_prompt = format!(
        "现在，用户的问题为：{}，请你根据用户的问题生成对应的shell命令来实现用户的需求。",
        prompt
    );

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
    dotenv().ok();
    let cli = Cli::parse();
    let term = Term::stdout();

    term.write_line(&format!("{}", style("🤔 正在思考中...").blue()))?;
    let command = get_ai_response(&cli.prompt, cli.debug).await?;

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

            let output = Command::new("sh")
                .arg("-c")
                .arg(&command)
                .output()
                .context("Failed to execute command")?;

            if output.status.success() {
                term.write_line(&format!("{}", style("✅ 命令执行成功！").green().bold()))?;
                if cli.verbose && !output.stdout.is_empty() {
                    term.write_line("")?;
                    term.write_line(&String::from_utf8_lossy(&output.stdout))?;
                }
            } else {
                let error = String::from_utf8_lossy(&output.stderr);
                term.write_line(&format!(
                    "{} {}",
                    style("❌ 命令执行失败：").red().bold(),
                    style(&error).red()
                ))?;
            }
        }
    }

    Ok(())
}
