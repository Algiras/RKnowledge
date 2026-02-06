use anyhow::{Context, Result};
use console::{style, Emoji};
use futures::stream::{self, StreamExt};
use indicatif::{HumanDuration, ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use walkdir::WalkDir;

use crate::cli::{LlmProvider, OutputDestination};
use crate::config::Config;
use crate::graph::builder::GraphBuilder;
use crate::graph::neo4j::Neo4jClient;
use crate::llm::LlmClient;
use crate::parser::DocumentParser;

static LOOKING_GLASS: Emoji<'_, '_> = Emoji("🔍 ", "");
static PAPER: Emoji<'_, '_> = Emoji("📄 ", "");
static BRAIN: Emoji<'_, '_> = Emoji("🧠 ", "");
static LINK: Emoji<'_, '_> = Emoji("🔗 ", "");
static SPARKLE: Emoji<'_, '_> = Emoji("✨ ", "");
static ROCKET: Emoji<'_, '_> = Emoji("🚀 ", "");
static CHECK: Emoji<'_, '_> = Emoji("✅ ", "[OK] ");
static DATABASE: Emoji<'_, '_> = Emoji("💾 ", "");

pub async fn run(
    path: PathBuf,
    provider: Option<LlmProvider>,
    model: Option<String>,
    output: OutputDestination,
    chunk_size: usize,
    chunk_overlap: usize,
    concurrency: usize,
    append: bool,
) -> Result<()> {
    let started = Instant::now();
    
    println!();
    println!("{}", style(" RKnowledge - Knowledge Graph Builder ").bold().reverse());
    println!();

    // Load configuration
    let config = Config::load().context("Failed to load configuration. Run 'rknowledge init' first.")?;

    // Determine provider and model
    let provider = provider.unwrap_or_else(|| {
        match config.default_provider.as_str() {
            "openai" => LlmProvider::OpenAI,
            "ollama" => LlmProvider::Ollama,
            "google" => LlmProvider::Google,
            _ => LlmProvider::Anthropic,
        }
    });

    let model = model.or(config.default_model.clone());
    let model_display = model.clone().unwrap_or_else(|| "default".to_string());

    println!("{}Provider: {}", BRAIN, style(&provider.to_string()).cyan().bold());
    println!("{}Model: {}", BRAIN, style(&model_display).cyan());
    println!("{}Source: {}", PAPER, style(path.display()).cyan());
    if concurrency > 1 {
        println!("{}Concurrency: {}", BRAIN, style(concurrency).cyan());
    }
    if append {
        println!("{}Mode: {}", DATABASE, style("append (merge with existing)").yellow());
    }
    println!();

    // Collect documents
    print!("{}Scanning for documents... ", LOOKING_GLASS);
    let documents = collect_documents(&path)?;
    println!("{}", style(format!("found {}", documents.len())).green().bold());

    if documents.is_empty() {
        println!();
        println!("{}", style("No supported documents found (.pdf, .md, .txt, .html)").yellow());
        return Ok(());
    }

    // Parse documents
    let parser = DocumentParser::new(chunk_size, chunk_overlap);
    let mut all_chunks = Vec::new();

    let pb = ProgressBar::new(documents.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(&format!("{}{{spinner:.green}} [{{elapsed_precise}}] {{bar:40.cyan/blue}} {{pos}}/{{len}} {{msg}}", PAPER))
            .unwrap()
            .progress_chars("━━╸━"),
    );
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    for doc_path in &documents {
        let filename = doc_path.file_name().unwrap_or_default().to_string_lossy();
        pb.set_message(format!("{}", style(filename).dim()));
        let chunks = parser.parse(doc_path)?;
        all_chunks.extend(chunks);
        pb.inc(1);
    }
    pb.finish_and_clear();
    println!("{}Parsed {} documents into {} chunks", 
        CHECK, 
        style(documents.len()).green().bold(),
        style(all_chunks.len()).green().bold()
    );

    // Create LLM client (wrapped in Arc for concurrent access)
    let llm_client = Arc::new(LlmClient::new(provider, &config, model.as_deref())?);

    // Build knowledge graph
    println!();
    println!("{}Extracting knowledge from text...", BRAIN);
    
    let mut builder = GraphBuilder::new();
    
    let pb = ProgressBar::new(all_chunks.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(&format!("{}{{spinner:.green}} [{{elapsed_precise}}] {{bar:40.magenta/blue}} {{pos}}/{{len}} | {{msg}}", LINK))
            .unwrap()
            .progress_chars("━━╸━"),
    );
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    let effective_concurrency = concurrency.max(1);

    // Process chunks concurrently
    let pb_ref = &pb;
    let client_ref = &llm_client;
    let results: Vec<(String, Vec<crate::llm::Relation>)> = stream::iter(all_chunks.iter().enumerate())
        .map(|(i, chunk)| {
            let text = chunk.text.clone();
            let chunk_id = chunk.id.clone();
            let client = Arc::clone(client_ref);
            async move {
                pb_ref.set_message(format!("chunk {}/{}", i + 1, pb_ref.length().unwrap_or(0)));
                let relations = client.extract_relations(&text).await.unwrap_or_default();
                pb_ref.inc(1);
                (chunk_id, relations)
            }
        })
        .buffer_unordered(effective_concurrency)
        .collect()
        .await;

    let mut total_relations = 0;
    for (chunk_id, relations) in results {
        total_relations += relations.len();
        builder.add_relations(relations, &chunk_id);
    }

    pb.finish_and_clear();
    println!("{}Extracted {} relations from {} chunks (concurrency: {})", 
        CHECK,
        style(total_relations).green().bold(),
        style(all_chunks.len()).green().bold(),
        style(effective_concurrency).cyan(),
    );

    // Calculate contextual proximity
    print!("{}Calculating contextual proximity... ", LINK);
    builder.calculate_contextual_proximity();
    println!("{}", style("done").green());

    let graph = builder.build();
    println!();
    println!("{}Graph Statistics:", SPARKLE);
    println!("  {} Nodes (concepts): {}", style("•").cyan(), style(graph.node_count()).green().bold());
    println!("  {} Edges (relations): {}", style("•").cyan(), style(graph.edge_count()).green().bold());

    // Output results
    println!();
    match output {
        OutputDestination::Neo4j => {
            let neo4j_client = Neo4jClient::new(&config.neo4j).await?;
            if append {
                print!("{}Merging into Neo4j... ", DATABASE);
                neo4j_client.merge_graph(&builder).await?;
            } else {
                print!("{}Storing in Neo4j... ", DATABASE);
                neo4j_client.store_graph(&builder).await?;
            }
            println!("{}", style("done").green());
            println!();
            println!("{}Query your graph:", ROCKET);
            println!("  {} rknowledge query \"your question\"", style("$").dim());
            println!("  {} rknowledge stats", style("$").dim());
            println!("  {} rknowledge viz", style("$").dim());
        }
        OutputDestination::Json => {
            let json_path = path.with_extension("kg.json");
            crate::export::export_json(&builder, &json_path)?;
            println!("{}Exported to {}", CHECK, style(json_path.display()).cyan());
        }
        OutputDestination::Csv => {
            let nodes_path = path.with_extension("nodes.csv");
            let edges_path = path.with_extension("edges.csv");
            crate::export::export_csv(&builder, &nodes_path, &edges_path)?;
            println!("{}Exported to:", CHECK);
            println!("  • {}", style(nodes_path.display()).cyan());
            println!("  • {}", style(edges_path.display()).cyan());
        }
    }

    println!();
    println!(
        "{}Done in {}",
        SPARKLE,
        style(HumanDuration(started.elapsed())).green().bold()
    );

    Ok(())
}

fn collect_documents(path: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut documents = Vec::new();

    if path.is_file() {
        if is_supported_file(path) {
            documents.push(path.clone());
        }
    } else if path.is_dir() {
        for entry in WalkDir::new(path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let entry_path = entry.path();
            if entry_path.is_file() && is_supported_file(entry_path) {
                documents.push(entry_path.to_path_buf());
            }
        }
    }

    Ok(documents)
}

fn is_supported_file(path: &std::path::Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some("pdf") | Some("txt") | Some("md") | Some("html") | Some("htm") => true,
        _ => false,
    }
}
