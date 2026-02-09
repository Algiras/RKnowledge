use anyhow::{Context, Result};
use console::{Emoji, style};
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

use crate::config::Config;
use crate::graph::analytics;
use crate::graph::builder::GraphBuilder;
use crate::graph::neo4j::Neo4jClient;

static CHART: Emoji<'_, '_> = Emoji("📊 ", "");
static TROPHY: Emoji<'_, '_> = Emoji("🏆 ", "");
static GRAPH: Emoji<'_, '_> = Emoji("🔗 ", "");
static DATABASE: Emoji<'_, '_> = Emoji("💾 ", "");

pub async fn run(tenant: Option<&str>) -> Result<()> {
    println!();
    println!(
        "{}",
        style(" RKnowledge - Graph Statistics ").bold().reverse()
    );
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

    spinner.set_message("Computing analytics...");

    // Rebuild petgraph from Neo4j data
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
    // Also add isolated nodes
    for node in &nodes {
        builder.ensure_node_public(&node.label);
    }

    let graph = builder.build();
    let stats = analytics::compute_stats(&graph);

    spinner.finish_and_clear();

    // Print stats
    println!("{}Graph Overview", CHART);
    println!();
    println!(
        "  {} Nodes:                {}",
        style("•").cyan(),
        style(stats.node_count).green().bold()
    );
    println!(
        "  {} Edges:                {}",
        style("•").cyan(),
        style(stats.edge_count).green().bold()
    );
    println!(
        "  {} Connected components: {}",
        style("•").cyan(),
        style(stats.connected_components).green().bold()
    );
    println!(
        "  {} Communities:          {}",
        style("•").cyan(),
        style(stats.community_count).green().bold()
    );
    println!(
        "  {} Density:              {}",
        style("•").cyan(),
        style(format!("{:.4}", stats.density)).green()
    );
    println!(
        "  {} Avg degree:           {}",
        style("•").cyan(),
        style(format!("{:.1}", stats.avg_degree)).green()
    );
    println!(
        "  {} Max degree:           {}",
        style("•").cyan(),
        style(stats.max_degree).green().bold()
    );

    if !stats.top_pagerank.is_empty() {
        println!();
        println!("{}Top Concepts by PageRank", TROPHY);
        println!();
        for (i, (label, score)) in stats.top_pagerank.iter().enumerate() {
            let bar_len = (score * 200.0).min(30.0) as usize;
            let bar = "█".repeat(bar_len);
            println!(
                "  {:>2}. {:<30} {} {:.4}",
                i + 1,
                style(label).cyan().bold(),
                style(&bar).magenta(),
                style(score).dim(),
            );
        }
    }

    if !stats.top_degree.is_empty() {
        println!();
        println!("{}Most Connected Concepts", GRAPH);
        println!();
        for (i, (label, degree)) in stats.top_degree.iter().enumerate() {
            let bar_len = (*degree).min(30);
            let bar = "█".repeat(bar_len);
            println!(
                "  {:>2}. {:<30} {} ({})",
                i + 1,
                style(label).cyan().bold(),
                style(&bar).blue(),
                style(degree).dim(),
            );
        }
    }

    // Entity type distribution
    let mut type_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for node in &nodes {
        let t = node.entity_type.as_deref().unwrap_or("untyped");
        *type_counts.entry(t.to_string()).or_insert(0) += 1;
    }
    if !type_counts.is_empty() {
        let mut type_vec: Vec<(String, usize)> = type_counts.into_iter().collect();
        type_vec.sort_by(|a, b| b.1.cmp(&a.1));

        println!();
        println!("{}Entity Types", CHART);
        println!();
        for (t, count) in &type_vec {
            let bar_len = (*count).min(30);
            let bar = "█".repeat(bar_len);
            println!(
                "  {:<25} {} ({})",
                style(t).yellow(),
                style(&bar).green(),
                style(count).dim(),
            );
        }
    }

    println!();

    Ok(())
}
