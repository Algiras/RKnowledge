use anyhow::{Context, Result};
use console::{Emoji, style};
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

use crate::config::Config;
use crate::graph::analytics;
use crate::graph::builder::GraphBuilder;
use crate::graph::neo4j::Neo4jClient;

static ROUTE: Emoji<'_, '_> = Emoji("🛤️  ", "");
static DATABASE: Emoji<'_, '_> = Emoji("💾 ", "");
#[allow(dead_code)]
static CHECK: Emoji<'_, '_> = Emoji("✅ ", "[OK] ");

pub async fn run(from: String, to: String, tenant: Option<&str>) -> Result<()> {
    println!();
    println!("{}", style(" RKnowledge - Shortest Path ").bold().reverse());
    println!();

    let config =
        Config::load().context("Failed to load configuration. Run 'rknowledge init' first.")?;

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template(&format!("{}{{spinner:.green}} {{msg}}", DATABASE))
            .unwrap(),
    );
    spinner.enable_steady_tick(Duration::from_millis(100));
    spinner.set_message("Fetching graph from Neo4j...");

    let neo4j_client = Neo4jClient::new(&config.neo4j).await?;
    let (nodes, edges) = neo4j_client.fetch_graph(tenant).await?;

    spinner.set_message("Finding shortest path...");

    // Rebuild petgraph
    let mut builder = GraphBuilder::new();
    for edge in &edges {
        builder.add_relations(
            vec![crate::llm::Relation {
                node_1: edge.source.clone(),
                node_1_type: None,
                node_2: edge.target.clone(),
                node_2_type: None,
                edge: edge.relation.clone(),
            }],
            "neo4j",
        );
    }
    for node in &nodes {
        builder.ensure_node_public(&node.label);
    }

    let graph = builder.build();

    spinner.finish_and_clear();

    println!(
        "{}Finding path: {} {} {}",
        ROUTE,
        style(&from).cyan().bold(),
        style("→").dim(),
        style(&to).cyan().bold()
    );
    println!();

    match analytics::shortest_path(&graph, &from, &to) {
        Some((cost, path)) => {
            println!("{}", style("Path found!").green().bold());
            println!();

            for (i, label) in path.iter().enumerate() {
                if i == 0 {
                    println!(
                        "  {} {}",
                        style("●").green().bold(),
                        style(label).cyan().bold()
                    );
                } else {
                    // Find the edge relation between previous and current
                    let prev = &path[i - 1];
                    let relation = edges
                        .iter()
                        .find(|e| {
                            (e.source.to_lowercase() == prev.to_lowercase()
                                && e.target.to_lowercase() == label.to_lowercase())
                                || (e.source.to_lowercase() == label.to_lowercase()
                                    && e.target.to_lowercase() == prev.to_lowercase())
                        })
                        .map(|e| e.relation.as_str())
                        .unwrap_or("related");

                    println!("  {} {}", style("│").dim(), style(relation).dim());
                    if i == path.len() - 1 {
                        println!(
                            "  {} {}",
                            style("●").green().bold(),
                            style(label).cyan().bold()
                        );
                    } else {
                        println!("  {} {}", style("◦").dim(), style(label).cyan());
                    }
                }
            }

            println!();
            println!(
                "  Path length: {} hops, cost: {:.3}",
                style(path.len() - 1).green().bold(),
                style(cost).dim()
            );
        }
        None => {
            println!(
                "{}",
                style("No path found between these concepts.").yellow()
            );
            println!();
            println!("  Make sure both concepts exist in the graph.");
            println!("  Try: {} rknowledge query \"{}\"", style("$").dim(), &from);
        }
    }

    println!();
    Ok(())
}
