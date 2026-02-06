use anyhow::{Context, Result};
use neo4rs::{query, Graph, Row};
use serde::{Deserialize, Serialize};

use crate::config::Neo4jConfig;
use super::builder::GraphBuilder;

/// Node representation for Neo4j
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub community: Option<usize>,
    pub degree: Option<usize>,
    #[serde(default)]
    pub entity_type: Option<String>,
}

/// Edge representation for Neo4j
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub weight: f64,
}

/// Neo4j client for storing and querying knowledge graphs
pub struct Neo4jClient {
    graph: Graph,
}

impl Neo4jClient {
    /// Create a new Neo4j client
    pub async fn new(config: &Neo4jConfig) -> Result<Self> {
        let graph = Graph::new(&config.uri, &config.user, &config.password)
            .await
            .context("Failed to connect to Neo4j. Is Neo4j running?")?;

        Ok(Self { graph })
    }

    /// Store a graph in Neo4j (replaces existing data)
    pub async fn store_graph(&self, builder: &GraphBuilder) -> Result<()> {
        // Clear existing data
        self.graph
            .run(query("MATCH (n:Concept) DETACH DELETE n"))
            .await
            .context("Failed to clear existing concepts")?;

        // Create index on id for faster lookups
        self.graph
            .run(query("CREATE INDEX concept_id IF NOT EXISTS FOR (n:Concept) ON (n.id)"))
            .await
            .ok();

        // Create nodes with community info and entity type
        let nodes = builder.get_nodes();
        for node in &nodes {
            let q = query(
                "CREATE (n:Concept {id: $id, label: $label, degree: $degree, community: $community, entity_type: $entity_type})"
            )
            .param("id", node.id.clone())
            .param("label", node.label.clone())
            .param("degree", node.degree as i64)
            .param("community", node.community.unwrap_or(0) as i64)
            .param("entity_type", node.entity_type.clone().unwrap_or_else(|| "concept".to_string()));

            self.graph.run(q).await.context("Failed to create node")?;
        }

        // Create edges
        let edges = builder.get_edges();
        for edge in &edges {
            let q = query(
                "MATCH (a:Concept {id: $source}), (b:Concept {id: $target}) \
                 CREATE (a)-[r:RELATES_TO {relation: $relation, weight: $weight}]->(b)"
            )
            .param("source", edge.source.clone())
            .param("target", edge.target.clone())
            .param("relation", edge.relation.clone())
            .param("weight", edge.weight);

            self.graph.run(q).await.context("Failed to create edge")?;
        }

        Ok(())
    }

    /// Merge a graph into Neo4j (append mode -- preserves existing data)
    pub async fn merge_graph(&self, builder: &GraphBuilder) -> Result<()> {
        // Create index on id for faster lookups
        self.graph
            .run(query("CREATE INDEX concept_id IF NOT EXISTS FOR (n:Concept) ON (n.id)"))
            .await
            .ok();

        // MERGE nodes (create if not exists, update if exists)
        let nodes = builder.get_nodes();
        for node in &nodes {
            let q = query(
                "MERGE (n:Concept {id: $id}) \
                 ON CREATE SET n.label = $label, n.degree = $degree, n.community = $community, n.entity_type = $entity_type \
                 ON MATCH SET n.degree = n.degree + $degree, n.community = $community, n.entity_type = $entity_type"
            )
            .param("id", node.id.clone())
            .param("label", node.label.clone())
            .param("degree", node.degree as i64)
            .param("community", node.community.unwrap_or(0) as i64)
            .param("entity_type", node.entity_type.clone().unwrap_or_else(|| "concept".to_string()));

            self.graph.run(q).await.context("Failed to merge node")?;
        }

        // MERGE edges (accumulate weight on duplicate)
        let edges = builder.get_edges();
        for edge in &edges {
            let q = query(
                "MATCH (a:Concept {id: $source}), (b:Concept {id: $target}) \
                 MERGE (a)-[r:RELATES_TO {relation: $relation}]->(b) \
                 ON CREATE SET r.weight = $weight \
                 ON MATCH SET r.weight = r.weight + $weight"
            )
            .param("source", edge.source.clone())
            .param("target", edge.target.clone())
            .param("relation", edge.relation.clone())
            .param("weight", edge.weight);

            self.graph.run(q).await.context("Failed to merge edge")?;
        }

        Ok(())
    }

