pub mod commands;

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "rknowledge")]
#[command(author = "RKnowledge Team")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "High-performance knowledge graph extraction CLI using LLMs", long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize configuration and optionally start Neo4j
    Init {
        /// Force overwrite existing configuration
        #[arg(short, long, default_value = "false")]
        force: bool,
    },

    /// Configure API keys for LLM providers
    Auth {
        /// Provider to configure (anthropic, openai, google, ollama)
        #[arg(short, long)]
        provider: Option<LlmProvider>,

        /// Set API key directly (alternative to interactive prompt)
        #[arg(short, long)]
        key: Option<String>,

        /// List configured providers and their status
        #[arg(long, default_value = "false")]
        list: bool,
    },

    /// Process documents and build knowledge graph
    Build {
        /// Path to document(s) or directory
        #[arg(required = true)]
        path: PathBuf,

        /// LLM provider to use
        #[arg(short, long, env = "RKNOWLEDGE_PROVIDER")]
        provider: Option<LlmProvider>,

        /// Model to use (provider-specific)
        #[arg(short, long, env = "RKNOWLEDGE_MODEL")]
        model: Option<String>,

        /// Output destination
        #[arg(short, long, default_value = "neo4j")]
        output: OutputDestination,

        /// Chunk size for text splitting
        #[arg(long, default_value = "1500")]
        chunk_size: usize,

        /// Chunk overlap for text splitting
        #[arg(long, default_value = "150")]
        chunk_overlap: usize,

        /// Number of concurrent LLM requests
        #[arg(short = 'j', long, default_value = "4")]
        concurrency: usize,

        /// Append to existing graph instead of replacing
        #[arg(long, default_value = "false")]
        append: bool,
    },

    /// Export knowledge graph to various formats
    Export {
        /// Export format
        #[arg(short, long, default_value = "json")]
        format: ExportFormat,

        /// Output file path
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Query the knowledge graph
    Query {
        /// Natural language query or Cypher query (prefix with 'cypher:')
        query: String,

        /// Traversal depth for matching concepts (hops from match)
        #[arg(short, long, default_value = "1")]
        depth: usize,
    },

    /// Find shortest path between two concepts
    Path {
        /// Source concept
        from: String,

        /// Target concept
        to: String,
    },

    /// Show graph statistics and analytics
    Stats,

    /// List detected communities and their members
    Communities,

    /// Start visualization server
    Viz {
        /// Port to serve visualization on
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum LlmProvider {
    #[default]
    Anthropic,
    OpenAI,
    Ollama,
    Google,
}

impl std::fmt::Display for LlmProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmProvider::Anthropic => write!(f, "anthropic"),
            LlmProvider::OpenAI => write!(f, "openai"),
            LlmProvider::Ollama => write!(f, "ollama"),
            LlmProvider::Google => write!(f, "google"),
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum OutputDestination {
    #[default]
    Neo4j,
    Json,
    Csv,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ExportFormat {
    Json,
    Csv,
    Graphml,
    Cypher,
}

impl std::fmt::Display for ExportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportFormat::Json => write!(f, "json"),
            ExportFormat::Csv => write!(f, "csv"),
            ExportFormat::Graphml => write!(f, "graphml"),
            ExportFormat::Cypher => write!(f, "cypher"),
        }
    }
}
