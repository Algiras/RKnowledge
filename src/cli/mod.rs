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
    #[command(long_about = "Configure API keys for LLM providers.\n\n\
        Supported providers: anthropic, openai, google, ollama.\n\
        All four providers support a custom base_url in the config file,\n\
        so you can point any provider at a proxy, gateway, or compatible service.\n\n\
        The OpenAI provider works with any OpenAI-compatible API (Groq, DeepSeek,\n\
        Mistral, Together AI, OpenRouter, Azure, LM Studio, vLLM, etc.).\n\n\
        The Anthropic provider works with any Anthropic Messages API-compatible\n\
        service (proxies, AWS Bedrock gateways, etc.).\n\n\
        Google accepts both GOOGLE_API_KEY and GEMINI_API_KEY environment variables.\n\n\
        Set base_url in ~/.config/rknowledge/config.toml for each provider.")]
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

        /// LLM provider (anthropic, openai, google, ollama). OpenAI-compatible APIs (Groq, DeepSeek, etc.) use 'openai' with a custom base_url in config
        #[arg(short, long, env = "RKNOWLEDGE_PROVIDER")]
        provider: Option<LlmProvider>,

        /// Model name (provider-specific, e.g. claude-sonnet-4-20250514, gpt-4o, gemini-2.0-flash, mistral)
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

    /// Check system health and diagnose common problems
    Doctor,
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
