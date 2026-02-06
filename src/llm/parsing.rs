//! Shared JSON parsing utilities for LLM response handling.
//!
//! LLM responses are unpredictable — they may wrap JSON in markdown fences,
//! include explanatory text before/after, or return malformed JSON. This module
//! provides robust extraction and parsing of `Vec<Relation>` from raw LLM output.

use anyhow::Result;

use super::Relation;

/// Parse a raw LLM response string into a `Vec<Relation>`.
///
/// Handles:
/// - Clean JSON arrays
/// - JSON wrapped in markdown code fences (` ```json ... ``` `)
/// - JSON with leading/trailing prose
/// - Completely invalid JSON (returns empty vec with a warning)
/// - Empty node names and self-loops (filtered out)
///
/// Node names are lowercased and trimmed. Empty relations are discarded.
pub fn parse_relations_json(response: &str) -> Result<Vec<Relation>> {
    let json_str = extract_json_array(response);

    match serde_json::from_str::<Vec<Relation>>(&json_str) {
        Ok(relations) => Ok(relations
            .into_iter()
            .map(|mut r| {
                r.node_1 = r.node_1.to_lowercase().trim().to_string();
                r.node_2 = r.node_2.to_lowercase().trim().to_string();
                r.edge = r.edge.trim().to_string();
                r
            })
            .filter(|r| !r.node_1.is_empty() && !r.node_2.is_empty() && r.node_1 != r.node_2)
            .collect()),
        Err(e) => {
            tracing::warn!(
                "Failed to parse relations JSON: {}. Response: {}",
                e,
                response
            );
            Ok(vec![])
        }
    }
}

/// Extract a JSON array from a response that may contain extra text.
///
/// Tries the following strategies in order:
/// 1. Strip markdown code fences (` ```json ... ``` `)
/// 2. If the (cleaned) text starts with `[`, find matching `]`
/// 3. Search for the first `[` in the text and find its matching `]`
/// 4. Fall back to returning the original text as-is
pub fn extract_json_array(response: &str) -> String {
    let response = response.trim();

    // Strip markdown code fences if present
    let stripped = strip_code_fences(response);

    // Strategy 1: starts with [
    if stripped.starts_with('[')
        && let Some(end) = find_matching_bracket(stripped)
    {
        return stripped[..=end].to_string();
    }

    // Strategy 2: find first [ anywhere
    if let Some(start) = stripped.find('[')
        && let Some(end) = find_matching_bracket(&stripped[start..])
    {
        return stripped[start..=start + end].to_string();
    }

    // Fallback
    stripped.to_string()
}

/// Strip markdown code fences (``` or ```json) from around content.
fn strip_code_fences(s: &str) -> &str {
    let s = s.trim();

    // Handle ```json\n...\n``` or ```\n...\n```
    if s.starts_with("```") {
        // Find the end of the opening fence line
        if let Some(first_newline) = s.find('\n') {
            let inner = &s[first_newline + 1..];
            // Find closing fence
            if let Some(closing) = inner.rfind("```") {
                return inner[..closing].trim();
            }
        }
    }

    s
}

