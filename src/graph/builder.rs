use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::llm::Relation;

/// A node in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub degree: usize,
    pub community: Option<usize>,
    #[serde(default)]
    pub entity_type: Option<String>,
}

/// An edge in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub weight: f64,
    pub chunk_ids: Vec<String>,
}

/// Builder for constructing knowledge graphs
pub struct GraphBuilder {
    /// Map from node label to node index
    node_indices: HashMap<String, NodeIndex>,
    /// The underlying graph
    graph: DiGraph<String, EdgeData>,
    /// Track which nodes appear in which chunks
    node_chunks: HashMap<String, HashSet<String>>,
    /// Edge data aggregated by (source, target) pair
    edges: HashMap<(String, String), EdgeData>,
    /// Entity types per node label (most recently seen type wins)
    node_types: HashMap<String, String>,
}

#[derive(Clone)]
struct EdgeData {
    relations: Vec<String>,
    weight: f64,
    chunk_ids: HashSet<String>,
}

impl GraphBuilder {
    pub fn new() -> Self {
        Self {
            node_indices: HashMap::new(),
            graph: DiGraph::new(),
            node_chunks: HashMap::new(),
            edges: HashMap::new(),
            node_types: HashMap::new(),
        }
    }

    /// Add a node to the graph if it doesn't exist (public accessor)
    pub fn ensure_node_public(&mut self, label: &str) -> NodeIndex {
        self.ensure_node(label)
    }

    /// Add a node to the graph if it doesn't exist
    fn ensure_node(&mut self, label: &str) -> NodeIndex {
        let label = label.to_lowercase().trim().to_string();

        if let Some(&index) = self.node_indices.get(&label) {
            index
        } else {
            let index = self.graph.add_node(label.clone());
            self.node_indices.insert(label, index);
            index
        }
    }

    /// Add relations from LLM extraction
    pub fn add_relations(&mut self, relations: Vec<Relation>, chunk_id: &str) {
        for relation in relations {
            let node_1 = relation.node_1.to_lowercase().trim().to_string();
            let node_2 = relation.node_2.to_lowercase().trim().to_string();

            if node_1.is_empty() || node_2.is_empty() || node_1 == node_2 {
                continue;
            }

            // Ensure nodes exist
            self.ensure_node(&node_1);
            self.ensure_node(&node_2);

            // Store entity types if provided
            if let Some(t) = &relation.node_1_type {
                let t = t.to_lowercase().trim().to_string();
                if !t.is_empty() {
                    self.node_types.insert(node_1.clone(), t);
                }
            }
            if let Some(t) = &relation.node_2_type {
                let t = t.to_lowercase().trim().to_string();
                if !t.is_empty() {
                    self.node_types.insert(node_2.clone(), t);
                }
            }

            // Track which chunks contain which nodes
            self.node_chunks
                .entry(node_1.clone())
                .or_default()
                .insert(chunk_id.to_string());
            self.node_chunks
                .entry(node_2.clone())
                .or_default()
                .insert(chunk_id.to_string());

            // Add or update edge
            let key = if node_1 < node_2 {
                (node_1.clone(), node_2.clone())
            } else {
                (node_2.clone(), node_1.clone())
            };

            let edge_data = self.edges.entry(key).or_insert_with(|| EdgeData {
                relations: Vec::new(),
                weight: 0.0,
                chunk_ids: HashSet::new(),
            });

            edge_data.relations.push(relation.edge);
            edge_data.weight += 4.0; // Weight for explicit relation
            edge_data.chunk_ids.insert(chunk_id.to_string());
        }
    }

    /// Calculate contextual proximity edges
    /// Nodes that appear in the same chunk are related by contextual proximity
    pub fn calculate_contextual_proximity(&mut self) {
        // Group nodes by chunk
        let mut chunk_nodes: HashMap<String, Vec<String>> = HashMap::new();

        for (node, chunks) in &self.node_chunks {
            for chunk_id in chunks {
                chunk_nodes
                    .entry(chunk_id.clone())
                    .or_default()
                    .push(node.clone());
            }
        }

        // Create edges between nodes in the same chunk
        for (chunk_id, nodes) in &chunk_nodes {
            for i in 0..nodes.len() {
                for j in (i + 1)..nodes.len() {
                    let node_1 = &nodes[i];
                    let node_2 = &nodes[j];

                    let key = if node_1 < node_2 {
                        (node_1.clone(), node_2.clone())
                    } else {
                        (node_2.clone(), node_1.clone())
                    };

                    let edge_data = self.edges.entry(key).or_insert_with(|| EdgeData {
                        relations: Vec::new(),
                        weight: 0.0,
                        chunk_ids: HashSet::new(),
                    });

                    // Only add contextual proximity if no explicit relation exists
                    if (edge_data.relations.is_empty()
                        || !edge_data
                            .relations
                            .iter()
                            .any(|r| r != "contextual proximity"))
                        && !edge_data
                            .relations
                            .contains(&"contextual proximity".to_string())
                    {
                        edge_data.relations.push("contextual proximity".to_string());
                    }
                    edge_data.weight += 1.0; // Weight for contextual proximity
                    edge_data.chunk_ids.insert(chunk_id.clone());
                }
            }
        }
    }

