use petgraph::algo::{connected_components, dijkstra};
use petgraph::graph::{DiGraph, NodeIndex, UnGraph};
use petgraph::visit::EdgeRef;
use std::collections::HashMap;

/// Graph analytics results
#[derive(Debug, Clone)]
pub struct GraphStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub connected_components: usize,
    pub density: f64,
    pub avg_degree: f64,
    pub max_degree: usize,
    pub community_count: usize,
    pub top_pagerank: Vec<(String, f64)>,
    pub top_degree: Vec<(String, usize)>,
}

/// Compute PageRank scores for all nodes.
///
/// Uses the power iteration method with a damping factor (typically 0.85).
pub fn pagerank(
    graph: &DiGraph<String, f64>,
    damping: f64,
    iterations: usize,
) -> HashMap<NodeIndex, f64> {
    let n = graph.node_count();
    if n == 0 {
        return HashMap::new();
    }

    let n_f = n as f64;
    let mut scores: HashMap<NodeIndex, f64> =
        graph.node_indices().map(|ni| (ni, 1.0 / n_f)).collect();

    for _ in 0..iterations {
        let mut new_scores: HashMap<NodeIndex, f64> = graph
            .node_indices()
            .map(|ni| (ni, (1.0 - damping) / n_f))
            .collect();

        for ni in graph.node_indices() {
            let out_degree = graph.edges(ni).count();
            if out_degree == 0 {
                // Dangling node: distribute evenly
                let share = scores[&ni] * damping / n_f;
                for nj in graph.node_indices() {
                    *new_scores.get_mut(&nj).unwrap() += share;
                }
            } else {
                let share = scores[&ni] * damping / out_degree as f64;
                for edge in graph.edges(ni) {
                    *new_scores.get_mut(&edge.target()).unwrap() += share;
                }
            }
        }

        scores = new_scores;
    }

    scores
}

/// Compute degree (in + out) for each node.
pub fn node_degrees(graph: &DiGraph<String, f64>) -> HashMap<NodeIndex, usize> {
    graph
        .node_indices()
        .map(|ni| {
            let in_deg = graph
                .edges_directed(ni, petgraph::Direction::Incoming)
                .count();
            let out_deg = graph
                .edges_directed(ni, petgraph::Direction::Outgoing)
                .count();
            (ni, in_deg + out_deg)
        })
        .collect()
}

/// Find the shortest path between two nodes by label.
/// Uses an undirected view of the graph since edges are normalized by alphabetical order.
/// Returns (total_weight, list_of_node_labels) or None if no path exists.
pub fn shortest_path(
    graph: &DiGraph<String, f64>,
    from_label: &str,
    to_label: &str,
) -> Option<(f64, Vec<String>)> {
    let from_label_lower = from_label.to_lowercase();
    let to_label_lower = to_label.to_lowercase();

    if from_label_lower == to_label_lower {
        let idx = graph
            .node_indices()
            .find(|&ni| graph[ni].to_lowercase() == from_label_lower)?;
        return Some((0.0, vec![graph[idx].clone()]));
    }

    // Build undirected graph for path finding
    let mut undirected: UnGraph<String, f64> = UnGraph::new_undirected();
    let mut dir_to_undir: HashMap<NodeIndex, petgraph::graph::NodeIndex> = HashMap::new();
    let mut undir_to_dir: HashMap<petgraph::graph::NodeIndex, NodeIndex> = HashMap::new();

    for ni in graph.node_indices() {
        let new_idx = undirected.add_node(graph[ni].clone());
        dir_to_undir.insert(ni, new_idx);
        undir_to_dir.insert(new_idx, ni);
    }

    for edge in graph.edge_references() {
        let src = dir_to_undir[&edge.source()];
        let tgt = dir_to_undir[&edge.target()];
        if undirected.find_edge(src, tgt).is_none() {
            undirected.add_edge(src, tgt, *edge.weight());
        }
    }

    let from_dir = graph
        .node_indices()
        .find(|&ni| graph[ni].to_lowercase() == from_label_lower)?;
    let to_dir = graph
        .node_indices()
        .find(|&ni| graph[ni].to_lowercase() == to_label_lower)?;
    let from_idx = dir_to_undir[&from_dir];
    let to_idx = dir_to_undir[&to_dir];

    // Dijkstra with inverted weights (higher weight = stronger connection = shorter path)
    let costs = dijkstra(&undirected, from_idx, Some(to_idx), |e| {
        1.0 / e.weight().max(0.001)
    });
    let costs_std: HashMap<petgraph::graph::NodeIndex, f64> = costs.into_iter().collect();

    let cost = *costs_std.get(&to_idx)?;

    // BFS path reconstruction on undirected graph
    let path = reconstruct_path_undirected(&undirected, from_idx, to_idx, &costs_std)?;
    let labels: Vec<String> = path.iter().map(|&ni| undirected[ni].clone()).collect();

    Some((cost, labels))
}

