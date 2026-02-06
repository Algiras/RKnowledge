# Architecture

> This document describes the internal architecture of RKnowledge so that AI agents and
> contributors can understand, analyze, and improve the codebase.

## Overview

RKnowledge is a Rust CLI that extracts knowledge graphs from documents using LLMs. The
pipeline is: **parse documents → chunk text → LLM extraction → graph building → storage/export**.

```
Input Documents → Parser → Chunker → LLM Client → Graph Builder → Neo4j / Export / Viz
```

## Module Map

```
src/
├── main.rs              # Entry point: CLI dispatch, tracing init
├── error.rs             # Error types (currently uses anyhow throughout)
├── config.rs            # TOML config loading with env var expansion
├── cli/
│   ├── mod.rs           # CLI definition (clap): Commands, enums
│   └── commands/
│       ├── mod.rs       # Command module declarations
│       ├── init.rs      # `init` - creates config, starts Neo4j Docker
│       ├── auth.rs      # `auth` - interactive API key configuration
│       ├── build.rs     # `build` - main pipeline: parse → LLM → graph → store
│       ├── query.rs     # `query` - natural language or Cypher search
│       ├── export.rs    # `export` - fetch from Neo4j → file format
│       └── viz.rs       # `viz` - generate HTML + open browser
├── parser/
│   ├── mod.rs           # DocumentParser: dispatch by extension, chunking
│   ├── chunker.rs       # TextChunker: sliding-window text splitter
│   ├── pdf.rs           # PDF text extraction (pdf-extract)
│   ├── text.rs          # Plain text loader
│   ├── markdown.rs      # Markdown → text (pulldown-cmark)
│   └── html.rs          # HTML → text (scraper)
├── llm/
│   ├── mod.rs           # LlmProviderTrait, LlmClient abstraction
│   ├── prompts.rs       # System/user prompts for relation extraction
│   ├── anthropic.rs     # Anthropic Claude provider
│   ├── openai.rs        # OpenAI GPT provider (also OpenAI-compatible)
│   ├── ollama.rs        # Ollama local provider
│   └── google.rs        # Google Gemini provider
├── graph/
│   ├── mod.rs           # Module exports
│   ├── builder.rs       # GraphBuilder: nodes, edges, contextual proximity
│   └── neo4j.rs         # Neo4jClient: store, fetch, query, search
└── export.rs            # Export functions: JSON, CSV, GraphML, Cypher
```

## Data Flow

### 1. Document Parsing (`parser/`)

```
File → Extension dispatch → Format-specific extractor → Raw text
                                                          ↓
                                                    TextChunker
                                                          ↓
                                              Vec<Document> (chunks)
```

Each chunk is a `Document` with a UUID `id`, `text`, `source` path, and `chunk_index`.
The chunker uses a sliding window with configurable `chunk_size` (default 1500) and
`chunk_overlap` (default 150).

### 2. LLM Extraction (`llm/`)

```
Chunk text → System prompt + User prompt → LLM API call → JSON response → Vec<Relation>
```

The prompt asks the LLM to extract `(node_1, node_2, edge)` triples. Each relation
represents: *"node_1 is related to node_2 via edge"*.

**Provider abstraction**: `LlmProviderTrait` with `extract_relations()`. Each provider
handles its own API format, auth, and JSON parsing.

**JSON parsing is lenient**: providers try to extract JSON from LLM responses even if
surrounded by markdown code fences or extra text.

### 3. Graph Building (`graph/builder.rs`)

```
Vec<Relation> → GraphBuilder → petgraph::DiGraph
                    │
                    ├── Node deduplication (lowercase, trimmed)
                    ├── Edge aggregation (same pair → merge relations, increase weight)
                    └── Contextual proximity (nodes in same chunk get weight=1 edges)
```

**Key data structures:**
- `node_indices: HashMap<String, NodeIndex>` — dedup by label
- `edges: HashMap<(String, String), EdgeData>` — aggregated edge data
- `node_chunks: HashMap<String, HashSet<String>>` — which chunks reference which nodes

**Edge weighting:**
- Explicit LLM relation: +4.0 weight
- Contextual proximity (same chunk): +1.0 weight
- Multiple occurrences accumulate

