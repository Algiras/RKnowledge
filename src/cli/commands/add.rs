use anyhow::{Context, Result};
use console::{Emoji, style};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use crate::config::Config;
use crate::graph::builder::GraphBuilder;
use crate::graph::neo4j::Neo4jClient;
use crate::llm::Relation;

static CHECK: Emoji<'_, '_> = Emoji("✅ ", "[OK] ");
static LINK: Emoji<'_, '_> = Emoji("🔗 ", "");
static PLUS: Emoji<'_, '_> = Emoji("➕ ", "+ ");

/// Batch import format for --from-file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationImport {
    pub node1: String,
    pub node2: String,
    pub relation: String,
    #[serde(default)]
    pub type1: Option<String>,
    #[serde(default)]
    pub type2: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub async fn run(
    node1: Option<String>,
    node2: Option<String>,
    relation: Option<String>,
    type1: Option<String>,
    type2: Option<String>,
    interactive: bool,
    from_file: Option<PathBuf>,
    tenant: Option<&str>,
) -> Result<()> {
    let config =
        Config::load().context("Failed to load configuration. Run 'rknowledge init' first.")?;

    println!();
    println!(
        "{}",
        style(" RKnowledge - Manual Relation Insert ")
            .bold()
            .reverse()
    );
    println!();

    // Collect relations to add
    let mut relations_to_add: Vec<RelationImport> = Vec::new();

    if let Some(file_path) = from_file {
        // Batch import from file
        println!(
            "{}Importing from {}...",
            LINK,
            style(file_path.display()).cyan()
        );
        let content = std::fs::read_to_string(&file_path)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

        relations_to_add = serde_json::from_str(&content).with_context(
            || "Failed to parse JSON. Expected array of {node1, node2, relation, type1?, type2?}",
        )?;

        println!(
            "{}Found {} relations to import",
            CHECK,
            style(relations_to_add.len()).green().bold()
        );
    } else if interactive {
        // Interactive mode
        println!(
            "{}Enter relations interactively (empty node1 to finish):",
            LINK
        );
        println!();

        let stdin = io::stdin();
        let mut stdout = io::stdout();

        loop {
            print!("  Node 1: ");
            stdout.flush()?;
            let mut node1_input = String::new();
            stdin.lock().read_line(&mut node1_input)?;
            let node1_input = node1_input.trim().to_string();

            if node1_input.is_empty() {
                break;
            }

            print!("  Node 2: ");
            stdout.flush()?;
            let mut node2_input = String::new();
            stdin.lock().read_line(&mut node2_input)?;
            let node2_input = node2_input.trim().to_string();

            if node2_input.is_empty() {
                println!(
                    "  {} Node 2 cannot be empty, skipping...",
                    style("⚠").yellow()
                );
                continue;
            }

            print!("  Relation: ");
            stdout.flush()?;
            let mut relation_input = String::new();
            stdin.lock().read_line(&mut relation_input)?;
            let relation_input = relation_input.trim().to_string();

            if relation_input.is_empty() {
                println!(
                    "  {} Relation cannot be empty, skipping...",
                    style("⚠").yellow()
                );
                continue;
            }

            print!("  Type for '{}' (optional): ", node1_input);
            stdout.flush()?;
            let mut type1_input = String::new();
            stdin.lock().read_line(&mut type1_input)?;
            let type1_input = type1_input.trim().to_string();

            print!("  Type for '{}' (optional): ", node2_input);
            stdout.flush()?;
            let mut type2_input = String::new();
            stdin.lock().read_line(&mut type2_input)?;
            let type2_input = type2_input.trim().to_string();

            relations_to_add.push(RelationImport {
                node1: node1_input,
                node2: node2_input,
                relation: relation_input,
                type1: if type1_input.is_empty() {
                    None
                } else {
                    Some(type1_input)
                },
                type2: if type2_input.is_empty() {
                    None
                } else {
                    Some(type2_input)
                },
            });

            println!("  {} Added!", style("✓").green());
            println!();
        }
    } else {
        // Single relation from arguments
        let node1 = node1.context("Node 1 is required (or use --interactive or --from-file)")?;
        let node2 = node2.context("Node 2 is required")?;
        let relation = relation.context("Relation is required (--relation \"description\")")?;

        relations_to_add.push(RelationImport {
            node1,
            node2,
            relation,
            type1,
            type2,
        });
    }

    if relations_to_add.is_empty() {
        println!("{}", style("No relations to add.").yellow());
        return Ok(());
    }

    // Build graph with relations
    let mut builder = GraphBuilder::new();
    if let Some(t) = tenant {
        builder.set_tenant(t);
    }

    for import in &relations_to_add {
        let rel = Relation {
            node_1: import.node1.clone(),
            node_1_type: import.type1.clone(),
            node_2: import.node2.clone(),
            node_2_type: import.type2.clone(),
            edge: import.relation.clone(),
        };
        builder.add_relations(vec![rel], "manual");
    }

    // Store in Neo4j
    let neo4j_client = Neo4jClient::new(&config.neo4j).await?;

    print!("{}Merging into Neo4j... ", LINK);
    neo4j_client.merge_graph(&builder).await?;
    println!("{}", style("done").green());

    // Summary
    println!();
    for import in &relations_to_add {
        println!(
            "  {} {} {} {} {} {}",
            PLUS,
            style(&import.node1).cyan().bold(),
            style("→").dim(),
            style(&import.relation).yellow(),
            style("→").dim(),
            style(&import.node2).cyan().bold()
        );
    }

    println!();
    println!(
        "{}Added {} relation(s) to the knowledge graph",
        CHECK,
        style(relations_to_add.len()).green().bold()
    );

    Ok(())
}