/// Find the index of the `]` that matches the first `[` in the string.
///
/// Returns `None` if brackets are unbalanced.
fn find_matching_bracket(s: &str) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, c) in s.chars().enumerate() {
        if escape_next {
            escape_next = false;
            continue;
        }
        if c == '\\' && in_string {
            escape_next = true;
            continue;
        }
        if c == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── extract_json_array ──────────────────────────────────────────────

    #[test]
    fn test_extract_clean_array() {
        let input = r#"[{"node_1":"a","node_2":"b","edge":"rel"}]"#;
        let result = extract_json_array(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_extract_with_leading_text() {
        let input = r#"Here are the relations: [{"node_1":"a","node_2":"b","edge":"rel"}]"#;
        let result = extract_json_array(input);
        assert!(result.starts_with('['));
        assert!(result.ends_with(']'));
        assert!(result.contains("\"node_1\""));
    }

    #[test]
    fn test_extract_with_trailing_text() {
        let input = r#"[{"node_1":"a","node_2":"b","edge":"rel"}] Hope this helps!"#;
        let result = extract_json_array(input);
        assert!(result.starts_with('['));
        assert!(result.ends_with(']'));
        assert!(!result.contains("Hope"));
    }

    #[test]
    fn test_extract_with_markdown_fences() {
        let input = "```json\n[{\"node_1\":\"a\",\"node_2\":\"b\",\"edge\":\"r\"}]\n```";
        let result = extract_json_array(input);
        assert!(result.starts_with('['));
        assert!(result.ends_with(']'));
    }

    #[test]
    fn test_extract_with_plain_fences() {
        let input = "```\n[{\"node_1\":\"a\",\"node_2\":\"b\",\"edge\":\"r\"}]\n```";
        let result = extract_json_array(input);
        assert!(result.starts_with('['));
        assert!(result.ends_with(']'));
    }

    #[test]
    fn test_extract_empty_array() {
        assert_eq!(extract_json_array("[]"), "[]");
    }

    #[test]
    fn test_extract_no_json() {
        let input = "I couldn't extract any relations from this text.";
        let result = extract_json_array(input);
        // Should return as-is when no brackets found
        assert_eq!(result, input);
    }

    #[test]
    fn test_extract_nested_brackets_in_strings() {
        let input = r#"[{"node_1":"array[0]","node_2":"b","edge":"r"}]"#;
        let result = extract_json_array(input);
        assert!(result.starts_with('['));
        assert!(result.ends_with(']'));
        // Should parse correctly because brackets inside strings are ignored
        let parsed: Vec<Relation> = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed[0].node_1, "array[0]");
    }

    // ── find_matching_bracket ───────────────────────────────────────────

    #[test]
    fn test_bracket_simple() {
        assert_eq!(find_matching_bracket("[abc]"), Some(4));
    }

    #[test]
    fn test_bracket_nested() {
        assert_eq!(find_matching_bracket("[[a],[b]]"), Some(8));
    }

    #[test]
    fn test_bracket_unbalanced() {
        assert_eq!(find_matching_bracket("[abc"), None);
    }

    #[test]
    fn test_bracket_string_with_brackets() {
        assert_eq!(find_matching_bracket(r#"["a]b"]"#), Some(6));
    }

    #[test]
    fn test_bracket_escaped_quote() {
        // String with escaped quote inside: ["a\"b"]
        assert_eq!(find_matching_bracket(r#"["a\"b"]"#), Some(7));
    }

    // ── strip_code_fences ───────────────────────────────────────────────

    #[test]
    fn test_strip_json_fences() {
        assert_eq!(strip_code_fences("```json\n[1,2,3]\n```"), "[1,2,3]");
    }

    #[test]
    fn test_strip_plain_fences() {
        assert_eq!(strip_code_fences("```\nhello\n```"), "hello");
    }

    #[test]
    fn test_strip_no_fences() {
        assert_eq!(strip_code_fences("[1,2,3]"), "[1,2,3]");
    }

    // ── parse_relations_json ────────────────────────────────────────────

    #[test]
    fn test_parse_valid_json() {
        let input = r#"[
            {"node_1": "Rust", "node_2": "Systems Programming", "edge": "is used for"},
            {"node_1": "Tokio", "node_2": "Rust", "edge": "is an async runtime for"}
        ]"#;
        let relations = parse_relations_json(input).unwrap();
        assert_eq!(relations.len(), 2);
        assert_eq!(relations[0].node_1, "rust");
        assert_eq!(relations[0].node_2, "systems programming");
        assert_eq!(relations[1].node_1, "tokio");
    }

    #[test]
    fn test_parse_lowercases_nodes() {
        let input = r#"[{"node_1": "UPPER CASE", "node_2": "Mixed Case", "edge": "some edge"}]"#;
        let relations = parse_relations_json(input).unwrap();
        assert_eq!(relations[0].node_1, "upper case");
        assert_eq!(relations[0].node_2, "mixed case");
    }

    #[test]
    fn test_parse_trims_whitespace() {
        let input = r#"[{"node_1": "  padded  ", "node_2": "  also padded  ", "edge": " edge "}]"#;
        let relations = parse_relations_json(input).unwrap();
        assert_eq!(relations[0].node_1, "padded");
        assert_eq!(relations[0].node_2, "also padded");
        assert_eq!(relations[0].edge, "edge");
    }

    #[test]
    fn test_parse_filters_empty_nodes() {
        let input = r#"[
            {"node_1": "valid", "node_2": "also valid", "edge": "ok"},
            {"node_1": "", "node_2": "orphan", "edge": "bad"},
            {"node_1": "orphan", "node_2": "", "edge": "bad"}
        ]"#;
        let relations = parse_relations_json(input).unwrap();
        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].node_1, "valid");
    }

    #[test]
    fn test_parse_filters_self_loops() {
        let input = r#"[{"node_1": "same", "node_2": "same", "edge": "self ref"}]"#;
        let relations = parse_relations_json(input).unwrap();
        assert_eq!(relations.len(), 0);
    }

    #[test]
    fn test_parse_self_loop_case_insensitive() {
        let input = r#"[{"node_1": "RUST", "node_2": "rust", "edge": "self"}]"#;
        let relations = parse_relations_json(input).unwrap();
        assert_eq!(relations.len(), 0);
    }

    #[test]
    fn test_parse_invalid_json_returns_empty() {
        let input = "This is not JSON at all, just some text.";
        let relations = parse_relations_json(input).unwrap();
        assert!(relations.is_empty());
    }

    #[test]
    fn test_parse_empty_array() {
        let relations = parse_relations_json("[]").unwrap();
        assert!(relations.is_empty());
    }

    #[test]
    fn test_parse_wrapped_in_prose() {
        let input = r#"Based on the text, here are the extracted relations:

```json
[
  {"node_1": "machine learning", "node_2": "artificial intelligence", "edge": "is a subset of"},
  {"node_1": "neural networks", "node_2": "deep learning", "edge": "are the basis of"}
]
```

These represent the key relationships found in the text."#;
        let relations = parse_relations_json(input).unwrap();
        assert_eq!(relations.len(), 2);
        assert_eq!(relations[0].node_1, "machine learning");
    }

    #[test]
    fn test_parse_real_ollama_response() {
        // Simulates a typical Ollama/Mistral response with extra whitespace
        let input = r#"
[
    {
        "node_1": "artificial intelligence",
        "node_2": "machine learning",
        "edge": "AI encompasses machine learning as a key subfield"
    },
    {
        "node_1": "neural networks",
        "node_2": "deep learning",
        "edge": "deep learning uses neural networks with many layers"
    },
    {
        "node_1": "nlp",
        "node_2": "large language models",
        "edge": "LLMs are a major advancement in NLP"
    }
]
"#;
        let relations = parse_relations_json(input).unwrap();
        assert_eq!(relations.len(), 3);
    }

    #[test]
    fn test_parse_handles_unicode() {
        // Use raw string with actual UTF-8 bytes
        let input = r#"[{"node_1": "tokyo", "node_2": "japan", "edge": "capital of"}]"#;
        let relations = parse_relations_json(input).unwrap();
        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].node_1, "tokyo");
        assert_eq!(relations[0].node_2, "japan");
    }

    #[test]
    fn test_parse_handles_accented_chars() {
        // JSON with unicode escape sequences (as LLMs sometimes produce)
        let input = r#"[{"node_1": "caf\u00e9", "node_2": "th\u00e9", "edge": "serves"}]"#;
        let relations = parse_relations_json(input).unwrap();
        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].node_1, "caf\u{00e9}"); // serde decodes \u00e9 to é
    }

    #[test]
    fn test_parse_handles_special_chars_in_edge() {
        let input = r#"[{"node_1": "a", "node_2": "b", "edge": "relates to (see also: \"c\")"}]"#;
        let relations = parse_relations_json(input).unwrap();
        assert_eq!(relations.len(), 1);
    }
}
