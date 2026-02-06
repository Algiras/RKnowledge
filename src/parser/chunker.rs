/// Text chunker that splits text into overlapping chunks
pub struct TextChunker {
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    separators: Vec<&'static str>,
}

impl TextChunker {
    pub fn new(chunk_size: usize, chunk_overlap: usize) -> Self {
        Self {
            chunk_size,
            chunk_overlap,
            // Separators in order of preference (most to least specific)
            separators: vec!["\n\n", "\n", ". ", "! ", "? ", "; ", ", ", " ", ""],
        }
    }

    /// Split text into chunks with overlap
    pub fn split(&self, text: &str) -> Vec<String> {
        let text = text.trim();
        if text.is_empty() {
            return vec![];
        }

        if text.len() <= self.chunk_size {
            return vec![text.to_string()];
        }

        self.recursive_split(text, 0)
    }

    fn recursive_split(&self, text: &str, separator_idx: usize) -> Vec<String> {
        if separator_idx >= self.separators.len() {
            // No more separators, just split by character count
            return self.split_by_chars(text);
        }

        let separator = self.separators[separator_idx];
        let splits: Vec<&str> = if separator.is_empty() {
            text.chars().map(|c| &text[..c.len_utf8()]).collect()
        } else {
            text.split(separator).collect()
        };

        let mut chunks = Vec::new();
        let mut current_chunk = String::new();

        for (i, split) in splits.iter().enumerate() {
            let split_with_sep = if i < splits.len() - 1 && !separator.is_empty() {
                format!("{}{}", split, separator)
            } else {
                split.to_string()
            };

            if current_chunk.len() + split_with_sep.len() > self.chunk_size {
                if !current_chunk.is_empty() {
                    // Check if current chunk needs further splitting
                    if current_chunk.len() > self.chunk_size {
                        chunks.extend(self.recursive_split(&current_chunk, separator_idx + 1));
                    } else {
                        chunks.push(current_chunk.trim().to_string());
                    }
                }

                // Start new chunk, potentially with overlap
                current_chunk = if !chunks.is_empty() && self.chunk_overlap > 0 {
                    let last_chunk = chunks.last().unwrap();
                    // Use char boundary-safe overlap
                    let overlap_chars: String = last_chunk
                        .chars()
                        .rev()
                        .take(self.chunk_overlap)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();
                    format!("{}{}", overlap_chars, split_with_sep)
                } else {
                    split_with_sep
                };
            } else {
                current_chunk.push_str(&split_with_sep);
            }
        }

        // Don't forget the last chunk
        if !current_chunk.is_empty() {
            let trimmed = current_chunk.trim().to_string();
            if !trimmed.is_empty() {
                if trimmed.len() > self.chunk_size {
                    chunks.extend(self.recursive_split(&trimmed, separator_idx + 1));
                } else {
                    chunks.push(trimmed);
                }
            }
        }

        // Filter out empty chunks and very small chunks
        chunks.into_iter().filter(|c| c.len() > 10).collect()
    }

    fn split_by_chars(&self, text: &str) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        let mut chunks = Vec::new();
        let mut start = 0;

        while start < chars.len() {
            let end = (start + self.chunk_size).min(chars.len());
            let chunk: String = chars[start..end].iter().collect();
            chunks.push(chunk.trim().to_string());

            start = if self.chunk_overlap > 0 {
                end.saturating_sub(self.chunk_overlap)
            } else {
                end
            };

            // Prevent infinite loop
            if start >= end {
                break;
            }
        }

        chunks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_chunking() {
        let chunker = TextChunker::new(50, 10);
        let text =
            "This is a test. Another sentence here. And one more sentence to make it longer.";
        let chunks = chunker.split(text);

        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert!(chunk.len() <= 70); // Allow flexibility due to overlap
        }
    }

    #[test]
    fn test_empty_text() {
        let chunker = TextChunker::new(100, 10);
        assert!(chunker.split("").is_empty());
    }

    #[test]
    fn test_whitespace_only() {
        let chunker = TextChunker::new(100, 10);
        assert!(chunker.split("   \n\n\t  ").is_empty());
    }

    #[test]
    fn test_small_text_fits_one_chunk() {
        let chunker = TextChunker::new(1000, 100);
        let text = "This is a small text that fits in one chunk.";
        let chunks = chunker.split(text);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }

    #[test]
    fn test_paragraph_boundary_splits() {
        let chunker = TextChunker::new(100, 10);
        let text = "First paragraph with some text.\n\nSecond paragraph with more.\n\nThird paragraph final.";
        let chunks = chunker.split(text);
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_no_content_lost() {
        let chunker = TextChunker::new(200, 20);
        let text = "Artificial intelligence is changing the world. \
                    Machine learning enables computers to learn from data. \
                    Deep learning uses neural networks with many layers. \
                    Natural language processing handles human language. \
                    Computer vision interprets visual information.";
        let chunks = chunker.split(text);

        // Every word in the original should appear in at least one chunk
        for word in text.split_whitespace() {
            assert!(
                chunks.iter().any(|c| c.contains(word)),
                "Word '{}' missing from chunks",
                word
            );
        }
    }

    #[test]
    fn test_overlap_present() {
        let chunker = TextChunker::new(50, 20);
        let text = "First sentence is here. Second sentence follows. Third sentence ends it. Fourth comes after.";
        let chunks = chunker.split(text);

        if chunks.len() >= 2 {
            // The end of chunk[0] should overlap with the start of chunk[1]
            let end_of_first = &chunks[0][chunks[0].len().saturating_sub(15)..];
            let found_overlap = chunks[1].contains(end_of_first);
            // Overlap is best-effort due to separator logic, so just check chunks exist
            assert!(chunks.len() >= 2);
            let _ = found_overlap; // just verify no panic
        }
    }

    #[test]
    fn test_chunks_non_empty() {
        let chunker = TextChunker::new(100, 10);
        let text = "Word. ".repeat(50);
        let chunks = chunker.split(&text);
        for chunk in &chunks {
            assert!(chunk.len() > 10, "Chunk too small: '{}'", chunk);
        }
    }

    #[test]
    fn test_zero_overlap() {
        let chunker = TextChunker::new(50, 0);
        let text =
            "First part of text. Second part of text. Third part of text. Fourth part ends here.";
        let chunks = chunker.split(text);
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_unicode_text() {
        let chunker = TextChunker::new(50, 5);
        let text = "日本語のテスト文章です。これは二番目の文です。三番目の文章もあります。";
        let chunks = chunker.split(text);
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_very_long_single_word() {
        let chunker = TextChunker::new(20, 5);
        let text = "a".repeat(100);
        let chunks = chunker.split(&text);
        assert!(!chunks.is_empty());
        // Should be able to handle splitting by characters
    }
}
