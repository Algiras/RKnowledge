use anyhow::{Context, Result};
use std::path::Path;

/// Extract text from a PDF file
pub fn extract_text(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("Failed to read PDF file: {}", path.display()))?;

    let text = pdf_extract::extract_text_from_mem(&bytes)
        .with_context(|| format!("Failed to extract text from PDF: {}", path.display()))?;

    // Clean up the extracted text
    let cleaned = clean_pdf_text(&text);

    Ok(cleaned)
}

/// Clean up extracted PDF text
fn clean_pdf_text(text: &str) -> String {
    text.lines()
        // Remove empty lines and whitespace-only lines
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        // Join with single newlines
        .collect::<Vec<_>>()
        .join("\n")
        // Normalize whitespace
        .replace("  ", " ")
        // Remove common PDF artifacts
        .replace("\u{0}", "")
        .replace("\u{FEFF}", "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_pdf_text() {
        let dirty = "  Hello  \n\n\n  World  \n  ";
        let clean = clean_pdf_text(dirty);
        assert_eq!(clean, "Hello\nWorld");
    }
}
