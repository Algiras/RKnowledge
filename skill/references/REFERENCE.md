# RKnowledge Reference

Detailed configuration and usage reference for RKnowledge.

## Configuration File

The configuration file is located at `~/.config/rknowledge/config.toml`.

### Full Configuration Example

```toml
# Default provider and model
default_provider = "anthropic"
default_model = "claude-sonnet-4-20250514"

# Text chunking settings
chunk_size = 1500
chunk_overlap = 150

# Provider configurations
[providers.anthropic]
api_key = "${ANTHROPIC_API_KEY}"
model = "claude-sonnet-4-20250514"

[providers.openai]
api_key = "${OPENAI_API_KEY}"
base_url = "https://api.openai.com/v1"  # Optional, for custom endpoints
model = "gpt-4o"

[providers.ollama]
base_url = "http://localhost:11434"
model = "mistral"

[providers.google]
api_key = "${GOOGLE_API_KEY}"
model = "gemini-pro"

# Neo4j configuration
[neo4j]
uri = "bolt://localhost:7687"
user = "neo4j"
password = "rknowledge"
database = "neo4j"  # Optional, defaults to "neo4j"
```

### Environment Variables

API keys can be set via environment variables:

| Variable | Description |
|----------|-------------|
| `ANTHROPIC_API_KEY` | Anthropic API key |
| `OPENAI_API_KEY` | OpenAI API key |
| `GOOGLE_API_KEY` | Google AI API key |
| `RKNOWLEDGE_PROVIDER` | Default provider override |
| `RKNOWLEDGE_MODEL` | Default model override |

## Graph Schema

### Node Properties

| Property | Type | Description |
|----------|------|-------------|
| `id` | String | Unique identifier (lowercased concept name) |
| `label` | String | Display label |
| `degree` | Integer | Number of connections |

### Edge Properties

| Property | Type | Description |
|----------|------|-------------|
| `relation` | String | Relationship description |
| `weight` | Float | Connection strength |

### Edge Types

1. **Explicit Relations**: Extracted by the LLM from text (weight: 4.0 per occurrence)
2. **Contextual Proximity**: Concepts appearing in the same chunk (weight: 1.0 per co-occurrence)

## Cypher Query Examples

### Find all concepts

```cypher
MATCH (n:Concept) RETURN n.label, n.degree ORDER BY n.degree DESC LIMIT 20
```

### Find related concepts

```cypher
MATCH (n:Concept)-[r:RELATES_TO]-(m:Concept)
WHERE n.label CONTAINS 'authentication'
RETURN n.label, r.relation, m.label
```

### Find paths between concepts

```cypher
MATCH path = shortestPath((a:Concept)-[*]-(b:Concept))
WHERE a.label = 'user' AND b.label = 'database'
RETURN path
```

### Get graph statistics

```cypher
MATCH (n:Concept) 
WITH count(n) as nodes
MATCH ()-[r:RELATES_TO]->()
RETURN nodes, count(r) as edges
```

### Find communities (clusters)

```cypher
CALL gds.louvain.stream('concept-graph')
YIELD nodeId, communityId
RETURN gds.util.asNode(nodeId).label AS concept, communityId
ORDER BY communityId, concept
```

## Export Formats

### JSON

```json
{
  "nodes": [
    {"id": "concept1", "label": "Concept 1", "degree": 5, "community": null}
  ],
  "edges": [
    {"source": "concept1", "target": "concept2", "relation": "relates to", "weight": 4.0}
  ]
}
```

### CSV

**nodes.csv**:
```csv
id,label,degree,community
"concept1","Concept 1",5,0
```

**edges.csv**:
```csv
source,target,relation,weight
"concept1","concept2","relates to",4.0
```

### GraphML

Standard GraphML format compatible with tools like Gephi, yEd, and NetworkX.

### Cypher

Neo4j import statements for recreating the graph in another database.

## Performance Tuning

### Large Documents

For documents with many pages:
- Increase `chunk_size` to reduce API calls
- Use a faster model (e.g., `gpt-4o-mini` or `claude-3-haiku`)

### Memory Usage

Neo4j memory settings in `docker-compose.yml`:
```yaml
environment:
  - NEO4J_dbms_memory_heap_max__size=2G
  - NEO4J_dbms_memory_pagecache_size=1G
```

### Batch Processing

For very large corpora, process in batches:
```bash
# Process subdirectories separately
for dir in ./docs/*/; do
  rknowledge build "$dir" --output neo4j
done
```

## API Integration

### Connecting to Neo4j from Python

```python
from neo4j import GraphDatabase

driver = GraphDatabase.driver(
    "bolt://localhost:7687",
    auth=("neo4j", "rknowledge")
)

with driver.session() as session:
    result = session.run(
        "MATCH (n:Concept)-[r]->(m) RETURN n, r, m LIMIT 10"
    )
    for record in result:
        print(record)
```

### Connecting from JavaScript

```javascript
const neo4j = require('neo4j-driver');

const driver = neo4j.driver(
  'bolt://localhost:7687',
  neo4j.auth.basic('neo4j', 'rknowledge')
);

const session = driver.session();
const result = await session.run(
  'MATCH (n:Concept) RETURN n.label LIMIT 10'
);
```

## Troubleshooting

### Common Issues

**"Connection refused" to Neo4j**
- Ensure Docker is running: `docker ps`
- Check Neo4j container: `docker logs rknowledge-neo4j`
- Wait for Neo4j to fully start (30-60 seconds)

**"API key not found"**
- Set environment variable: `export ANTHROPIC_API_KEY=your-key`
- Or edit config file: `~/.config/rknowledge/config.toml`

**"Failed to parse response"**
- Try a different model
- Check if the text contains unusual formatting
- Increase chunk size to provide more context

**"Out of memory"**
- Reduce chunk size
- Process fewer documents at once
- Increase Docker memory allocation

### Debug Logging

Enable debug output:
```bash
RUST_LOG=debug rknowledge build ./docs
```

### Reset Everything

```bash
# Stop and remove Neo4j container
docker compose -f ~/.config/rknowledge/docker-compose.yml down -v

# Remove config
rm -rf ~/.config/rknowledge

# Reinitialize
rknowledge init
```
