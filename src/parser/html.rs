use anyhow::{Context, Result};
use scraper::{Html, Selector};
use std::path::Path;

/// Extract text from an HTML file
pub fn extract_text(path: &Path) -> Result<String> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read HTML file: {}", path.display()))?;

    Ok(html_to_text(&content))
}

/// Convert HTML to plain text
fn html_to_text(html: &str) -> String {
    let document = Html::parse_document(html);

    // Remove script and style elements
    let mut text_parts = Vec::new();

    // Try to get main content first
    let main_selectors = ["main", "article", "body"];
    let mut found_main = false;

    for selector_str in main_selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            if let Some(element) = document.select(&selector).next() {
                extract_element_text(&element, &mut text_parts);
                found_main = true;
                break;
            }
        }
    }

    if !found_main {
        // Fall back to extracting from root
        if let Ok(selector) = Selector::parse("html") {
            if let Some(element) = document.select(&selector).next() {
                extract_element_text(&element, &mut text_parts);
            }
        }
    }

    // Join and clean up
    let text = text_parts.join(" ");
    clean_html_text(&text)
}

fn extract_element_text(element: &scraper::ElementRef, parts: &mut Vec<String>) {
    // Skip script and style elements
    let tag_name = element.value().name();
    if tag_name == "script" || tag_name == "style" || tag_name == "noscript" {
        return;
    }

    for node in element.children() {
        if let Some(text) = node.value().as_text() {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                parts.push(trimmed.to_string());
            }
        } else if let Some(child_element) = scraper::ElementRef::wrap(node) {
            extract_element_text(&child_element, parts);

            // Add newlines after block elements
            let child_tag = child_element.value().name();
            if matches!(
                child_tag,
                "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li" | "br" | "tr"
            ) {
                parts.push("\n".to_string());
            }
        }
    }
}

fn clean_html_text(text: &str) -> String {
    // Decode HTML entities
    let text = text
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");

    // Normalize whitespace
    let text: String = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    // Restore paragraph breaks
    text.replace(" \n ", "\n\n")
        .replace("\n ", "\n")
        .replace(" \n", "\n")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_to_text() {
        let html = r#"
            <html>
                <head><title>Test</title></head>
                <body>
                    <h1>Hello World</h1>
                    <p>This is a <strong>test</strong> paragraph.</p>
                    <script>console.log('ignored');</script>
                </body>
            </html>
        "#;
        
        let text = html_to_text(html);
        assert!(text.contains("Hello World"));
        assert!(text.contains("test paragraph"));
        assert!(!text.contains("console.log"));
    }

    #[test]
    fn test_html_entities() {
        let html = "<p>Tom &amp; Jerry &lt;3</p>";
        let text = html_to_text(html);
        assert!(text.contains("Tom & Jerry <3"));
    }
}
