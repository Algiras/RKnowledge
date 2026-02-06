use anyhow::{Context, Result};
use console::{style, Emoji};
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

use crate::config::Config;
use crate::graph::neo4j::Neo4jClient;
use crate::graph::builder::GraphBuilder;
use crate::graph::community;

static PEOPLE: Emoji<'_, '_> = Emoji("👥 ", "");
static DATABASE: Emoji<'_, '_> = Emoji("💾 ", "");
static COMMUNITY: Emoji<'_, '_> = Emoji("🏘️  ", "");

pub async fn run() -> Result<()> {
    println!();
    println!("{}", style(" RKnowledge - Community Detection ").bold().reverse());
    println!();

    let config = Config::load().context("Failed to load configuration. Run 'rknowledge init' first.")?;

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

    spinner.set_message("Detecting communities...");

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
    let communities = community::label_propagation(&graph, 50);
    let summary = community::community_summary(&graph, &communities);

    spinner.finish_and_clear();

    if summary.is_empty() {
        println!("{}", style("No communities detected (empty graph).").yellow());
        return Ok(());
    }

    // Build a lookup: node_label -> entity_type
    let node_type_map: std::collections::HashMap<String, String> = nodes
        .iter()
        .map(|n| (n.label.to_lowercase(), n.entity_type.clone().unwrap_or_else(|| "untyped".to_string())))
        .collect();

    println!("{}Detected {} communities from {} nodes", 
        COMMUNITY,
        style(summary.len()).green().bold(),
        style(nodes.len()).cyan(),
    );
    println!();

    let colors = ["red", "blue", "green", "yellow", "magenta", "cyan"];

    for (i, (community_id, members)) in summary.iter().enumerate() {
        let color_idx = i % colors.len();
        let color = colors[color_idx];

        let header = format!("Community {} ({} members)", community_id, members.len());
        match color {
            "red" => println!("{}{}",  PEOPLE, style(&header).red().bold()),
            "blue" => println!("{}{}", PEOPLE, style(&header).blue().bold()),
            "green" => println!("{}{}", PEOPLE, style(&header).green().bold()),
            "yellow" => println!("{}{}", PEOPLE, style(&header).yellow().bold()),
            "magenta" => println!("{}{}", PEOPLE, style(&header).magenta().bold()),
            _ => println!("{}{}", PEOPLE, style(&header).cyan().bold()),
        }

        // Show entity type distribution for this community
        let mut type_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for member in members {
            let t = node_type_map.get(member).map(|s| s.as_str()).unwrap_or("untyped");
            *type_counts.entry(t).or_insert(0) += 1;
        }
        let mut type_list: Vec<(&str, usize)> = type_counts.into_iter().collect();
        type_list.sort_by(|a, b| b.1.cmp(&a.1));
        let type_str: Vec<String> = type_list.iter().map(|(t, c)| format!("{} {}", c, t)).collect();
        println!("    {} {}", style("types:").dim(), style(type_str.join(", ")).dim());

        // Show up to 20 members with their type
        let show_count = members.len().min(20);
        for member in &members[..show_count] {
            let t = node_type_map.get(member).map(|s| s.as_str()).unwrap_or("");
            if t.is_empty() || t == "untyped" {
                println!("    {} {}", style("•").dim(), style(member).cyan());
            } else {
                println!("    {} {} {}", style("•").dim(), style(member).cyan(), style(format!("[{}]", t)).dim());
            }
        }
        if members.len() > 20 {
            println!("    {} ... and {} more", style("•").dim(), style(members.len() - 20).dim());
        }
        println!();
    }

    Ok(())
}