    /// Build and return the final graph
    pub fn build(&self) -> DiGraph<String, f64> {
        let mut graph = DiGraph::new();
        let mut indices: HashMap<String, NodeIndex> = HashMap::new();

        // Add all nodes
        for label in self.node_indices.keys() {
            let idx = graph.add_node(label.clone());
            indices.insert(label.clone(), idx);
        }

        // Add all edges
        for ((source, target), edge_data) in &self.edges {
            if let (Some(&src_idx), Some(&tgt_idx)) = (indices.get(source), indices.get(target)) {
                graph.add_edge(src_idx, tgt_idx, edge_data.weight);
            }
        }

        graph
    }

    /// Get all nodes with their metadata, including community assignments and entity types
    pub fn get_nodes(&self) -> Vec<GraphNode> {
        let graph = self.build();
        let communities = super::community::label_propagation(&graph, 50);

        self.node_indices
            .iter()
            .map(|(label, &_node_idx)| {
                let degree = self
                    .edges
                    .keys()
                    .filter(|(s, t)| s == label || t == label)
                    .count();

                // Find the matching NodeIndex in the built graph
                let community = graph
                    .node_indices()
                    .find(|&ni| graph[ni] == *label)
                    .and_then(|ni| communities.get(&ni).copied());

                let entity_type = self.node_types.get(label).cloned();

                GraphNode {
                    id: label.clone(),
                    label: label.clone(),
                    degree,
                    community,
                    entity_type,
                }
            })
            .collect()
    }

    /// Get all edges with their metadata
    pub fn get_edges(&self) -> Vec<GraphEdge> {
        self.edges
            .iter()
            .map(|((source, target), data)| {
                let relation = data
                    .relations
                    .iter()
                    .find(|r| *r != "contextual proximity")
                    .cloned()
                    .unwrap_or_else(|| {
                        if data.relations.contains(&"contextual proximity".to_string()) {
                            "contextual proximity".to_string()
                        } else {
                            "related".to_string()
                        }
                    });

                GraphEdge {
                    source: source.clone(),
                    target: target.clone(),
                    relation,
                    weight: data.weight,
                    chunk_ids: data.chunk_ids.iter().cloned().collect(),
                }
            })
            .collect()
    }

    /// Get node count
    #[allow(dead_code)]
    pub fn node_count(&self) -> usize {
        self.node_indices.len()
    }

    /// Get edge count
    #[allow(dead_code)]
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}