### 4. Storage (`graph/neo4j.rs`)

Neo4j schema:
```cypher
(:Concept {id, label, degree}) -[:RELATES_TO {relation, weight}]-> (:Concept)
```

The client clears existing `Concept` nodes before each `store_graph` call (destructive).
Index on `Concept.id` is created for lookup speed.

### 5. Export (`export.rs`)

Supports JSON, CSV, GraphML, and Cypher statement export. All export functions accept
slices of `GraphNode` and `GraphEdge` (the Neo4j types). The `build` command can bypass
Neo4j and export directly from the `GraphBuilder`.

### 6. Visualization (`cli/commands/viz.rs`)

Generates a standalone HTML file using vis-network.js. Graph data is embedded as inline
JSON. The file is written to a temp directory and opened in the default browser.

## Configuration (`config.rs`)

TOML file at `~/<config_dir>/rknowledge/config.toml` (platform-dependent via `dirs` crate).

Environment variable expansion: `${VAR_NAME}` or `$VAR_NAME` in any string field.

```toml
default_provider = "ollama"
default_model = "mistral"
chunk_size = 1500
chunk_overlap = 150

[providers.anthropic]
api_key = "${ANTHROPIC_API_KEY}"
model = "claude-sonnet-4-20250514"

[providers.ollama]
base_url = "http://localhost:11434"
model = "mistral"

[neo4j]
uri = "bolt://localhost:7687"
user = "neo4j"
password = "rknowledge"
database = "neo4j"
```

## Dependencies (key crates)

| Crate | Purpose |
|-------|---------|
| `clap` | CLI argument parsing with derive macros |
| `tokio` | Async runtime |
| `reqwest` | HTTP client for LLM APIs |
| `neo4rs` | Async Neo4j Bolt driver |
| `petgraph` | In-memory graph data structure |
| `serde` / `toml` / `serde_json` | Serialization |
| `pdf-extract` | PDF text extraction |
| `pulldown-cmark` | Markdown parsing |
| `scraper` | HTML parsing |
| `indicatif` / `console` | TUI progress bars and styling |
| `anyhow` | Error handling |
| `tracing` | Structured logging |

## Known Limitations & Improvement Areas

### Performance
- **Sequential LLM calls**: Chunks are processed one at a time. Could be parallelized
  with `futures::stream::buffered()` for concurrent API calls.
- **Neo4j batch writes**: Nodes and edges are inserted one at a time. Could use
  `UNWIND` for batch Cypher operations.
- **Graph building**: `calculate_contextual_proximity()` is O(n^2) per chunk.
  For large chunks with many nodes, this creates many edges.

### Functionality
- **No incremental updates**: `store_graph` clears all existing data. Should support
  merging new data with existing graph.
- **No community detection**: The `community` field on nodes is always `None`.
  Could implement Louvain or Label Propagation.
- **Query is basic**: Natural language search just regex-matches labels. Could use
  embeddings or LLM-powered query translation.
- **No streaming**: Large PDFs load entirely into memory before chunking.
- **No deduplication across runs**: Running the same documents twice doubles the data.

### Code Quality
- `error.rs` is effectively unused (everything uses `anyhow`). Could define proper
  error types for better error messages.
- Some `#[allow(dead_code)]` annotations — functions written but unused.
- `row_to_json` in `neo4j.rs` uses a hardcoded column name list. Should use
  the row's actual column names.
- Missing doc comments on many public types/functions.
- Test coverage is minimal — mostly unit tests, no integration tests.

### Distribution
- Install script downloads from GitHub Releases but the release workflow
  has not been tested end-to-end yet.
- No Homebrew formula or Cargo publish setup.
- No shell completions (clap supports generating these).

## Testing

```bash
# Unit tests
cargo test

# With logging
RUST_LOG=debug cargo test

# Specific module
cargo test graph::builder

# Integration test (requires Neo4j running)
cargo test -- --ignored
```

## Release Process

1. Update version in `Cargo.toml`
2. Commit and tag: `git tag v0.x.y`
3. Push tag: `git push origin v0.x.y`
4. GitHub Actions builds for all platforms and creates a release
5. Install script picks up the latest release automatically
