use anyhow::{Context, Result};
use console::{style, Emoji};

use crate::config::Config;
use crate::graph::neo4j::Neo4jClient;

static SEARCH: Emoji<'_, '_> = Emoji("🔍 ", "");
static GRAPH: Emoji<'_, '_> = Emoji("🔗 ", "");

pub async fn run(query: String, depth: usize) -> Result<()> {
    // Load configuration
    let config = Config::load().context("Failed to load configuration. Run 'rknowledge init' first.")?;

    // Connect to Neo4j
    let neo4j_client = Neo4jClient::new(&config.neo4j).await?;

    // Check if it's a Cypher query or natural language
    if query.to_lowercase().starts_with("cypher:") {
        let cypher = query.strip_prefix("cypher:").unwrap_or(&query).trim();
        let cypher = cypher.strip_prefix(':').unwrap_or(cypher).trim();
        println!("{}Executing Cypher query...", GRAPH);
        let results = neo4j_client.execute_cypher(cypher).await?;
        print_results(&results);
    } else {
        println!("{}Searching knowledge graph (depth: {})...", SEARCH, style(depth).cyan());
        println!("  Query: {}", style(&query).cyan());

        if depth > 1 {
            // Use variable-length path pattern for deeper traversal
            let results = neo4j_client.search_concepts_depth(&query, depth).await?;
            if results.is_empty() {
                println!();
                println!("{}", style("No matching concepts found.").yellow());
            } else {
                println!();
                println!("{}Related concepts (up to {} hops):", GRAPH, depth);
                for (concept, relations) in &results {
                    println!();
                    println!("  {}", style(concept).cyan().bold());
                    for (related, edge) in relations {
                        println!("    {} {} {}", style("→").dim(), style(edge).dim(), related);
                    }
                }
            }
        } else {
            let results = neo4j_client.search_concepts(&query).await?;
            if results.is_empty() {
                println!();
                println!("{}", style("No matching concepts found.").yellow());
            } else {
                println!();
                println!("{}Related concepts:", GRAPH);
                for (concept, relations) in &results {
                    println!();
                    println!("  {}", style(concept).cyan().bold());
                    for (related, edge) in relations {
                        println!("    {} {} {}", style("→").dim(), style(edge).dim(), related);
                    }
                }
            }
        }
    }

    Ok(())
}

fn print_results(results: &[serde_json::Value]) {
    if results.is_empty() {
        println!("{}", style("No results found.").yellow());
        return;
    }

    println!();
    println!("Results ({} rows):", style(results.len()).green().bold());
    println!();
    
    for (i, result) in results.iter().enumerate() {
        if let Some(obj) = result.as_object() {
            // Pretty print as table-like format
            let parts: Vec<String> = obj.iter()
                .filter(|(k, _)| *k != "raw")
                .map(|(k, v)| {
                    let val = match v {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Number(n) => n.to_string(),
                        _ => v.to_string(),
                    };
                    format!("{}: {}", style(k).dim(), style(val).cyan())
                })
                .collect();
            
            if !parts.is_empty() {
                println!("  {}. {}", i + 1, parts.join(" | "));
            } else if let Some(raw) = obj.get("raw") {
                println!("  {}. {}", i + 1, style(raw.to_string()).dim());
            }
        } else {
            println!("  {}. {}", i + 1, serde_json::to_string_pretty(result).unwrap_or_default());
        }
    }
}