impl Default for GraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(n1: &str, n2: &str, edge: &str) -> Relation {
        Relation {
            node_1: n1.to_string(),
            node_1_type: None,
            node_2: n2.to_string(),
            node_2_type: None,
            edge: edge.to_string(),
        }
    }

    #[test]
    fn test_add_relations_basic() {
        let mut builder = GraphBuilder::new();
        builder.add_relations(
            vec![
                rel("Alice", "Bob", "knows"),
                rel("Bob", "Charlie", "works with"),
            ],
            "c1",
        );
        assert_eq!(builder.node_count(), 3);
        assert_eq!(builder.edge_count(), 2);
    }

    #[test]
    fn test_node_deduplication_case_insensitive() {
        let mut builder = GraphBuilder::new();
        builder.add_relations(vec![rel("Rust", "RUST", "same")], "c1");
        // "rust" == "rust" so self-loop is skipped, but node is created once
        // Actually the self-loop filter kicks in: node_1 == node_2 after lowercase
        assert_eq!(builder.node_count(), 0); // self-loop skipped entirely
        assert_eq!(builder.edge_count(), 0);
    }

    #[test]
    fn test_self_loop_skipped() {
        let mut builder = GraphBuilder::new();
        builder.add_relations(vec![rel("same", "same", "identity")], "c1");
        assert_eq!(builder.node_count(), 0);
        assert_eq!(builder.edge_count(), 0);
    }

    #[test]
    fn test_empty_node_skipped() {
        let mut builder = GraphBuilder::new();
        builder.add_relations(
            vec![rel("", "something", "edge"), rel("something", "", "edge")],
            "c1",
        );
        assert_eq!(builder.node_count(), 0);
        assert_eq!(builder.edge_count(), 0);
    }

    #[test]
    fn test_edge_weight_accumulates() {
        let mut builder = GraphBuilder::new();
        // Same pair mentioned twice
        builder.add_relations(vec![rel("a", "b", "first")], "c1");
        builder.add_relations(vec![rel("a", "b", "second")], "c2");

        let edges = builder.get_edges();
        assert_eq!(edges.len(), 1);
        // Each explicit relation adds 4.0
        assert!((edges[0].weight - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_edge_direction_normalized() {
        let mut builder = GraphBuilder::new();
        // (a, b) and (b, a) should be the same edge
        builder.add_relations(vec![rel("b", "a", "first")], "c1");
        builder.add_relations(vec![rel("a", "b", "second")], "c2");

        assert_eq!(builder.edge_count(), 1);
        let edges = builder.get_edges();
        assert!((edges[0].weight - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_contextual_proximity_creates_edges() {
        let mut builder = GraphBuilder::new();
        // Three nodes in the same chunk but only 2 explicit edges
        builder.add_relations(
            vec![
                rel("concept1", "concept2", "relates to"),
                rel("concept1", "concept3", "influences"),
            ],
            "chunk1",
        );
        assert_eq!(builder.edge_count(), 2);

        builder.calculate_contextual_proximity();
        // concept2-concept3 should now also have an edge
        assert_eq!(builder.edge_count(), 3);

        let edges = builder.get_edges();
        let proximity_edge = edges
            .iter()
            .find(|e| {
                (e.source == "concept2" && e.target == "concept3")
                    || (e.source == "concept3" && e.target == "concept2")
            })
            .expect("proximity edge should exist");
        assert_eq!(proximity_edge.relation, "contextual proximity");
    }

    #[test]
    fn test_contextual_proximity_across_chunks() {
        let mut builder = GraphBuilder::new();
        builder.add_relations(vec![rel("a", "b", "r1")], "chunk1");
        builder.add_relations(vec![rel("c", "d", "r2")], "chunk2");

        builder.calculate_contextual_proximity();
        // a-b in chunk1, c-d in chunk2 -> no cross-chunk proximity edges (only within chunks)
        // chunk1 has a,b -> proximity edge a-b already exists as explicit
        // chunk2 has c,d -> proximity edge c-d already exists as explicit
        // No new edges should be created beyond the 2 explicit ones
        assert_eq!(builder.edge_count(), 2);
    }

    #[test]
    fn test_get_nodes_degree() {
        let mut builder = GraphBuilder::new();
        builder.add_relations(
            vec![
                rel("hub", "spoke1", "r"),
                rel("hub", "spoke2", "r"),
                rel("hub", "spoke3", "r"),
            ],
            "c1",
        );

        let nodes = builder.get_nodes();
        let hub = nodes.iter().find(|n| n.id == "hub").unwrap();
        assert_eq!(hub.degree, 3); // connected to 3 edges
    }

    #[test]
    fn test_get_edges_prefers_explicit_over_proximity() {
        let mut builder = GraphBuilder::new();
        builder.add_relations(vec![rel("a", "b", "explicit relation")], "c1");
        builder.calculate_contextual_proximity();

        let edges = builder.get_edges();
        let edge = edges
            .iter()
            .find(|e| e.source == "a" || e.target == "a")
            .unwrap();
        assert_eq!(edge.relation, "explicit relation");
    }

    #[test]
    fn test_build_petgraph() {
        let mut builder = GraphBuilder::new();
        builder.add_relations(vec![rel("a", "b", "r1"), rel("b", "c", "r2")], "c1");

        let graph = builder.build();
        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edge_count(), 2);
    }

    #[test]
    fn test_large_graph() {
        let mut builder = GraphBuilder::new();
        let mut relations = Vec::new();
        for i in 0..100 {
            relations.push(rel(
                &format!("node_{}", i),
                &format!("node_{}", i + 1),
                &format!("edge_{}", i),
            ));
        }
        builder.add_relations(relations, "big_chunk");
        assert_eq!(builder.node_count(), 101);
        assert_eq!(builder.edge_count(), 100);
    }

    #[test]
    fn test_chunk_ids_tracked() {
        let mut builder = GraphBuilder::new();
        builder.add_relations(vec![rel("a", "b", "r")], "chunk_1");
        builder.add_relations(vec![rel("a", "b", "r")], "chunk_2");

        let edges = builder.get_edges();
        assert_eq!(edges[0].chunk_ids.len(), 2);
        assert!(edges[0].chunk_ids.contains(&"chunk_1".to_string()));
        assert!(edges[0].chunk_ids.contains(&"chunk_2".to_string()));
    }
}
