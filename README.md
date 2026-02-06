# 🧠 RKnowledge

[![CI](https://github.com/algimantask/rknowledge/actions/workflows/ci.yml/badge.svg)](https://github.com/algimantask/rknowledge/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/algimantask/rknowledge)](https://github.com/algimantask/rknowledge/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

High-performance knowledge graph extraction CLI using LLMs. Extract concepts and relationships from documents and store them in Neo4j.

**[Documentation](https://algimantask.github.io/rknowledge/)** · **[Architecture](ARCHITECTURE.md)** · **[Contributing](CONTRIBUTING.md)**

## Features

- **Multi-format support**: PDF, Markdown, HTML, and plain text
- **Multiple LLM providers**: Anthropic, OpenAI, Google, and Ollama (local)
- **Neo4j backend**: Full graph database with Cypher queries
- **Multiple export formats**: JSON, CSV, GraphML, Cypher
- **Interactive visualization**: Browser-based graph visualization
- **Fast**: Written in Rust for maximum performance, single binary

## Installation

### One-liner (recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/algimantask/rknowledge/main/install.sh | bash
```

Options:
```bash
# Specific version
curl -fsSL ... | bash -s -- --version v0.1.0

# Custom install directory
curl -fsSL ... | bash -s -- --install-dir /usr/local/bin
```

### From Source

```bash
git clone https://github.com/algimantask/rknowledge.git
cd rknowledge
cargo build --release
cp target/release/rknowledge ~/.local/bin/
```

### As a Skill

```bash
npx skills add algimantask/rknowledge
```

## Quick Start

```bash
# 1. Initialize configuration and start Neo4j
rknowledge init

# 2. Configure your LLM provider (interactive)
rknowledge auth

# 3. Build a knowledge graph from documents
rknowledge build ./docs/

# 4. Query the graph
rknowledge query "What are the main concepts?"

# 5. Visualize
rknowledge viz
```

## Commands

| Command | Description |
|---------|-------------|
| `init` | Initialize config and start Neo4j via Docker |
| `auth` | Configure API keys for LLM providers (interactive) |
| `build <path>` | Process documents and build knowledge graph |
| `query <query>` | Search the graph (natural language or `cypher:` prefix) |
| `export` | Export graph to JSON, CSV, GraphML, or Cypher |
| `viz` | Open interactive visualization in browser |

### Build Options

```bash
rknowledge build ./docs \
  --provider ollama \          # anthropic, openai, ollama, google
  --model mistral \            # provider-specific model name
  --output neo4j \             # neo4j, json, csv
  --chunk-size 1500 \          # text chunk size (chars)
  --chunk-overlap 150          # overlap between chunks
```

### Query Examples

```bash
# Natural language search
rknowledge query "machine learning"

# Direct Cypher query
rknowledge query "cypher: MATCH (n:Concept) RETURN n.label, n.degree ORDER BY n.degree DESC LIMIT 10"

# Find all relationships for a concept
rknowledge query "cypher: MATCH (n:Concept)-[r]->(m) WHERE n.label CONTAINS 'ai' RETURN n.label, r.relation, m.label"
```

## Configuration

Configuration is stored at `~/<config_dir>/rknowledge/config.toml`:

```toml
default_provider = "ollama"
default_model = "mistral"
chunk_size = 1500
chunk_overlap = 150

[providers.anthropic]
api_key = "${ANTHROPIC_API_KEY}"
model = "claude-sonnet-4-20250514"

[providers.openai]
api_key = "${OPENAI_API_KEY}"
model = "gpt-4o"

[providers.ollama]
base_url = "http://localhost:11434"
model = "mistral"

[providers.google]
api_key = "${GOOGLE_API_KEY}"
model = "gemini-pro"

[neo4j]
uri = "bolt://localhost:7687"
user = "neo4j"
password = "rknowledge"
database = "neo4j"
```

## LLM Providers

| Provider | Setup | Best For |
|----------|-------|----------|
| **Ollama** | `ollama run mistral` | Free, local, private data |
| **Anthropic** | `export ANTHROPIC_API_KEY=...` | Highest quality extraction |
| **OpenAI** | `export OPENAI_API_KEY=...` | Good balance of quality/speed |
| **Google** | `export GOOGLE_API_KEY=...` | Gemini models |

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                      CLI (clap)                              │
├──────────┬───────────┬────────────┬────────────┬─────────────┤
│  init    │  auth     │  build     │  query     │  viz/export │
├──────────┴───────────┴────────────┴────────────┴─────────────┤
│                                                              │
│  ┌────────────┐  ┌────────────┐  ┌─────────────────────┐    │
│  │   Parser   │  │ LLM Client │  │   Graph Builder     │    │
│  │ PDF/MD/HTML│→ │ Multi-prov │→ │ petgraph + proximity│    │
│  │ + Chunker  │  │ async HTTP │  │                     │    │
│  └────────────┘  └────────────┘  └─────────────────────┘    │
│                                           │                  │
│  ┌────────────────────────────────────────┴──────────────┐   │
│  │                  Storage Layer                        │   │
│  │  ┌──────────┐  ┌──────────────┐  ┌────────────────┐  │   │
│  │  │  Neo4j   │  │ Export       │  │ Visualization  │  │   │
│  │  │ (Docker) │  │ JSON/CSV/GML │  │ (vis-network)  │  │   │
│  │  └──────────┘  └──────────────┘  └────────────────┘  │   │
│  └───────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full deep-dive.

## How It Works

1. **Document Parsing**: Documents are loaded and converted to plain text
2. **Chunking**: Text is split into overlapping chunks (default 1500 chars)
3. **LLM Extraction**: Each chunk is sent to the LLM to extract `(concept, concept, relationship)` triples
4. **Graph Building**: Concepts become nodes, relationships become edges
5. **Contextual Proximity**: Concepts in the same chunk get additional weighted edges
6. **Storage**: Graph is stored in Neo4j for querying and visualization

## Export Formats

```bash
rknowledge export --format json --output graph.json
rknowledge export --format csv --output graph          # → graph.nodes.csv, graph.edges.csv
rknowledge export --format graphml --output graph.graphml
rknowledge export --format cypher --output import.cypher
```

## Neo4j Access

After `rknowledge init`, Neo4j is available at:
- **Browser**: http://localhost:7474
- **Bolt**: bolt://localhost:7687
- **Credentials**: neo4j / rknowledge

## Development

```bash
cargo test                  # Run tests
cargo clippy                # Lint
cargo fmt                   # Format
RUST_LOG=debug cargo run -- build ./demo_data  # Debug logging
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full development guide.

## License

MIT

## Credits

Inspired by [rahulnyk/knowledge_graph](https://github.com/rahulnyk/knowledge_graph).
