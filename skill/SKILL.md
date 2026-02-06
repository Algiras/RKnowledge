---
name: rknowledge
description: Build knowledge graphs from documents using LLMs. Extract concepts and relationships from PDF, Markdown, HTML, and text files, store in Neo4j, and export to various formats. Use when analyzing documents, building knowledge bases, or creating graph-based RAG systems.
license: MIT
metadata:
  author: rknowledge
  version: "0.2.0"
compatibility: Requires Docker for Neo4j backend. Supports Anthropic, OpenAI, Google, and Ollama (local) LLM providers.
---

# RKnowledge - Knowledge Graph Builder

Build knowledge graphs from any text corpus using LLMs. This skill helps you extract concepts and relationships from documents and store them in a queryable graph database.

## When to Use

Use this skill when you need to:
- Extract knowledge from documents (PDF, Markdown, HTML, TXT)
- Build a knowledge graph for Graph RAG applications
- Analyze relationships between concepts in a corpus
- Create visual representations of document content
- Query extracted knowledge using natural language or Cypher

## Quick Start

### 1. Initialize

```bash
# Install and initialize rknowledge
rknowledge init
```

This creates a configuration file and starts Neo4j via Docker.

### 2. Configure API Keys

Use the `auth` command to configure your LLM provider:

```bash
# Interactive setup
rknowledge auth

# Or specify provider directly
rknowledge auth --provider anthropic

# Or set directly with key
rknowledge auth --provider anthropic --key your-key-here

# List configured providers
rknowledge auth --list
```

Alternatively, use environment variables:

```bash
export ANTHROPIC_API_KEY=your-key-here
# or
export OPENAI_API_KEY=your-key-here
# or
export GOOGLE_API_KEY=your-key-here   # also accepts GEMINI_API_KEY
# or use Ollama for local models (no API key needed)
```

### 3. Build Knowledge Graph

```bash
# Process a single document
rknowledge build ./document.pdf

# Process a directory of documents
rknowledge build ./docs/

# Specify provider and model
rknowledge build ./docs/ --provider anthropic --model claude-sonnet-4-20250514
```

### 4. Query the Graph

```bash
# Natural language search
rknowledge query "What concepts relate to authentication?"

# Direct Cypher query
rknowledge query "cypher: MATCH (n)-[r]->(m) RETURN n, r, m LIMIT 10"
```

### 5. Export

```bash
# Export to JSON
rknowledge export --format json --output graph.json

# Export to CSV (creates nodes.csv and edges.csv)
rknowledge export --format csv --output graph

# Export to GraphML
rknowledge export --format graphml --output graph.graphml

# Export to Cypher statements
rknowledge export --format cypher --output import.cypher
```

### 6. Visualize

```bash
# Open interactive visualization in browser
rknowledge viz
```

## Commands Reference

| Command | Description |
|---------|-------------|
| `rknowledge init` | Initialize config and start Neo4j |
| `rknowledge auth` | Configure API keys for LLM providers |
| `rknowledge build <path>` | Process documents and build graph |
| `rknowledge query <query>` | Search or query the graph |
| `rknowledge path <from> <to>` | Find shortest path between concepts |
| `rknowledge stats` | Show graph statistics and analytics |
| `rknowledge communities` | List detected communities and members |
| `rknowledge export` | Export graph to various formats |
| `rknowledge viz` | Open interactive visualization in browser |
| `rknowledge doctor` | Check system health and diagnose problems |

## Build Options

| Option | Description | Default |
|--------|-------------|---------|
| `--provider` | LLM provider (anthropic, openai, ollama, google) | anthropic |
| `--model` | Model to use | Provider default |
| `--output` | Output destination (neo4j, json, csv) | neo4j |
| `--chunk-size` | Text chunk size in characters | 1500 |
| `--chunk-overlap` | Overlap between chunks | 150 |
| `--concurrency, -j` | Number of concurrent LLM requests | 4 |
| `--append` | Append to existing graph (incremental) | false |

## Supported File Types

- **PDF** (.pdf) - Extracts text from PDF documents
- **Markdown** (.md) - Parses and extracts text from Markdown
- **HTML** (.html, .htm) - Extracts text content from HTML
- **Plain Text** (.txt) - Direct text processing

