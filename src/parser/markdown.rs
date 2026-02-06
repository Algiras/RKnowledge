use anyhow::{Context, Result};
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use std::path::Path;

/// Extract plain text from a Markdown file
pub fn extract_text(path: &Path) -> Result<String> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read markdown file: {}", path.display()))?;

    Ok(markdown_to_text(&content))
}

/// Convert Markdown to plain text, preserving structure
fn markdown_to_text(markdown: &str) -> String {
    let parser = Parser::new(markdown);
    let mut text = String::new();

    for event in parser {
        match event {
            Event::Text(t) | Event::Code(t) => {
                text.push_str(&t);
            }
            Event::SoftBreak | Event::HardBreak => {
                text.push('\n');
            }
            Event::Start(Tag::Paragraph) => {
                if !text.is_empty() && !text.ends_with('\n') {
                    text.push('\n');
                }
            }
            Event::End(TagEnd::Paragraph) => {
                text.push_str("\n\n");
            }
            Event::Start(Tag::Heading { .. }) => {
                if !text.is_empty() && !text.ends_with('\n') {
                    text.push('\n');
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                text.push_str("\n\n");
            }
            Event::Start(Tag::Item) => {
                text.push_str("- ");
            }
            Event::End(TagEnd::Item) => {
                text.push('\n');
            }
            Event::Start(Tag::CodeBlock(_)) => {
                text.push_str("\n[code]\n");
            }
            Event::End(TagEnd::CodeBlock) => {
                text.push_str("\n[/code]\n\n");
            }
            Event::Start(Tag::BlockQuote(_)) => {
                text.push_str("> ");
            }
            _ => {}
        }
    }

    // Clean up extra whitespace
    text.lines()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_to_text() {
        let md = "# Hello\n\nThis is a **test** with `code`.\n\n- Item 1\n- Item 2";
        let text = markdown_to_text(md);
        
        assert!(text.contains("Hello"));
        assert!(text.contains("test"));
        assert!(text.contains("code"));
        assert!(text.contains("Item 1"));
    }

    #[test]
    fn test_code_block() {
        let md = "```rust\nfn main() {}\n```";
        let text = markdown_to_text(md);
        
        assert!(text.contains("[code]"));
        assert!(text.contains("fn main()"));
    }
}
