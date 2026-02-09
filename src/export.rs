use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use crate::graph::builder::GraphBuilder;
use crate::graph::neo4j::{GraphEdge, GraphNode};

/// Export format for JSON
#[derive(Serialize, Deserialize)]
struct JsonExport {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
}

/// Export graph from builder to JSON file
pub fn export_json(builder: &GraphBuilder, path: &Path) -> Result<()> {
    let nodes: Vec<GraphNode> = builder
        .get_nodes()
        .into_iter()
        .map(|n| GraphNode {
            id: n.id,
            label: n.label,
            community: n.community,
            degree: Some(n.degree),
            entity_type: n.entity_type,
            tenant: n.tenant,
        })
        .collect();

    let edges: Vec<GraphEdge> = builder
        .get_edges()
        .into_iter()
        .map(|e| GraphEdge {
            source: e.source,
            target: e.target,
            relation: e.relation,
            weight: e.weight,
        })
        .collect();

    export_json_from_data(&nodes, &edges, path)
}

/// Export nodes and edges to JSON file
pub fn export_json_from_data(nodes: &[GraphNode], edges: &[GraphEdge], path: &Path) -> Result<()> {
    let export = JsonExport {
        nodes: nodes.to_vec(),
        edges: edges.to_vec(),
    };

    let file =
        File::create(path).with_context(|| format!("Failed to create file: {}", path.display()))?;
    let writer = BufWriter::new(file);

    serde_json::to_writer_pretty(writer, &export).context("Failed to write JSON")?;

    Ok(())
}

/// Export graph from builder to CSV files
pub fn export_csv(builder: &GraphBuilder, nodes_path: &Path, edges_path: &Path) -> Result<()> {
    let nodes: Vec<GraphNode> = builder
        .get_nodes()
        .into_iter()
        .map(|n| GraphNode {
            id: n.id,
            label: n.label,
            community: n.community,
            degree: Some(n.degree),
            entity_type: n.entity_type,
            tenant: n.tenant,
        })
        .collect();

    let edges: Vec<GraphEdge> = builder
        .get_edges()
        .into_iter()
        .map(|e| GraphEdge {
            source: e.source,
            target: e.target,
            relation: e.relation,
            weight: e.weight,
        })
        .collect();

    export_csv_from_data(&nodes, &edges, nodes_path, edges_path)
}

/// Export nodes and edges to CSV files
pub fn export_csv_from_data(
    nodes: &[GraphNode],
    edges: &[GraphEdge],
    nodes_path: &Path,
    edges_path: &Path,
) -> Result<()> {
    // Write nodes CSV
    let file = File::create(nodes_path)
        .with_context(|| format!("Failed to create file: {}", nodes_path.display()))?;
    let mut writer = BufWriter::new(file);

    writeln!(writer, "id,label,degree,community")?;
    for node in nodes {
        writeln!(
            writer,
            "\"{}\",\"{}\",{},{}",
            escape_csv(&node.id),
            escape_csv(&node.label),
            node.degree.unwrap_or(0),
            node.community.unwrap_or(0)
        )?;
    }

    // Write edges CSV
    let file = File::create(edges_path)
        .with_context(|| format!("Failed to create file: {}", edges_path.display()))?;
    let mut writer = BufWriter::new(file);

    writeln!(writer, "source,target,relation,weight")?;
    for edge in edges {
        writeln!(
            writer,
            "\"{}\",\"{}\",\"{}\",{}",
            escape_csv(&edge.source),
            escape_csv(&edge.target),
            escape_csv(&edge.relation),
            edge.weight
        )?;
    }

    Ok(())
}

