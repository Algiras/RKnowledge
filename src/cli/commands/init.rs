use anyhow::{Context, Result};
use console::{style, Emoji};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::process::Command;
use std::time::Duration;

use crate::config::{Config, Neo4jConfig, ProviderConfig, ProvidersConfig};

static ROCKET: Emoji<'_, '_> = Emoji("🚀 ", "");
static GEAR: Emoji<'_, '_> = Emoji("⚙️  ", "");
static CHECK: Emoji<'_, '_> = Emoji("✅ ", "[OK] ");
static WARN: Emoji<'_, '_> = Emoji("⚠️  ", "[!] ");
static DATABASE: Emoji<'_, '_> = Emoji("💾 ", "");
static KEY: Emoji<'_, '_> = Emoji("🔑 ", "");

pub async fn run(force: bool) -> Result<()> {
    println!();
    println!("{}", style(" RKnowledge - Initialization ").bold().reverse());
    println!();

    let config_dir = Config::config_dir()?;
    let config_path = config_dir.join("config.toml");

    // Check if config already exists
    if config_path.exists() && !force {
        println!(
            "{}Configuration already exists at {}",
            WARN,
            style(config_path.display()).cyan()
        );
        println!("  Use {} to overwrite", style("--force").yellow());
        return Ok(());
    }

    // Create config directory
    fs::create_dir_all(&config_dir).context("Failed to create config directory")?;

    // Create default configuration
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template(&format!("{}{{spinner:.green}} {{msg}}", GEAR))
            .unwrap(),
    );
    spinner.enable_steady_tick(Duration::from_millis(100));
    spinner.set_message("Creating configuration...");

    let default_config = Config {
        default_provider: "ollama".to_string(),
        default_model: Some("mistral".to_string()),
        chunk_size: 1500,
        chunk_overlap: 150,
        providers: ProvidersConfig {
            anthropic: Some(ProviderConfig {
                api_key: "${ANTHROPIC_API_KEY}".to_string(),
                base_url: None,
                model: Some("claude-sonnet-4-20250514".to_string()),
            }),
            openai: Some(ProviderConfig {
                api_key: "${OPENAI_API_KEY}".to_string(),
                base_url: None,
                model: Some("gpt-4o".to_string()),
            }),
            ollama: Some(ProviderConfig {
                api_key: String::new(),
                base_url: Some("http://localhost:11434".to_string()),
                model: Some("mistral".to_string()),
            }),
            google: Some(ProviderConfig {
                api_key: "${GOOGLE_API_KEY}".to_string(),
                base_url: None,
                model: Some("gemini-pro".to_string()),
            }),
        },
        neo4j: Neo4jConfig {
            uri: "bolt://localhost:7687".to_string(),
            user: "neo4j".to_string(),
            password: "rknowledge".to_string(),
            database: Some("neo4j".to_string()),
        },
    };

    // Write config file
    let config_content = toml::to_string_pretty(&default_config)?;
    fs::write(&config_path, config_content).context("Failed to write config file")?;
    spinner.finish_and_clear();

    println!(
        "{}Created configuration at {}",
        CHECK,
        style(config_path.display()).cyan()
    );

    // Create docker-compose.yml in config directory
    let docker_compose_path = config_dir.join("docker-compose.yml");
    let docker_compose_content = include_str!("../../../assets/docker-compose.yml");
    fs::write(&docker_compose_path, docker_compose_content)
        .context("Failed to write docker-compose.yml")?;

    println!(
        "{}Created docker-compose.yml",
        CHECK,
    );

    // Check if Docker is available
    let docker_available = Command::new("docker")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    println!();
    if docker_available {
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .template(&format!("{}{{spinner:.green}} {{msg}}", DATABASE))
                .unwrap(),
        );
        spinner.enable_steady_tick(Duration::from_millis(100));
        spinner.set_message("Starting Neo4j with Docker...");

        let status = Command::new("docker")
            .args(["compose", "-f", docker_compose_path.to_str().unwrap(), "up", "-d"])
            .output();

        spinner.finish_and_clear();

        match status {
            Ok(output) if output.status.success() => {
                println!("{}Neo4j started successfully", CHECK);
                println!();
                println!("  {} Neo4j Browser: {}", style("→").cyan(), style("http://localhost:7474").blue().underlined());
                println!("  {} Credentials: {} / {}", style("→").cyan(), style("neo4j").green(), style("rknowledge").green());
            }
            _ => {
                println!(
                    "{}Failed to start Neo4j. Start manually with:",
                    WARN,
                );
                println!(
                    "  {} docker compose -f {} up -d",
                    style("$").dim(),
                    docker_compose_path.display()
                );
            }
        }
    } else {
        println!(
            "{}Docker not found. Install Docker to use Neo4j backend.",
            WARN,
        );
        println!(
            "  Once installed, run: {} docker compose -f {} up -d",
            style("$").dim(),
            docker_compose_path.display()
        );
    }

    println!();
    println!("{}", style("━".repeat(50)).dim());
    println!();
    println!("{}Next steps:", ROCKET);
    println!();
    println!("  {}Configure your LLM provider:", KEY);
    println!("    {} rknowledge auth", style("$").dim());
    println!();
    println!("  {}Build your first knowledge graph:", ROCKET);
    println!("    {} rknowledge build ./your-documents/", style("$").dim());
    println!();

    Ok(())
}
