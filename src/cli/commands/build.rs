use anyhow::{Context, Result};
use console::{Emoji, style};
use indicatif::{HumanDuration, ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::time::Instant;
use walkdir::WalkDir;

use crate::cli::{LlmProvider, OutputDestination};
use crate::config::Config;
use crate::graph::builder::GraphBuilder;
use crate::graph::neo4j::Neo4jClient;
use crate::llm::batch_processor::{BatchProcessor, DocumentSelector};
use crate::llm::LlmClient;
use crate::parser::DocumentParser;
use crate::parser::ModelContextLimits;

static LOOKING_GLASS: Emoji<'_, '_> = Emoji("🔍 ", "");
static PAPER: Emoji<'_, '_> = Emoji("📄 ", "");
static BRAIN: Emoji<'_, '_> = Emoji("🧠 ", "");
static LINK: Emoji<'_, '_> = Emoji("🔗 ", "");
static SPARKLE: Emoji<'_, '_> = Emoji("✨ ", "");
static ROCKET: Emoji<'_, '_> = Emoji("🚀 ", "");
static CHECK: Emoji<'_, '_> = Emoji("✅ ", "[OK] ");
static DATABASE: Emoji<'_, '_> = Emoji("💾 ", "");

#[allow(clippy::too_many_arguments)]
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
    println!(
        "{}",
        style(" RKnowledge - Knowledge Graph Builder ")
            .bold()
            .reverse()
    );
    println!();

    // Load configuration
    let config =
        Config::load().context("Failed to load configuration. Run 'rknowledge init' first.")?;

    // Determine provider and model
    let provider = provider.unwrap_or(match config.default_provider.as_str() {
        "openai" => LlmProvider::OpenAI,
        "ollama" => LlmProvider::Ollama,
        "google" => LlmProvider::Google,
        _ => LlmProvider::Anthropic,
    });

    let model = model.or(config.default_model.clone());
    let model_display = model.clone().unwrap_or_else(|| "default".to_string());

    // Auto-detect if we should use adaptive processing for local models
    let use_adaptive = matches!(provider, LlmProvider::Ollama);
    let detected_context = model.as_ref()
        .map(|m| ModelContextLimits::get_context_size(m))
        .unwrap_or(4096);

    println!(
        "{}Provider: {}",
        BRAIN,
        style(&provider.to_string()).cyan().bold()
    );
    println!("{}Model: {}", BRAIN, style(&model_display).cyan());
    println!("{}Source: {}", PAPER, style(path.display()).cyan());
    if use_adaptive {
        println!("{}Adaptive chunking: {} ({} tokens)", BRAIN, style("enabled").green(), style(detected_context).cyan());
    }
    if concurrency > 1 {
        println!("{}Concurrency: {}", BRAIN, style(concurrency).cyan());
    }
    if append {
        println!(
            "{}Mode: {}",
            DATABASE,
            style("append (merge with existing)").yellow()
        );
    }
    println!();

    // Collect documents
    print!("{}Scanning for documents... ", LOOKING_GLASS);
    let documents = collect_documents(&path)?;
    println!(
        "{}",
        style(format!("found {}", documents.len())).green().bold()
    );

    if documents.is_empty() {
        println!();
        println!(
            "{}",
            style("No supported documents found (.pdf, .md, .txt, .html)").yellow()
        );
        return Ok(());
    }

    // Parse documents
    let parser = DocumentParser::new(chunk_size, chunk_overlap);
    let mut doc_contents: Vec<(String, String)> = Vec::new(); // (source, text)

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
        // Combine chunks back into full document text for batch processing
        let full_text: String = chunks.iter().map(|c| c.text.clone()).collect::<Vec<_>>().join("\n\n");
        doc_contents.push((doc_path.to_string_lossy().to_string(), full_text));
        pb.inc(1);
    }
    pb.finish_and_clear();

    // Smart document selection for large codebases
    let selected_docs = if doc_contents.len() > 100 {
        println!("{}Large codebase detected ({} docs). Selecting representative documents...", BRAIN, doc_contents.len());
        DocumentSelector::select_representative_docs(&doc_contents, 5)
    } else {
        doc_contents
    };

    println!(
        "{}Parsed {} documents ({} selected for processing)",
        CHECK,
        style(documents.len()).green().bold(),
        style(selected_docs.len()).green().bold()
    );

    // Create LLM client
    let llm_client = LlmClient::new(provider, &config, model.as_deref())?;

    // Build knowledge graph
    println!();
    println!("{}Extracting knowledge from text...", BRAIN);

    let mut builder = GraphBuilder::new();

    // Use batch processor for efficient large codebase processing
    let batch_size = if use_adaptive { 3 } else { 5 }; // Smaller batches for local models
    let mut processor = BatchProcessor::new(
        llm_client,
        &model_display,
        concurrency.max(1),
        batch_size,
    );

    // Enable progress persistence
    let output_json_path = path.with_extension("kg.json");
    processor = processor.with_progress_persistence(&output_json_path);
    processor.load_progress().await?;

    // Process documents in batches
    let relations_result = processor.process_documents(selected_docs).await?;

    // Add all relations to builder
    let mut total_relations = 0;
    for relation in relations_result {
        total_relations += 1;
        builder.add_relations(vec![relation], "document");
    }
    let stats = processor.get_stats();
    println!(
        "{}Extracted {} relations from {} documents (batch size: {}, concurrency: {})",
        CHECK,
        style(total_relations).green().bold(),
        style(stats.total_documents).green().bold(),
        style(batch_size).cyan(),
        style(concurrency.max(1)).cyan(),
    );

    // Calculate contextual proximity
    print!("{}Calculating contextual proximity... ", LINK);
    builder.calculate_contextual_proximity();
    println!("{}", style("done").green());

    let graph = builder.build();
    println!();
    println!("{}Graph Statistics:", SPARKLE);
    println!(
        "  {} Nodes (concepts): {}",
        style("•").cyan(),
        style(graph.node_count()).green().bold()
    );
    println!(
        "  {} Edges (relations): {}",
        style("•").cyan(),
        style(graph.edge_count()).green().bold()
    );

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
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("pdf") | Some("txt") | Some("md") | Some("html") | Some("htm")
    )
}
