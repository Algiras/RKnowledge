use anyhow::{Context, Result};
use console::{Emoji, style};
use std::fs;
use std::io::{self, Write};

use crate::cli::LlmProvider;
use crate::config::Config;

static KEY: Emoji<'_, '_> = Emoji("🔑 ", "");
static CHECK: Emoji<'_, '_> = Emoji("✅ ", "[OK] ");
static CROSS: Emoji<'_, '_> = Emoji("❌ ", "[X] ");
static ROBOT: Emoji<'_, '_> = Emoji("🤖 ", "");

pub async fn run(provider: Option<LlmProvider>, key: Option<String>, list: bool) -> Result<()> {
    println!();
    println!(
        "{}",
        style(" RKnowledge - Authentication ").bold().reverse()
    );
    println!();

    // List configured providers
    if list {
        return list_providers().await;
    }

    // If no provider specified, show interactive menu
    let provider = match provider {
        Some(p) => p,
        None => select_provider()?,
    };

    // Get API key
    let api_key = match key {
        Some(k) => k,
        None => prompt_api_key(&provider)?,
    };

    // Save to config
    save_api_key(provider, &api_key)?;

    println!();
    println!(
        "{}API key for {} configured successfully!",
        CHECK,
        style(provider.to_string()).cyan().bold()
    );

    Ok(())
}

async fn list_providers() -> Result<()> {
    println!("{}Configured LLM Providers", ROBOT);
    println!();

    let config = match Config::load() {
        Ok(c) => c,
        Err(_) => {
            println!(
                "{}",
                style("No configuration found. Run 'rknowledge init' first.").yellow()
            );
            return Ok(());
        }
    };

    // Check each provider
    let providers = [
        ("Anthropic", check_provider_status(&config, "anthropic")),
        ("OpenAI", check_provider_status(&config, "openai")),
        ("Google", check_provider_status(&config, "google")),
        ("Ollama", check_ollama_status(&config)),
    ];

    for (name, (configured, detail)) in providers {
        let status_icon = if configured { CHECK } else { CROSS };
        let status_text = if configured {
            style("Configured").green()
        } else {
            style("Not configured").red()
        };

        println!(
            "  {}{:<12} {} {}",
            status_icon,
            name,
            status_text,
            style(detail).dim()
        );
    }

    println!();
    println!("{}Set API keys with:", KEY);
    println!("  {} rknowledge auth --provider <name>", style("$").dim());
    println!();
    println!("Or set environment variables:");
    println!("  {} export ANTHROPIC_API_KEY=your-key", style("$").dim());
    println!("  {} export OPENAI_API_KEY=your-key", style("$").dim());
    println!("  {} export GOOGLE_API_KEY=your-key", style("$").dim());

    Ok(())
}

fn check_provider_status(config: &Config, provider: &str) -> (bool, String) {
    let env_var = match provider {
        "anthropic" => "ANTHROPIC_API_KEY",
        "openai" => "OPENAI_API_KEY",
        "google" => "GOOGLE_API_KEY",
        _ => "",
    };

    // Check environment variable first
    if !env_var.is_empty()
        && let Ok(val) = std::env::var(env_var)
        && !val.is_empty()
    {
        return (true, format!("(from {})", env_var));
    }

    // Check config file
    if let Some(provider_config) = config.get_provider(provider)
        && !provider_config.api_key.is_empty()
        && !provider_config.api_key.starts_with("${")
    {
        return (true, "(from config)".to_string());
    }

    (false, String::new())
}

fn check_ollama_status(config: &Config) -> (bool, String) {
    if let Some(provider_config) = config.get_provider("ollama") {
        let base_url = provider_config
            .base_url
            .as_deref()
            .unwrap_or("http://localhost:11434");

        // Try to check if Ollama is running (synchronous check)
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build();

        if let Ok(client) = client
            && let Ok(resp) = client.get(format!("{}/api/tags", base_url)).send()
            && resp.status().is_success()
        {
            return (true, format!("(running at {})", base_url));
        }

        return (false, format!("(not running at {})", base_url));
    }

    (false, String::new())
}