/// Reconstruct shortest path on an undirected graph from dijkstra costs.
fn reconstruct_path_undirected(
    graph: &UnGraph<String, f64>,
    from: petgraph::graph::NodeIndex,
    to: petgraph::graph::NodeIndex,
    costs: &HashMap<petgraph::graph::NodeIndex, f64>,
) -> Option<Vec<petgraph::graph::NodeIndex>> {
    if from == to {
        return Some(vec![from]);
    }

    let mut path = vec![to];
    let mut current = to;

    for _ in 0..graph.node_count() {
        if current == from {
            path.reverse();
            return Some(path);
        }

        let current_cost = *costs.get(&current)?;
        let mut best_prev: Option<(petgraph::graph::NodeIndex, f64)> = None;

        for edge in graph.edges(current) {
            let neighbor = edge.target();
            if neighbor == current {
                continue;
            }
            if let Some(&neighbor_cost) = costs.get(&neighbor) {
                let edge_cost = 1.0 / edge.weight().max(0.001);
                let diff = (neighbor_cost + edge_cost - current_cost).abs();
                if diff < 1e-6 && (best_prev.is_none() || neighbor_cost < best_prev.unwrap().1) {
                    best_prev = Some((neighbor, neighbor_cost));
                }
            }
        }

        match best_prev {
            Some((prev, _)) => {
                path.push(prev);
                current = prev;
            }
            None => return None,
        }
    }

    None
}

