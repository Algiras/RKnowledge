use anyhow::Result;
use console::{Emoji, style};
use std::process::Command;
use std::time::Duration;

use crate::config::Config;

static DOCTOR: Emoji<'_, '_> = Emoji("🩺 ", "");
static PASS: Emoji<'_, '_> = Emoji("✅ ", "[OK] ");
static FAIL: Emoji<'_, '_> = Emoji("❌ ", "[!!] ");
static WARN: Emoji<'_, '_> = Emoji("⚠️  ", "[!] ");
static INFO: Emoji<'_, '_> = Emoji("ℹ️  ", "[i] ");
static ARROW: Emoji<'_, '_> = Emoji("   → ", "  -> ");

pub async fn run() -> Result<()> {
    println!();
    println!("{}", style(" RKnowledge Doctor ").bold().reverse());
    println!();
    println!("{}Running diagnostics...", DOCTOR);
    println!();

    let mut pass_count: u32 = 0;
    let mut fail_count: u32 = 0;
    let mut warn_count: u32 = 0;

    // ── 1. Binary version ────────────────────────────────────────────
    print_section("Binary");
    pass(
        &format!("rknowledge {}", env!("CARGO_PKG_VERSION")),
        &mut pass_count,
    );

    // ── 2. Config file ───────────────────────────────────────────────
    print_section("Configuration");

    let config_path = Config::config_path().ok();

    let config = if let Some(ref path) = config_path {
        if path.exists() {
            pass(
                &format!("Config found at {}", style(path.display()).dim()),
                &mut pass_count,
            );
            match Config::load() {
                Ok(c) => {
                    pass(
                        &format!("Config is valid TOML (provider: {})", c.default_provider),
                        &mut pass_count,
                    );
                    Some(c)
                }
                Err(e) => {
                    fail(&format!("Config parse error: {}", e), &mut fail_count);
                    hint("Run: rknowledge init --force");
                    None
                }
            }
        } else {
            fail("Config file not found", &mut fail_count);
            hint("Run: rknowledge init");
            None
        }
    } else {
        fail("Cannot determine config directory", &mut fail_count);
        None
    };

    // ── 3. Docker ────────────────────────────────────────────────────
    print_section("Docker");

    let docker_ok = check_command("docker", &["--version"]);
    if docker_ok {
        let version = get_command_output("docker", &["--version"]);
        pass(
            &format!("Docker installed ({})", version.trim()),
            &mut pass_count,
        );

        // Check Docker daemon running
        let daemon_ok = check_command("docker", &["info"]);
        if daemon_ok {
            pass("Docker daemon is running", &mut pass_count);
        } else {
            fail("Docker daemon is not running", &mut fail_count);
            hint("Start Docker Desktop or run: sudo systemctl start docker");
        }
    } else {
        fail("Docker not installed", &mut fail_count);
        hint("Install Docker: https://docs.docker.com/get-docker/");
    }

    // Check docker compose
    let compose_ok = check_command("docker", &["compose", "version"]);
    if compose_ok {
        let version = get_command_output("docker", &["compose", "version"]);
        pass(
            &format!("Docker Compose available ({})", version.trim()),
            &mut pass_count,
        );
    } else if docker_ok {
        warn("Docker Compose not available", &mut warn_count);
        hint("Docker Compose is bundled with Docker Desktop. Update Docker if missing.");
    }

    // ── 4. Neo4j container ───────────────────────────────────────────
    print_section("Neo4j");

    let container_running = check_container_running("rknowledge-neo4j");
    if container_running {
        pass("Neo4j container is running", &mut pass_count);

        let health = get_container_health("rknowledge-neo4j");
        match health.as_str() {
            "healthy" => pass("Container health: healthy", &mut pass_count),
            "starting" => warn(
                "Container health: starting (wait a moment)",
                &mut warn_count,
            ),
            "unhealthy" => {
                fail("Container health: unhealthy", &mut fail_count);
                hint("Check logs: docker logs rknowledge-neo4j");
            }
            other => {
                info(&format!("Container health: {}", other));
            }
        }
    } else {
        let container_exists = check_container_exists("rknowledge-neo4j");
        if container_exists {
            fail("Neo4j container exists but is stopped", &mut fail_count);
            hint("Start it: docker start rknowledge-neo4j");
        } else {
            fail("Neo4j container not found", &mut fail_count);
            hint("Run: rknowledge init");
        }
    }

    // ── 5. Neo4j connectivity ────────────────────────────────────────
    if let Some(ref config) = config {
        let neo4j_uri = &config.neo4j.uri;
        let bolt_host = neo4j_uri.replace("bolt://", "").replace("neo4j://", "");
        let parts: Vec<&str> = bolt_host.split(':').collect();
        let host = parts.first().unwrap_or(&"localhost");
        let port = parts.get(1).unwrap_or(&"7687");

        // TCP check on bolt port
        let bolt_reachable = check_tcp_port(host, port);
        if bolt_reachable {
            pass(
                &format!("Bolt port reachable ({}:{})", host, port),
                &mut pass_count,
            );

            // Try actual Neo4j connection
            match crate::graph::neo4j::Neo4jClient::new(&config.neo4j).await {
                Ok(client) => {
                    pass("Neo4j connection successful", &mut pass_count);

                    // Check graph data
                    match client.fetch_graph().await {
                        Ok((nodes, edges)) => {
                            if nodes.is_empty() {
                                info("Graph is empty (0 nodes, 0 edges)");
                                hint("Build a graph: rknowledge build ./your-docs/");
                            } else {
                                pass(
                                    &format!(
                                        "Graph has data: {} nodes, {} edges",
                                        style(nodes.len()).green().bold(),
                                        style(edges.len()).green().bold()
                                    ),
                                    &mut pass_count,
                                );
                            }
                        }
                        Err(e) => {
                            warn(&format!("Could not fetch graph: {}", e), &mut warn_count);
                        }
                    }
                }
                Err(e) => {
                    fail(&format!("Neo4j connection failed: {}", e), &mut fail_count);
                    hint(&format!(
                        "Check credentials in config: user={}, password=***",
                        config.neo4j.user
                    ));
                }
            }
        } else {
            fail(
                &format!("Bolt port not reachable ({}:{})", host, port),
                &mut fail_count,
            );
            hint("Is Neo4j running? Try: docker start rknowledge-neo4j");
        }

        // Check HTTP browser port
        let http_reachable = check_tcp_port(host, "7474");
        if http_reachable {
            pass(
                &format!("Neo4j Browser reachable ({}:7474)", host),
                &mut pass_count,
            );
        } else {
            warn(
                &format!("Neo4j Browser not reachable ({}:7474)", host),
                &mut warn_count,
            );
        }
    }

    // ── 6. LLM Providers ─────────────────────────────────────────────
    print_section("LLM Providers");

    if let Some(ref config) = config {
        // Ollama
        let ollama_url = config
            .providers
            .ollama
            .as_ref()
            .and_then(|p| p.base_url.as_deref())
            .unwrap_or("http://localhost:11434");

        match check_http_get(&format!("{}/api/tags", ollama_url)).await {
            Ok(200) => {
                pass(
                    &format!("Ollama is running at {}", ollama_url),
                    &mut pass_count,
                );
            }
            Ok(status) => {
                warn(
                    &format!("Ollama responded with status {} at {}", status, ollama_url),
                    &mut warn_count,
                );
            }
            Err(_) => {
                info(&format!("Ollama not reachable at {}", ollama_url));
                hint("Start Ollama: ollama serve");
            }
        }

        // Anthropic
        check_api_key_configured(
            "Anthropic",
            config.providers.anthropic.as_ref(),
            &["ANTHROPIC_API_KEY"],
            &mut pass_count,
            &mut warn_count,
        );

        // OpenAI
        check_api_key_configured(
            "OpenAI",
            config.providers.openai.as_ref(),
            &["OPENAI_API_KEY"],
            &mut pass_count,
            &mut warn_count,
        );

        // Google
        check_api_key_configured(
            "Google",
            config.providers.google.as_ref(),
            &["GOOGLE_API_KEY", "GEMINI_API_KEY"],
            &mut pass_count,
            &mut warn_count,
        );
    } else {
        warn("Skipping provider checks (no config)", &mut warn_count);
    }

    // ── 7. System info ───────────────────────────────────────────────
    print_section("System");

    info(&format!(
        "OS: {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));

    if let Ok(cwd) = std::env::current_dir() {
        info(&format!("Working directory: {}", cwd.display()));
    }

    if let Some(ref path) = config_path {
        info(&format!("Config path: {}", path.display()));
    }

    // ── Summary ──────────────────────────────────────────────────────
    println!();
    println!("{}", style("━".repeat(50)).dim());
    println!();

    let total = pass_count + fail_count + warn_count;
    print!(
        "  {} {} passed",
        style(pass_count).green().bold(),
        if pass_count == 1 { "check" } else { "checks" }
    );
    if warn_count > 0 {
        print!(
            ", {} {}",
            style(warn_count).yellow().bold(),
            if warn_count == 1 {
                "warning"
            } else {
                "warnings"
            }
        );
    }
    if fail_count > 0 {
        print!(
            ", {} {}",
            style(fail_count).red().bold(),
            if fail_count == 1 {
                "failure"
            } else {
                "failures"
            }
        );
    }
    println!(" ({}  total)", total);
    println!();

    if fail_count > 0 {
        println!(
            "  {}",
            style("Some checks failed. Fix the issues above and re-run:").red()
        );
        println!("    {} rknowledge doctor", style("$").dim());
    } else if warn_count > 0 {
        println!(
            "  {}",
            style("Everything essential works, but there are some warnings.").yellow()
        );
    } else {
        println!(
            "  {}",
            style("All checks passed! You're ready to go.")
                .green()
                .bold()
        );
    }
    println!();

    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────

fn print_section(name: &str) {
    println!("  {}", style(name).bold().underlined());
}

fn pass(msg: &str, count: &mut u32) {
    println!("  {}{}", PASS, msg);
    *count += 1;
}

fn fail(msg: &str, count: &mut u32) {
    println!("  {}{}", FAIL, style(msg).red());
    *count += 1;
}

fn warn(msg: &str, count: &mut u32) {
    println!("  {}{}", WARN, style(msg).yellow());
    *count += 1;
}

fn info(msg: &str) {
    println!("  {}{}", INFO, style(msg).dim());
}

fn hint(msg: &str) {
    println!("{}{}", ARROW, style(msg).dim());
}

fn check_command(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn get_command_output(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
}

fn check_container_running(name: &str) -> bool {
    let output = Command::new("docker")
        .args(["inspect", "-f", "{{.State.Running}}", name])
        .output();

    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim() == "true",
        _ => false,
    }
}

fn check_container_exists(name: &str) -> bool {
    Command::new("docker")
        .args(["inspect", name])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn get_container_health(name: &str) -> String {
    let output = Command::new("docker")
        .args([
            "inspect",
            "-f",
            "{{if .State.Health}}{{.State.Health.Status}}{{else}}no-healthcheck{{end}}",
            name,
        ])
        .output();

    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "unknown".to_string(),
    }
}

fn check_tcp_port(host: &str, port: &str) -> bool {
    let addr = format!("{}:{}", host, port);
    std::net::TcpStream::connect_timeout(
        &addr.parse().unwrap_or_else(|_| {
            format!("127.0.0.1:{}", port)
                .parse()
                .expect("fallback addr")
        }),
        Duration::from_secs(3),
    )
    .is_ok()
}

async fn check_http_get(url: &str) -> Result<u16, ()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|_| ())?;

    client
        .get(url)
        .send()
        .await
        .map(|r| r.status().as_u16())
        .map_err(|_| ())
}

fn check_api_key_configured(
    name: &str,
    provider: Option<&crate::config::ProviderConfig>,
    env_vars: &[&str],
    pass_count: &mut u32,
    warn_count: &mut u32,
) {
    // Check env vars first
    for env_var in env_vars {
        if let Ok(val) = std::env::var(env_var)
            && !val.is_empty()
        {
            pass(
                &format!("{} API key set via {}", name, style(*env_var).dim()),
                pass_count,
            );
            return;
        }
    }

    // Check config
    if let Some(p) = provider
        && !p.api_key.is_empty()
        && !p.api_key.starts_with("${")
    {
        pass(
            &format!("{} API key configured in config", name),
            pass_count,
        );
        return;
    }

    let vars_hint = env_vars.join(" or ");
    info(&format!(
        "{} not configured (set {} or run rknowledge auth)",
        name, vars_hint
    ));
    // Not a warning -- it's optional, only the active provider matters
    let _ = warn_count;
}
