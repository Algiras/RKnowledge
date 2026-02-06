use petgraph::graph::{DiGraph, NodeIndex, UnGraph};
use petgraph::visit::EdgeRef;
use std::collections::HashMap;

/// Detect communities using Label Propagation Algorithm (LPA).
///
/// This is an iterative algorithm where each node adopts the label most common
/// among its neighbors. It converges when no node changes its label.
///
/// Works on an undirected view of the directed graph and respects edge weights.
pub fn label_propagation(
    graph: &DiGraph<String, f64>,
    max_iterations: usize,
) -> HashMap<NodeIndex, usize> {
    if graph.node_count() == 0 {
        return HashMap::new();
    }

    // Build undirected weighted graph for community detection
    let mut undirected: UnGraph<String, f64> = UnGraph::new_undirected();
    let mut idx_map: HashMap<NodeIndex, petgraph::graph::NodeIndex> = HashMap::new();
    let mut rev_map: HashMap<petgraph::graph::NodeIndex, NodeIndex> = HashMap::new();

    for ni in graph.node_indices() {
        let label = graph[ni].clone();
        let new_idx = undirected.add_node(label);
        idx_map.insert(ni, new_idx);
        rev_map.insert(new_idx, ni);
    }

    for edge in graph.edge_references() {
        let src = idx_map[&edge.source()];
        let tgt = idx_map[&edge.target()];
        // Avoid duplicate edges in undirected graph - add only if not already present
        if undirected.find_edge(src, tgt).is_none() {
            undirected.add_edge(src, tgt, *edge.weight());
        }
    }

    // Initialize: each node gets its own unique label
    let node_indices: Vec<_> = undirected.node_indices().collect();
    let mut labels: HashMap<petgraph::graph::NodeIndex, usize> = node_indices
        .iter()
        .enumerate()
        .map(|(i, &ni)| (ni, i))
        .collect();

    // Iterate
    for _iter in 0..max_iterations {
        let mut changed = false;

        // Process nodes in index order (deterministic for reproducibility)
        for &ni in &node_indices {
            // Count weighted votes from neighbors
            let mut vote_weights: HashMap<usize, f64> = HashMap::new();

            for edge in undirected.edges(ni) {
                let neighbor = edge.target();
                if neighbor == ni {
                    // Also check source for undirected
                    continue;
                }
                let neighbor_label = labels[&neighbor];
                let weight = *edge.weight();
                *vote_weights.entry(neighbor_label).or_insert(0.0) += weight;
            }

            if vote_weights.is_empty() {
                continue; // Isolated node keeps its label
            }

            // Pick label with highest total weight (tie-break: smallest label)
            let best_label = vote_weights
                .into_iter()
                .max_by(|(l1, w1), (l2, w2)| {
                    w1.partial_cmp(w2)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| l2.cmp(l1)) // smaller label wins ties
                })
                .map(|(label, _)| label)
                .unwrap();

            let current = labels.get_mut(&ni).unwrap();
            if *current != best_label {
                *current = best_label;
                changed = true;
            }
        }

        if !changed {
            break;
        }
    }

    // Remap labels to contiguous community IDs starting from 0
    let mut community_id_map: HashMap<usize, usize> = HashMap::new();
    let mut next_id = 0;

    let mut result: HashMap<NodeIndex, usize> = HashMap::new();
    for &ni in &node_indices {
        let label = labels[&ni];
        let community = *community_id_map.entry(label).or_insert_with(|| {
            let id = next_id;
            next_id += 1;
            id
        });
        let orig_idx = rev_map[&ni];
        result.insert(orig_idx, community);
    }

    result
}