## LLM Providers

### Anthropic (Recommended)
```bash
export ANTHROPIC_API_KEY=your-key
rknowledge build ./docs --provider anthropic --model claude-sonnet-4-20250514
```

### OpenAI
```bash
export OPENAI_API_KEY=your-key
rknowledge build ./docs --provider openai --model gpt-4o
```

### Google (Gemini)
```bash
# Accepts either GOOGLE_API_KEY or GEMINI_API_KEY
export GEMINI_API_KEY=your-key
rknowledge build ./docs --provider google --model gemini-2.0-flash
```

### Ollama (Local - Free)
```bash
# Start Ollama and pull a model first
ollama pull mistral
rknowledge build ./docs --provider ollama --model mistral
```

### OpenAI-Compatible APIs (Groq, DeepSeek, Mistral, Together, etc.)

The OpenAI provider works with **any OpenAI-compatible API**. Set `base_url` in your config file to point to the service:

```toml
# ~/.config/rknowledge/config.toml

# Groq (fast inference)
[providers.openai]
api_key = "${GROQ_API_KEY}"
base_url = "https://api.groq.com/openai/v1"
model = "llama-3.3-70b-versatile"

# DeepSeek
[providers.openai]
api_key = "${DEEPSEEK_API_KEY}"
base_url = "https://api.deepseek.com/v1"
model = "deepseek-chat"

# Mistral
[providers.openai]
api_key = "${MISTRAL_API_KEY}"
base_url = "https://api.mistral.ai/v1"
model = "mistral-large-latest"

# Together AI
[providers.openai]
api_key = "${TOGETHER_API_KEY}"
base_url = "https://api.together.xyz/v1"
model = "meta-llama/Llama-3-70b-chat-hf"

# OpenRouter (access many models through one API)
[providers.openai]
api_key = "${OPENROUTER_API_KEY}"
base_url = "https://openrouter.ai/api/v1"
model = "anthropic/claude-sonnet-4-20250514"

# LM Studio or vLLM (local)
[providers.openai]
api_key = "not-needed"
base_url = "http://localhost:1234/v1"
model = "local-model"
```

Then build with:
```bash
rknowledge build ./docs --provider openai
```

## Example Workflows

### Build a Knowledge Base from Documentation

```bash
# Clone a repo's docs
git clone https://github.com/example/project docs

# Build knowledge graph
rknowledge build ./docs --provider anthropic

# Query for specific topics
rknowledge query "How does authentication work?"
```

### Analyze Research Papers

```bash
# Process PDF papers
rknowledge build ./papers/ --chunk-size 2000

# Export for further analysis
rknowledge export --format json --output research-graph.json
```

### Create Graph RAG Backend

```bash
# Build comprehensive graph
rknowledge build ./knowledge-base/

# Query programmatically via Neo4j
# Connect to bolt://localhost:7687 with neo4j/rknowledge
```

## Neo4j Access

After running `rknowledge init`, Neo4j is available at:
- **Browser**: http://localhost:7474
- **Bolt**: bolt://localhost:7687
- **Credentials**: neo4j / rknowledge

## Troubleshooting

Run the built-in diagnostics first:
```bash
rknowledge doctor
```

This checks config, Docker, Neo4j connectivity, LLM providers, and graph status.

### Neo4j Connection Failed
```bash
# Check if Docker is running
docker ps

# Restart Neo4j
cd ~/.config/rknowledge
docker compose up -d
```

### API Key Issues
```bash
# Verify API key is set
echo $ANTHROPIC_API_KEY

# Or check config file
cat ~/.config/rknowledge/config.toml
```

### Large Documents
For very large documents, increase chunk size:
```bash
rknowledge build ./large-doc.pdf --chunk-size 3000 --chunk-overlap 300
```

## See Also

- [REFERENCE.md](references/REFERENCE.md) - Detailed configuration reference
- [Neo4j Documentation](https://neo4j.com/docs/)
- [Cypher Query Language](https://neo4j.com/docs/cypher-manual/)