    /// Fetch all nodes and edges from Neo4j
    pub async fn fetch_graph(&self) -> Result<(Vec<GraphNode>, Vec<GraphEdge>)> {
        // Fetch nodes
        let mut result = self.graph
            .execute(query("MATCH (n:Concept) RETURN n.id AS id, n.label AS label, n.degree AS degree, n.community AS community, n.entity_type AS entity_type"))
            .await
            .context("Failed to fetch nodes")?;

        let mut nodes = Vec::new();
        while let Ok(Some(row)) = result.next().await {
            let id: String = row.get("id").unwrap_or_default();
            let label: String = row.get("label").unwrap_or_else(|_| id.clone());
            let degree: i64 = row.get("degree").unwrap_or(0);
            let community: i64 = row.get("community").unwrap_or(-1);
            let entity_type: Option<String> = row.get("entity_type").ok();

            nodes.push(GraphNode {
                id,
                label,
                community: if community >= 0 { Some(community as usize) } else { None },
                degree: Some(degree as usize),
                entity_type,
            });
        }

        // Fetch edges
        let mut result = self.graph
            .execute(query(
                "MATCH (a:Concept)-[r:RELATES_TO]->(b:Concept) \
                 RETURN a.id AS source, b.id AS target, r.relation AS relation, r.weight AS weight"
            ))
            .await
            .context("Failed to fetch edges")?;

        let mut edges = Vec::new();
        while let Ok(Some(row)) = result.next().await {
            let source: String = row.get("source").unwrap_or_default();
            let target: String = row.get("target").unwrap_or_default();
            let relation: String = row.get("relation").unwrap_or_else(|_| "related".to_string());
            let weight: f64 = row.get("weight").unwrap_or(1.0);

            edges.push(GraphEdge {
                source,
                target,
                relation,
                weight,
            });
        }

        Ok((nodes, edges))
    }

    /// Execute a raw Cypher query
    pub async fn execute_cypher(&self, cypher: &str) -> Result<Vec<serde_json::Value>> {
        let mut result = self.graph
            .execute(query(cypher))
            .await
            .context("Failed to execute Cypher query")?;

        let mut results = Vec::new();
        while let Ok(Some(row)) = result.next().await {
            // Convert row to JSON
            let json = row_to_json(&row);
            results.push(json);
        }

        Ok(results)
    }

    /// Search for concepts by name or relation
    pub async fn search_concepts(&self, search_term: &str) -> Result<Vec<(String, Vec<(String, String)>)>> {
        let search_pattern = format!("(?i).*{}.*", regex::escape(search_term));
        
        let mut result = self.graph
            .execute(query(
                "MATCH (n:Concept)-[r:RELATES_TO]-(m:Concept) \
                 WHERE n.label =~ $pattern OR r.relation =~ $pattern \
                 RETURN n.label AS concept, collect({related: m.label, edge: r.relation}) AS relations \
                 LIMIT 20"
            ).param("pattern", search_pattern))
            .await
            .context("Failed to search concepts")?;

        let mut results = Vec::new();
        while let Ok(Some(row)) = result.next().await {
            let concept: String = row.get("concept").unwrap_or_default();
            
            // Parse relations from the collected list
            let relations_json: Vec<serde_json::Value> = row.get("relations").unwrap_or_default();
            let relations: Vec<(String, String)> = relations_json
                .into_iter()
                .filter_map(|v| {
                    let related = v.get("related")?.as_str()?.to_string();
                    let edge = v.get("edge")?.as_str()?.to_string();
                    Some((related, edge))
                })
                .collect();

            if !relations.is_empty() {
                results.push((concept, relations));
            }
        }

        Ok(results)
    }

    /// Search for concepts with variable depth traversal
    pub async fn search_concepts_depth(&self, search_term: &str, depth: usize) -> Result<Vec<(String, Vec<(String, String)>)>> {
        let search_pattern = format!("(?i).*{}.*", regex::escape(search_term));
        let depth_val = depth.max(1).min(10) as i64; // Clamp to reasonable range

        let cypher = format!(
            "MATCH (n:Concept) WHERE n.label =~ $pattern \
             WITH n \
             MATCH path = (n)-[r:RELATES_TO*1..{}]-(m:Concept) \
             UNWIND relationships(path) AS rel \
             WITH n, endNode(rel) AS connected, rel \
             RETURN n.label AS concept, collect(DISTINCT {{related: connected.label, edge: rel.relation}}) AS relations \
             LIMIT 20",
            depth_val
        );

        let mut result = self.graph
            .execute(query(&cypher).param("pattern", search_pattern))
            .await
            .context("Failed to search concepts with depth")?;

        let mut results = Vec::new();
        while let Ok(Some(row)) = result.next().await {
            let concept: String = row.get("concept").unwrap_or_default();

            let relations_json: Vec<serde_json::Value> = row.get("relations").unwrap_or_default();
            let relations: Vec<(String, String)> = relations_json
                .into_iter()
                .filter_map(|v| {
                    let related = v.get("related")?.as_str()?.to_string();
                    let edge = v.get("edge")?.as_str()?.to_string();
                    Some((related, edge))
                })
                .collect();

            if !relations.is_empty() {
                results.push((concept, relations));
            }
        }

        Ok(results)
    }