/// Compute full graph statistics.
pub fn compute_stats(graph: &DiGraph<String, f64>) -> GraphStats {
    let node_count = graph.node_count();
    let edge_count = graph.edge_count();

    let connected = connected_components(graph);

    let density = if node_count > 1 {
        edge_count as f64 / (node_count as f64 * (node_count as f64 - 1.0))
    } else {
        0.0
    };

    let degrees = node_degrees(graph);
    let avg_degree = if node_count > 0 {
        degrees.values().sum::<usize>() as f64 / node_count as f64
    } else {
        0.0
    };
    let max_degree = degrees.values().copied().max().unwrap_or(0);

    // Top nodes by degree
    let mut degree_vec: Vec<(String, usize)> = degrees
        .iter()
        .map(|(&ni, &d)| (graph[ni].clone(), d))
        .collect();
    degree_vec.sort_by(|a, b| b.1.cmp(&a.1));
    degree_vec.truncate(10);

    // PageRank
    let pr = pagerank(graph, 0.85, 30);
    let mut pr_vec: Vec<(String, f64)> = pr
        .iter()
        .map(|(&ni, &score)| (graph[ni].clone(), score))
        .collect();
    pr_vec.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    pr_vec.truncate(10);

    // Community count
    let communities = super::community::label_propagation(graph, 50);
    let community_count = communities
        .values()
        .collect::<std::collections::HashSet<_>>()
        .len();

    GraphStats {
        node_count,
        edge_count,
        connected_components: connected,
        density,
        avg_degree,
        max_degree,
        community_count,
        top_pagerank: pr_vec,
        top_degree: degree_vec,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_simple_graph() -> DiGraph<String, f64> {
        let mut g = DiGraph::new();
        let a = g.add_node("a".into());
        let b = g.add_node("b".into());
        let c = g.add_node("c".into());
        g.add_edge(a, b, 4.0);
        g.add_edge(b, c, 4.0);
        g.add_edge(a, c, 2.0);
        g
    }

    #[test]
    fn test_pagerank_empty() {
        let g: DiGraph<String, f64> = DiGraph::new();
        let pr = pagerank(&g, 0.85, 10);
        assert!(pr.is_empty());
    }

    #[test]
    fn test_pagerank_sums_to_one() {
        let g = build_simple_graph();
        let pr = pagerank(&g, 0.85, 30);
        let total: f64 = pr.values().sum();
        assert!(
            (total - 1.0).abs() < 0.01,
            "PageRank should sum to ~1.0, got {}",
            total
        );
    }

    #[test]
    fn test_pagerank_sink_node_highest() {
        let g = build_simple_graph();
        let pr = pagerank(&g, 0.85, 30);
        // Node "c" receives links from both "a" and "b" but links to nobody
        let c_idx = g.node_indices().find(|&ni| g[ni] == "c").unwrap();
        let a_idx = g.node_indices().find(|&ni| g[ni] == "a").unwrap();
        assert!(
            pr[&c_idx] > pr[&a_idx],
            "c should have higher PageRank than a"
        );
    }

    #[test]
    fn test_node_degrees() {
        let g = build_simple_graph();
        let deg = node_degrees(&g);
        let a_idx = g.node_indices().find(|&ni| g[ni] == "a").unwrap();
        assert_eq!(deg[&a_idx], 2); // a -> b, a -> c (out edges only for 'a')
    }

    #[test]
    fn test_shortest_path_exists() {
        let g = build_simple_graph();
        let result = shortest_path(&g, "a", "c");
        assert!(result.is_some());
        let (cost, path) = result.unwrap();
        assert!(cost > 0.0);
        assert_eq!(path.first().unwrap(), "a");
        assert_eq!(path.last().unwrap(), "c");
    }

    #[test]
    fn test_shortest_path_no_path() {
        let mut g = DiGraph::new();
        g.add_node("isolated_a".into());
        g.add_node("isolated_b".into());
        let result = shortest_path(&g, "isolated_a", "isolated_b");
        assert!(result.is_none());
    }

    #[test]
    fn test_shortest_path_same_node() {
        let g = build_simple_graph();
        let result = shortest_path(&g, "a", "a");
        assert!(result.is_some());
        let (_, path) = result.unwrap();
        assert_eq!(path.len(), 1);
    }

    #[test]
    fn test_shortest_path_case_insensitive() {
        let g = build_simple_graph();
        let result = shortest_path(&g, "A", "C");
        assert!(result.is_some());
    }

    #[test]
    fn test_compute_stats() {
        let g = build_simple_graph();
        let stats = compute_stats(&g);
        assert_eq!(stats.node_count, 3);
        assert_eq!(stats.edge_count, 3);
        assert_eq!(stats.connected_components, 1);
        assert!(stats.density > 0.0);
        assert!(stats.avg_degree > 0.0);
        assert!(!stats.top_pagerank.is_empty());
        assert!(!stats.top_degree.is_empty());
    }

    #[test]
    fn test_compute_stats_disconnected() {
        let mut g = DiGraph::new();
        let a = g.add_node("a".into());
        let b = g.add_node("b".into());
        let c = g.add_node("c".into());
        let d = g.add_node("d".into());
        g.add_edge(a, b, 4.0);
        g.add_edge(c, d, 4.0);

        let stats = compute_stats(&g);
        assert_eq!(stats.connected_components, 2);
    }
}