fn select_provider() -> Result<LlmProvider> {
    println!("Select LLM Provider:");
    println!();
    println!("  {} Anthropic (Claude)", style("1.").cyan());
    println!("  {} OpenAI (GPT-4)", style("2.").cyan());
    println!("  {} Google (Gemini)", style("3.").cyan());
    println!("  {} Ollama (Local - Free)", style("4.").cyan());
    println!();

    print!("{} Enter choice [1-4]: ", style("?").green().bold());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    match input.trim() {
        "1" => Ok(LlmProvider::Anthropic),
        "2" => Ok(LlmProvider::OpenAI),
        "3" => Ok(LlmProvider::Google),
        "4" => Ok(LlmProvider::Ollama),
        _ => {
            println!(
                "  {}",
                style("Invalid choice, defaulting to Ollama").yellow()
            );
            Ok(LlmProvider::Ollama)
        }
    }
}

fn prompt_api_key(provider: &LlmProvider) -> Result<String> {
    let prompt = match provider {
        LlmProvider::Anthropic => "Enter your Anthropic API key",
        LlmProvider::OpenAI => "Enter your OpenAI API key",
        LlmProvider::Google => "Enter your Google API key",
        LlmProvider::Ollama => {
            println!();
            println!("  {} Ollama doesn't require an API key.", style("ℹ").blue());
            println!(
                "  Make sure Ollama is running: {} ollama serve",
                style("$").dim()
            );
            return Ok(String::new());
        }
    };

    print!("{} {}: ", style("?").green().bold(), prompt);
    io::stdout().flush()?;

    let mut api_key = String::new();
    io::stdin().read_line(&mut api_key)?;
    let api_key = api_key.trim().to_string();

    if api_key.is_empty() {
        anyhow::bail!("API key cannot be empty");
    }

    // Basic validation
    match provider {
        LlmProvider::Anthropic => {
            if !api_key.starts_with("sk-ant-") {
                println!(
                    "  {}",
                    style("Warning: Anthropic API keys typically start with 'sk-ant-'").yellow()
                );
            }
        }
        LlmProvider::OpenAI => {
            if !api_key.starts_with("sk-") {
                println!(
                    "  {}",
                    style("Warning: OpenAI API keys typically start with 'sk-'").yellow()
                );
            }
        }
        _ => {}
    }

    Ok(api_key)
}

fn save_api_key(provider: LlmProvider, api_key: &str) -> Result<()> {
    let config_path = Config::config_path()?;

    if !config_path.exists() {
        anyhow::bail!("Configuration not found. Run 'rknowledge init' first.");
    }

    let content = fs::read_to_string(&config_path).context("Failed to read config file")?;

    let provider_section = match provider {
        LlmProvider::Anthropic => "[providers.anthropic]",
        LlmProvider::OpenAI => "[providers.openai]",
        LlmProvider::Google => "[providers.google]",
        LlmProvider::Ollama => "[providers.ollama]",
    };

    // Find and update the API key in the config
    let mut lines: Vec<String> = content.lines().map(String::from).collect();
    let mut in_section = false;
    let mut key_updated = false;

    for line in &mut lines {
        if line.starts_with('[') {
            in_section = line.contains(&provider_section[1..provider_section.len() - 1]);
        }

        if in_section && line.trim().starts_with("api_key") {
            *line = format!("api_key = \"{}\"", api_key);
            key_updated = true;
        }
    }

    if !key_updated {
        // Section might not exist or api_key line not found
        // Find the section and add the key
        let mut section_found = false;
        for (i, line) in lines.iter().enumerate() {
            if line.contains(&provider_section[1..provider_section.len() - 1]) {
                section_found = true;
                // Insert api_key after section header
                lines.insert(i + 1, format!("api_key = \"{}\"", api_key));
                break;
            }
        }

        if !section_found {
            // Add new section
            lines.push(String::new());
            lines.push(provider_section.to_string());
            lines.push(format!("api_key = \"{}\"", api_key));
        }
    }

    let new_content = lines.join("\n");
    fs::write(&config_path, new_content).context("Failed to write config file")?;

    Ok(())
}
