# Contributing to RKnowledge

Thank you for considering contributing! This document provides guidelines and
information for contributors — human and AI alike.

## Getting Started

```bash
# Clone
git clone https://github.com/algimantask/rknowledge.git
cd rknowledge

# Build
cargo build

# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run -- build ./demo_data --provider ollama --model ministral-3:8b --output json
```

### Prerequisites

- **Rust** (latest stable, edition 2024)
- **Docker** (for Neo4j integration tests)
- **Ollama** (optional, for testing local LLM provider)

## Project Structure

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full module map and data flow.

## Development Workflow

### Branch Naming

- `feat/description` — new features
- `fix/description` — bug fixes
- `refactor/description` — code improvements
- `docs/description` — documentation only

### Commit Messages

Follow conventional commits:

```
feat: add concurrent LLM chunk processing
fix: handle empty LLM responses gracefully
refactor: extract JSON parsing into shared utility
docs: update architecture diagram
```

### Code Style

- Run `cargo fmt` before committing
- Run `cargo clippy -- -D warnings` — no warnings allowed
- Prefer `anyhow::Result` for application errors
- Use `console::style()` for TUI output (not `colored`)
- Use `tracing::info!()` for debug logging (not `println!` in library code)

## Adding a New LLM Provider

1. Create `src/llm/your_provider.rs`
2. Implement `LlmProviderTrait`:
   ```rust
   #[async_trait]
   impl LlmProviderTrait for YourProvider {
       async fn extract_relations(&self, text: &str) -> Result<Vec<Relation>> {
           // Make API call
           // Parse JSON response
           // Return Vec<Relation>
       }
       fn name(&self) -> &'static str { "your-provider" }
   }
   ```
3. Add module to `src/llm/mod.rs`
4. Add variant to `LlmProvider` enum in `src/cli/mod.rs`
5. Add construction in `LlmClient::new()` in `src/llm/mod.rs`
6. Add config section to `ProvidersConfig` in `src/config.rs`
7. Update `auth.rs` to support the new provider

## Adding a New Document Format

1. Create `src/parser/your_format.rs` with:
   ```rust
   pub fn extract_text(path: &Path) -> Result<String> { ... }
   ```
2. Add module to `src/parser/mod.rs`
3. Add extension match in `DocumentParser::parse()`
4. Add appropriate dependency to `Cargo.toml`

## Adding a New Export Format

1. Add function to `src/export.rs`:
   ```rust
   pub fn export_your_format(nodes: &[GraphNode], edges: &[GraphEdge], path: &Path) -> Result<()> { ... }
   ```
2. Add variant to `ExportFormat` in `src/cli/mod.rs`
3. Wire it in `src/cli/commands/export.rs`

## Testing

### Unit Tests

Each module has tests at the bottom in `#[cfg(test)]` blocks:

```bash
cargo test                    # all tests
cargo test graph::builder     # specific module
cargo test -- --nocapture     # see println output
```

### Integration Tests

Integration tests require a running Neo4j instance:

```bash
# Start Neo4j
docker compose -f assets/docker-compose.yml up -d

# Run integration tests
cargo test -- --ignored
```

### Manual Testing

```bash
# Full pipeline test with Ollama
rknowledge init --force
rknowledge build ./demo_data --provider ollama --model ministral-3:8b
rknowledge query "artificial intelligence"
rknowledge export --format json --output test.json
rknowledge viz
```

## Priority Improvements

These are the most impactful improvements to work on (see ARCHITECTURE.md for more):

1. **Concurrent LLM calls** — use `futures::stream::buffered()` to process multiple
   chunks in parallel
2. **Neo4j batch operations** — use `UNWIND` for inserting nodes/edges in bulk
3. **Incremental graph updates** — `MERGE` instead of `CREATE` to avoid duplicates
4. **Community detection** — implement Louvain algorithm using `petgraph`
5. **Shell completions** — clap can generate these automatically
6. **Better error types** — replace bare `anyhow` with typed errors for key failure modes
7. **Streaming parser** — process large files without loading entirely into memory

## Release Checklist

- [ ] All tests pass: `cargo test`
- [ ] No clippy warnings: `cargo clippy -- -D warnings`
- [ ] Code formatted: `cargo fmt`
- [ ] Version bumped in `Cargo.toml`
- [ ] ARCHITECTURE.md updated if structure changed
- [ ] README.md updated if user-facing changes
- [ ] Tag pushed: `git tag v0.x.y && git push origin v0.x.y`
