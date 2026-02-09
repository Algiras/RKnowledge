mod cli;
mod config;
mod error;
mod export;
mod graph;
mod llm;
mod parser;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use crate::cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing - only show warnings by default, use RUST_LOG=info for more detail
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")))
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Init { force } => {
            cli::commands::init::run(force).await?;
        }
        Commands::Auth {
            provider,
            key,
            list,
        } => {
            cli::commands::auth::run(provider, key, list).await?;
        }
        Commands::Build {
            path,
            provider,
            model,
            output,
            chunk_size,
            chunk_overlap,
            concurrency,
            append,
            domain,
            context,
            context_file,
        } => {
            cli::commands::build::run(
                path,
                provider,
                model,
                output,
                chunk_size,
                chunk_overlap,
                concurrency,
                append,
                domain,
                context,
                context_file,
                cli.tenant.as_deref(),
            )
            .await?;
        }
        Commands::Export { format, output } => {
            cli::commands::export::run(format, output, cli.tenant.as_deref()).await?;
        }
        Commands::Query { query, depth } => {
            cli::commands::query::run(query, depth).await?;
        }
        Commands::Path { from, to } => {
            cli::commands::path::run(from, to, cli.tenant.as_deref()).await?;
        }
        Commands::Stats => {
            cli::commands::stats::run(cli.tenant.as_deref()).await?;
        }
        Commands::Communities => {
            cli::commands::communities::run(cli.tenant.as_deref()).await?;
        }
        Commands::Viz { port } => {
            cli::commands::viz::run(port, cli.tenant.as_deref()).await?;
        }
        Commands::Doctor => {
            cli::commands::doctor::run().await?;
        }
        Commands::Add {
            node1,
            node2,
            relation,
            type1,
            type2,
            interactive,
            from_file,
        } => {
            cli::commands::add::run(
                node1,
                node2,
                relation,
                type1,
                type2,
                interactive,
                from_file,
                cli.tenant.as_deref(),
            )
            .await?;
        }
    }

    Ok(())
}
