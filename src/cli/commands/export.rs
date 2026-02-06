use anyhow::{Context, Result};
use console::{style, Emoji};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::time::Duration;

use crate::cli::ExportFormat;
use crate::config::Config;
use crate::graph::neo4j::Neo4jClient;

static DATABASE: Emoji<'_, '_> = Emoji("💾 ", "");
static CHECK: Emoji<'_, '_> = Emoji("✅ ", "[OK] ");
static FILE: Emoji<'_, '_> = Emoji("📁 ", "");

pub async fn run(format: ExportFormat, output: PathBuf) -> Result<()> {
    println!();
    println!("{}", style(" RKnowledge - Export ").bold().reverse());
    println!();

    // Load configuration
    let config = Config::load().context("Failed to load configuration. Run 'rknowledge init' first.")?;

    // Connect to Neo4j
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template(&format!("{}{{spinner:.green}} {{msg}}", DATABASE))
            .unwrap(),
    );
    spinner.enable_steady_tick(Duration::from_millis(100));
    spinner.set_message("Fetching graph from Neo4j...");

    let neo4j_client = Neo4jClient::new(&config.neo4j).await?;
    let (nodes, edges) = neo4j_client.fetch_graph().await?;

    spinner.finish_and_clear();
    println!(
        "{}Loaded {} nodes, {} edges",
        CHECK,
        style(nodes.len()).green().bold(),
        style(edges.len()).green().bold()
    );

    // Export based on format
    let format_name = match format {
        ExportFormat::Json => "JSON",
        ExportFormat::Csv => "CSV",
        ExportFormat::Graphml => "GraphML",
        ExportFormat::Cypher => "Cypher",
    };

    print!("{}Exporting to {}... ", FILE, style(format_name).cyan());

    match format {
        ExportFormat::Json => {
            crate::export::export_json_from_data(&nodes, &edges, &output)?;
            println!("{}", style("done").green());
            println!();
            println!("  {} {}", style("→").dim(), style(output.display()).cyan().underlined());
        }
        ExportFormat::Csv => {
            let nodes_path = output.with_extension("nodes.csv");
            let edges_path = output.with_extension("edges.csv");
            crate::export::export_csv_from_data(&nodes, &edges, &nodes_path, &edges_path)?;
            println!("{}", style("done").green());
            println!();
            println!("  {} {}", style("→").dim(), style(nodes_path.display()).cyan().underlined());
            println!("  {} {}", style("→").dim(), style(edges_path.display()).cyan().underlined());
        }
        ExportFormat::Graphml => {
            crate::export::export_graphml(&nodes, &edges, &output)?;
            println!("{}", style("done").green());
            println!();
            println!("  {} {}", style("→").dim(), style(output.display()).cyan().underlined());
        }
        ExportFormat::Cypher => {
            crate::export::export_cypher(&nodes, &edges, &output)?;
            println!("{}", style("done").green());
            println!();
            println!("  {} {}", style("→").dim(), style(output.display()).cyan().underlined());
        }
    }

    println!();

    Ok(())
}