    /// Get graph statistics
    #[allow(dead_code)]
    pub async fn get_stats(&self) -> Result<(usize, usize)> {
        let mut result = self.graph
            .execute(query("MATCH (n:Concept) RETURN count(n) AS node_count"))
            .await?;
        
        let node_count: i64 = if let Ok(Some(row)) = result.next().await {
            row.get("node_count").unwrap_or(0)
        } else {
            0
        };

        let mut result = self.graph
            .execute(query("MATCH ()-[r:RELATES_TO]->() RETURN count(r) AS edge_count"))
            .await?;
        
        let edge_count: i64 = if let Ok(Some(row)) = result.next().await {
            row.get("edge_count").unwrap_or(0)
        } else {
            0
        };

        Ok((node_count as usize, edge_count as usize))
    }
}

/// Convert a Neo4j row to a JSON value
fn row_to_json(row: &Row) -> serde_json::Value {
    // Try to extract common column types
    let mut result = serde_json::Map::new();
    
    // Try common column names
    for col in ["n", "m", "r", "n.label", "n.id", "n.degree", "m.label", "m.id", "count", "label", "id", "degree", "source", "target", "relation"] {
        if let Ok(val) = row.get::<String>(col) {
            result.insert(col.to_string(), serde_json::Value::String(val));
        } else if let Ok(val) = row.get::<i64>(col) {
            result.insert(col.to_string(), serde_json::Value::Number(val.into()));
        } else if let Ok(val) = row.get::<f64>(col) {
            if let Some(num) = serde_json::Number::from_f64(val) {
                result.insert(col.to_string(), serde_json::Value::Number(num));
            }
        }
    }
    
    if result.is_empty() {
        // Fallback to debug representation
        serde_json::json!({
            "raw": format!("{:?}", row)
        })
    } else {
        serde_json::Value::Object(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_node_serialization() {
        let node = GraphNode {
            id: "test".into(),
            label: "Test Node".into(),
            community: Some(1),
            degree: Some(5),
            entity_type: Some("concept".into()),
        };
        let json = serde_json::to_string(&node).unwrap();
        assert!(json.contains("Test Node"));
        assert!(json.contains("\"degree\":5"));
    }

    #[test]
    fn test_graph_node_deserialization() {
        let json = r#"{"id":"x","label":"X Node","community":null,"degree":3}"#;
        let node: GraphNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.id, "x");
        assert_eq!(node.degree, Some(3));
        assert!(node.community.is_none());
    }

    #[test]
    fn test_graph_edge_serialization() {
        let edge = GraphEdge {
            source: "a".into(),
            target: "b".into(),
            relation: "knows".into(),
            weight: 4.5,
        };
        let json = serde_json::to_string(&edge).unwrap();
        assert!(json.contains("\"source\":\"a\""));
        assert!(json.contains("4.5"));
    }

    #[test]
    fn test_graph_edge_deserialization() {
        let json = r#"{"source":"x","target":"y","relation":"uses","weight":2.0}"#;
        let edge: GraphEdge = serde_json::from_str(json).unwrap();
        assert_eq!(edge.source, "x");
        assert_eq!(edge.relation, "uses");
    }

    #[test]
    fn test_node_optional_fields() {
        let json = r#"{"id":"x","label":"X","community":null,"degree":null}"#;
        let node: GraphNode = serde_json::from_str(json).unwrap();
        assert!(node.community.is_none());
        assert!(node.degree.is_none());
    }

    #[test]
    fn test_roundtrip_node() {
        let original = GraphNode {
            id: "café".into(),
            label: "Café Node".into(),
            community: Some(42),
            degree: Some(10),
            entity_type: Some("location".into()),
        };
        let json = serde_json::to_string(&original).unwrap();
        let back: GraphNode = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, original.id);
        assert_eq!(back.community, original.community);
    }
}