/// Get a summary of communities: community_id -> list of node labels
pub fn community_summary(
    graph: &DiGraph<String, f64>,
    communities: &HashMap<NodeIndex, usize>,
) -> Vec<(usize, Vec<String>)> {
    let mut groups: HashMap<usize, Vec<String>> = HashMap::new();

    for (&ni, &community) in communities {
        let label = graph[ni].clone();
        groups.entry(community).or_default().push(label);
    }

    // Sort each group's members alphabetically
    for members in groups.values_mut() {
        members.sort();
    }

    // Sort groups by size descending
    let mut result: Vec<(usize, Vec<String>)> = groups.into_iter().collect();
    result.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_triangle() -> DiGraph<String, f64> {
        let mut g = DiGraph::new();
        let a = g.add_node("a".into());
        let b = g.add_node("b".into());
        let c = g.add_node("c".into());
        g.add_edge(a, b, 4.0);
        g.add_edge(b, c, 4.0);
        g.add_edge(a, c, 4.0);
        g
    }

    #[test]
    fn test_empty_graph() {
        let g: DiGraph<String, f64> = DiGraph::new();
        let communities = label_propagation(&g, 10);
        assert!(communities.is_empty());
    }

    #[test]
    fn test_single_node() {
        let mut g = DiGraph::new();
        let a = g.add_node("a".into());
        let communities = label_propagation(&g, 10);
        assert_eq!(communities.len(), 1);
        assert!(communities.contains_key(&a));
    }

    #[test]
    fn test_triangle_single_community() {
        let g = build_triangle();
        let communities = label_propagation(&g, 20);
        // All 3 nodes in a tight triangle should converge to same community
        let vals: Vec<usize> = communities.values().cloned().collect();
        assert_eq!(vals.len(), 3);
        assert!(
            vals[0] == vals[1] && vals[1] == vals[2],
            "triangle should be single community, got {:?}",
            vals
        );
    }

    #[test]
    fn test_two_disconnected_components() {
        let mut g = DiGraph::new();
        let a = g.add_node("a".into());
        let b = g.add_node("b".into());
        let c = g.add_node("c".into());
        let d = g.add_node("d".into());
        g.add_edge(a, b, 4.0);
        g.add_edge(c, d, 4.0);

        let communities = label_propagation(&g, 20);
        assert_ne!(
            communities[&a], communities[&c],
            "disconnected components should have different communities"
        );
        assert_eq!(communities[&a], communities[&b]);
        assert_eq!(communities[&c], communities[&d]);
    }

    #[test]
    fn test_two_clusters_with_weak_bridge() {
        let mut g = DiGraph::new();
        // Cluster 1: tight triangle
        let a = g.add_node("a".into());
        let b = g.add_node("b".into());
        let c = g.add_node("c".into());
        g.add_edge(a, b, 10.0);
        g.add_edge(b, c, 10.0);
        g.add_edge(a, c, 10.0);

        // Cluster 2: tight triangle
        let d = g.add_node("d".into());
        let e = g.add_node("e".into());
        let f = g.add_node("f".into());
        g.add_edge(d, e, 10.0);
        g.add_edge(e, f, 10.0);
        g.add_edge(d, f, 10.0);

        // Weak bridge
        g.add_edge(c, d, 0.1);

        let communities = label_propagation(&g, 50);
        // The two tightly-connected clusters should form distinct communities
        // (or the same, depending on convergence — but at minimum each cluster is internally consistent)
        assert_eq!(communities[&a], communities[&b]);
        assert_eq!(communities[&b], communities[&c]);
        assert_eq!(communities[&d], communities[&e]);
        assert_eq!(communities[&e], communities[&f]);
    }

    #[test]
    fn test_community_summary() {
        let mut g = DiGraph::new();
        let a = g.add_node("alpha".into());
        let b = g.add_node("beta".into());
        let _c = g.add_node("gamma".into());
        g.add_edge(a, b, 4.0);
        // c is isolated

        let communities = label_propagation(&g, 10);
        let summary = community_summary(&g, &communities);

        // Should have at least 2 communities (a+b) and (c)
        assert!(summary.len() >= 2);
        // Largest community first
        assert!(summary[0].1.len() >= summary[1].1.len());
    }

    #[test]
    fn test_community_ids_contiguous() {
        let g = build_triangle();
        let communities = label_propagation(&g, 10);
        let max_id = *communities.values().max().unwrap();
        let unique: std::collections::HashSet<usize> = communities.values().cloned().collect();
        // Community IDs should be contiguous from 0
        for id in 0..unique.len() {
            assert!(unique.contains(&id), "missing community id {}", id);
        }
        assert!(max_id < communities.len());
    }
}
