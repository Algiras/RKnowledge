use anyhow::{Context, Result};
use std::path::Path;

/// Extract text from a plain text file
pub fn extract_text(path: &Path) -> Result<String> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read text file: {}", path.display()))?;

    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_extract_text() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Hello, World!").unwrap();
        
        let text = extract_text(file.path()).unwrap();
        assert!(text.contains("Hello, World!"));
    }
}