/// Export to GraphML format
pub fn export_graphml(nodes: &[GraphNode], edges: &[GraphEdge], path: &Path) -> Result<()> {
    let file =
        File::create(path).with_context(|| format!("Failed to create file: {}", path.display()))?;
    let mut writer = BufWriter::new(file);

    // Write GraphML header
    writeln!(writer, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
    writeln!(
        writer,
        r#"<graphml xmlns="http://graphml.graphdrawing.org/xmlns" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="http://graphml.graphdrawing.org/xmlns http://graphml.graphdrawing.org/xmlns/1.0/graphml.xsd">"#
    )?;

    // Define attribute keys
    writeln!(
        writer,
        r#"  <key id="label" for="node" attr.name="label" attr.type="string"/>"#
    )?;
    writeln!(
        writer,
        r#"  <key id="degree" for="node" attr.name="degree" attr.type="int"/>"#
    )?;
    writeln!(
        writer,
        r#"  <key id="relation" for="edge" attr.name="relation" attr.type="string"/>"#
    )?;
    writeln!(
        writer,
        r#"  <key id="weight" for="edge" attr.name="weight" attr.type="double"/>"#
    )?;

    // Start graph
    writeln!(writer, r#"  <graph id="G" edgedefault="directed">"#)?;

    // Write nodes
    for node in nodes {
        writeln!(writer, r#"    <node id="{}">"#, escape_xml(&node.id))?;
        writeln!(
            writer,
            r#"      <data key="label">{}</data>"#,
            escape_xml(&node.label)
        )?;
        writeln!(
            writer,
            r#"      <data key="degree">{}</data>"#,
            node.degree.unwrap_or(0)
        )?;
        writeln!(writer, r#"    </node>"#)?;
    }

    // Write edges
    for (i, edge) in edges.iter().enumerate() {
        writeln!(
            writer,
            r#"    <edge id="e{}" source="{}" target="{}">"#,
            i,
            escape_xml(&edge.source),
            escape_xml(&edge.target)
        )?;
        writeln!(
            writer,
            r#"      <data key="relation">{}</data>"#,
            escape_xml(&edge.relation)
        )?;
        writeln!(writer, r#"      <data key="weight">{}</data>"#, edge.weight)?;
        writeln!(writer, r#"    </edge>"#)?;
    }

    // Close graph and graphml
    writeln!(writer, r#"  </graph>"#)?;
    writeln!(writer, r#"</graphml>"#)?;

    Ok(())
}

/// Export to Cypher statements for Neo4j import
pub fn export_cypher(nodes: &[GraphNode], edges: &[GraphEdge], path: &Path) -> Result<()> {
    let file =
        File::create(path).with_context(|| format!("Failed to create file: {}", path.display()))?;
    let mut writer = BufWriter::new(file);

    // Write header comment
    writeln!(writer, "// RKnowledge Graph Export")?;
    writeln!(writer, "// Generated Cypher statements for Neo4j import")?;
    writeln!(writer)?;

    // Clear existing data (optional)
    writeln!(writer, "// Clear existing concepts (uncomment if needed)")?;
    writeln!(writer, "// MATCH (n:Concept) DETACH DELETE n;")?;
    writeln!(writer)?;

    // Create constraint/index
    writeln!(writer, "// Create index for faster lookups")?;
    writeln!(
        writer,
        "CREATE INDEX concept_id IF NOT EXISTS FOR (n:Concept) ON (n.id);"
    )?;
    writeln!(writer)?;

    // Create nodes
    writeln!(writer, "// Create nodes")?;
    for node in nodes {
        writeln!(
            writer,
            "CREATE (n:Concept {{id: '{}', label: '{}', degree: {}}});",
            escape_cypher(&node.id),
            escape_cypher(&node.label),
            node.degree.unwrap_or(0)
        )?;
    }
    writeln!(writer)?;

    // Create edges
    writeln!(writer, "// Create relationships")?;
    for edge in edges {
        writeln!(
            writer,
            "MATCH (a:Concept {{id: '{}'}}), (b:Concept {{id: '{}'}}) CREATE (a)-[:RELATES_TO {{relation: '{}', weight: {}}}]->(b);",
            escape_cypher(&edge.source),
            escape_cypher(&edge.target),
            escape_cypher(&edge.relation),
            edge.weight
        )?;
    }

    Ok(())
}

/// Escape special characters for CSV
fn escape_csv(s: &str) -> String {
    s.replace('"', "\"\"")
}

/// Escape special characters for XML
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Escape special characters for Cypher strings
fn escape_cypher(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_nodes() -> Vec<GraphNode> {
        vec![
            GraphNode {
                id: "rust".into(),
                label: "Rust".into(),
                community: None,
                degree: Some(3),
                entity_type: Some("technology".into()),
                tenant: "default".into(),
            },
            GraphNode {
                id: "tokio".into(),
                label: "Tokio".into(),
                community: Some(1),
                degree: Some(1),
                entity_type: None,
                tenant: "default".into(),
            },
        ]
    }

    fn sample_edges() -> Vec<GraphEdge> {
        vec![GraphEdge {
            source: "rust".into(),
            target: "tokio".into(),
            relation: "uses".into(),
            weight: 4.0,
        }]
    }

    // ── JSON ────────────────────────────────────────────────────────

    #[test]
    fn test_export_json_creates_valid_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.json");
        export_json_from_data(&sample_nodes(), &sample_edges(), &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed["nodes"].is_array());
        assert!(parsed["edges"].is_array());
        assert_eq!(parsed["nodes"].as_array().unwrap().len(), 2);
        assert_eq!(parsed["edges"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_export_json_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("rt.json");
        export_json_from_data(&sample_nodes(), &sample_edges(), &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: JsonExport = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed.nodes[0].id, "rust");
        assert_eq!(parsed.edges[0].relation, "uses");
    }

    #[test]
    fn test_export_json_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("empty.json");
        export_json_from_data(&[], &[], &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: JsonExport = serde_json::from_str(&content).unwrap();
        assert!(parsed.nodes.is_empty());
        assert!(parsed.edges.is_empty());
    }

    // ── CSV ─────────────────────────────────────────────────────────

    #[test]
    fn test_export_csv_creates_both_files() {
        let dir = tempdir().unwrap();
        let np = dir.path().join("nodes.csv");
        let ep = dir.path().join("edges.csv");
        export_csv_from_data(&sample_nodes(), &sample_edges(), &np, &ep).unwrap();

        assert!(np.exists());
        assert!(ep.exists());
    }

    #[test]
    fn test_export_csv_header_and_rows() {
        let dir = tempdir().unwrap();
        let np = dir.path().join("nodes.csv");
        let ep = dir.path().join("edges.csv");
        export_csv_from_data(&sample_nodes(), &sample_edges(), &np, &ep).unwrap();

        let nodes_csv = std::fs::read_to_string(&np).unwrap();
        let lines: Vec<&str> = nodes_csv.lines().collect();
        assert_eq!(lines[0], "id,label,degree,community");
        assert_eq!(lines.len(), 3); // header + 2 nodes

        let edges_csv = std::fs::read_to_string(&ep).unwrap();
        let lines: Vec<&str> = edges_csv.lines().collect();
        assert_eq!(lines[0], "source,target,relation,weight");
        assert_eq!(lines.len(), 2); // header + 1 edge
    }

    #[test]
    fn test_export_csv_special_characters() {
        let nodes = vec![GraphNode {
            id: "test\"node".into(),
            label: "A \"quoted\" label".into(),
            community: None,
            degree: Some(0),
            entity_type: None,
            tenant: "default".into(),
        }];
        let dir = tempdir().unwrap();
        let np = dir.path().join("n.csv");
        let ep = dir.path().join("e.csv");
        export_csv_from_data(&nodes, &[], &np, &ep).unwrap();

        let content = std::fs::read_to_string(&np).unwrap();
        assert!(content.contains("\"\"quoted\"\"")); // CSV double-quote escaping
    }

    // ── GraphML ─────────────────────────────────────────────────────

    #[test]
    fn test_export_graphml_valid_xml() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.graphml");
        export_graphml(&sample_nodes(), &sample_edges(), &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("<?xml"));
        assert!(content.contains("<graphml"));
        assert!(content.contains("</graphml>"));
        assert!(content.contains("<node id=\"rust\">"));
        assert!(content.contains("<edge id=\"e0\""));
    }

    #[test]
    fn test_export_graphml_xml_escaping() {
        let nodes = vec![GraphNode {
            id: "a&b".into(),
            label: "<script>".into(),
            community: None,
            degree: Some(0),
            entity_type: None,
            tenant: "default".into(),
        }];
        let dir = tempdir().unwrap();
        let path = dir.path().join("escape.graphml");
        export_graphml(&nodes, &[], &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("a&amp;b"));
        assert!(content.contains("&lt;script&gt;"));
        assert!(!content.contains("<script>"));
    }

    // ── Cypher ──────────────────────────────────────────────────────

    #[test]
    fn test_export_cypher_statements() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.cypher");
        export_cypher(&sample_nodes(), &sample_edges(), &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("CREATE (n:Concept"));
        assert!(content.contains("MATCH (a:Concept"));
        assert!(content.contains("RELATES_TO"));
        assert!(content.contains("'rust'"));
        assert!(content.contains("'tokio'"));
    }

    #[test]
    fn test_export_cypher_escaping() {
        let nodes = vec![GraphNode {
            id: "it's".into(),
            label: "it's a test".into(),
            community: None,
            degree: Some(0),
            entity_type: None,
            tenant: "default".into(),
        }];
        let dir = tempdir().unwrap();
        let path = dir.path().join("esc.cypher");
        export_cypher(&nodes, &[], &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("it\\'s"));
        assert!(!content.contains("it's a")); // unescaped quote should not be there
    }

    // ── From builder ────────────────────────────────────────────────

    #[test]
    fn test_export_json_from_builder() {
        let mut builder = GraphBuilder::new();
        builder.add_relations(
            vec![crate::llm::Relation {
                node_1: "A".into(),
                node_1_type: None,
                node_2: "B".into(),
                node_2_type: None,
                edge: "links".into(),
            }],
            "c1",
        );

        let dir = tempdir().unwrap();
        let path = dir.path().join("builder.json");
        export_json(&builder, &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["nodes"].as_array().unwrap().len(), 2);
    }

    // ── Escape functions ────────────────────────────────────────────

    #[test]
    fn test_escape_csv_double_quotes() {
        assert_eq!(escape_csv("test\"quote"), "test\"\"quote");
        assert_eq!(escape_csv("no special"), "no special");
    }

    #[test]
    fn test_escape_xml_all_entities() {
        assert_eq!(escape_xml("<test>"), "&lt;test&gt;");
        assert_eq!(escape_xml("a & b"), "a &amp; b");
        assert_eq!(escape_xml("\"hello\""), "&quot;hello&quot;");
        assert_eq!(escape_xml("it's"), "it&apos;s");
    }

    #[test]
    fn test_escape_cypher_quotes_and_backslash() {
        assert_eq!(escape_cypher("test'quote"), "test\\'quote");
        assert_eq!(escape_cypher("back\\slash"), "back\\\\slash");
        assert_eq!(escape_cypher("normal"), "normal");
    }
}
