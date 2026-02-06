mod chunker;
mod pdf;
mod text;
mod markdown;
mod html;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

pub use chunker::TextChunker;

/// A document chunk with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub text: String,
    pub source: String,
    pub chunk_index: usize,
}

/// Parser for various document types
pub struct DocumentParser {
    chunker: TextChunker,
}

impl DocumentParser {
    pub fn new(chunk_size: usize, chunk_overlap: usize) -> Self {
        Self {
            chunker: TextChunker::new(chunk_size, chunk_overlap),
        }
    }

    /// Parse a document and return chunks
    pub fn parse(&self, path: &Path) -> Result<Vec<Document>> {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let text = match extension.as_str() {
            "pdf" => pdf::extract_text(path)?,
            "txt" => text::extract_text(path)?,
            "md" | "markdown" => markdown::extract_text(path)?,
            "html" | "htm" => html::extract_text(path)?,
            _ => anyhow::bail!("Unsupported file type: {}", extension),
        };

        let source = path.to_string_lossy().to_string();
        let chunks = self.chunker.split(&text);

        let documents: Vec<Document> = chunks
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| Document {
                id: Uuid::new_v4().to_string().replace("-", ""),
                text: chunk,
                source: source.clone(),
                chunk_index: i,
            })
            .collect();

        Ok(documents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_parser() {
        let parser = DocumentParser::new(100, 10);
        // Basic test that parser can be created
        assert!(parser.chunker.chunk_size == 100);
    }
}
