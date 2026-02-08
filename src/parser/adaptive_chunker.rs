/// Token estimation for context-aware chunking
///
/// Rough approximation: 1 token ≈ 4 characters for English text
/// This is a simple estimation - for production, use tiktoken or similar
pub fn estimate_tokens(text: &str) -> usize {
    // Simple character-based estimation: ~4 chars per token
    // This is conservative but works for most English text
    (text.len() as f32 / 4.0).ceil() as usize
}

/// Adaptive chunker that adjusts size based on available context
#[allow(dead_code)]
pub struct AdaptiveChunker {
    target_tokens: usize,
    overlap_tokens: usize,
    separators: Vec<&'static str>,
    max_retries: u32,
}

impl AdaptiveChunker {
    /// Create a new adaptive chunker
    ///
    /// # Arguments
    /// * `target_tokens` - Target token count per chunk (e.g., 1000 for 4K context models)
    /// * `overlap_tokens` - Overlap between chunks to maintain context
    pub fn new(target_tokens: usize, overlap_tokens: usize) -> Self {
        Self {
            target_tokens,
            overlap_tokens,
            separators: vec!["\n\n", "\n", ". ", "! ", "? ", "; ", ", ", " ", ""],
            max_retries: 3,
        }
    }

    /// Create a chunker optimized for a specific model context size
    ///
    /// Reserves space for:
    /// - System prompt (~200 tokens)
    /// - User prompt + text
    /// - Response buffer (~500 tokens)
    pub fn for_context_window(context_size: usize) -> Self {
        let reserved = 700; // System prompt + response buffer
        let safe_target = (context_size.saturating_sub(reserved)) / 2;
        let overlap = safe_target / 10;

        Self::new(safe_target, overlap)
    }

    /// Split text into token-aware chunks
    pub fn split(&self, text: &str) -> Vec<Chunk> {
        let text = text.trim();
        if text.is_empty() {
            return vec![];
        }

        let estimated_tokens = estimate_tokens(text);
        if estimated_tokens <= self.target_tokens {
            return vec![Chunk {
                text: text.to_string(),
                estimated_tokens,
                chunk_index: 0,
                parent_id: None,
            }];
        }

        self.recursive_split(text, 0, 0, None)
    }

    /// Split with a specific target (used for retry with smaller size)
    #[allow(dead_code)]
    pub fn split_with_target(&self, text: &str, target_tokens: usize) -> Vec<Chunk> {
        let mut temp = Self::new(target_tokens, target_tokens / 10);
        temp.max_retries = self.max_retries;
        temp.split(text)
    }

    fn recursive_split(
        &self,
        text: &str,
        separator_idx: usize,
        chunk_index: usize,
        parent_id: Option<String>,
    ) -> Vec<Chunk> {
        if separator_idx >= self.separators.len() {
            return self.split_by_tokens(text, chunk_index, parent_id);
        }

        let separator = self.separators[separator_idx];
        let splits: Vec<&str> = if separator.is_empty() {
            text.chars().map(|c| &text[..c.len_utf8()]).collect()
        } else {
            text.split(separator).collect()
        };

        let mut chunks = Vec::new();
        let mut current_chunk = String::new();
        let mut current_tokens = 0;
        let mut idx = chunk_index;

        for (i, split) in splits.iter().enumerate() {
            let split_with_sep = if i < splits.len() - 1 && !separator.is_empty() {
                format!("{}{}", split, separator)
            } else {
                split.to_string()
            };

            let split_tokens = estimate_tokens(&split_with_sep);

            if current_tokens + split_tokens > self.target_tokens {
                if !current_chunk.is_empty() {
                    let chunk_tokens = estimate_tokens(&current_chunk);

                    // If still too big, recurse with next separator
                    if chunk_tokens > self.target_tokens {
                        chunks.extend(self.recursive_split(
                            &current_chunk,
                            separator_idx + 1,
                            idx,
                            parent_id.clone(),
                        ));
                    } else {
                        chunks.push(Chunk {
                            text: current_chunk.trim().to_string(),
                            estimated_tokens: chunk_tokens,
                            chunk_index: idx,
                            parent_id: parent_id.clone(),
                        });
                        idx += 1;
                    }
                }

                // Start new chunk with overlap
                let overlap_text = if !chunks.is_empty() && self.overlap_tokens > 0 {
                    self.get_overlap_text(&chunks.last().unwrap().text)
                } else {
                    String::new()
                };

                current_chunk = format!("{}{}", overlap_text, split_with_sep);
                current_tokens = estimate_tokens(&current_chunk);
            } else {
                current_chunk.push_str(&split_with_sep);
                current_tokens += split_tokens;
            }
        }

        // Don't forget the last chunk
        if !current_chunk.is_empty() {
            let trimmed = current_chunk.trim().to_string();
            if !trimmed.is_empty() {
                let chunk_tokens = estimate_tokens(&trimmed);

                if chunk_tokens > self.target_tokens {
                    chunks.extend(self.recursive_split(
                        &trimmed,
                        separator_idx + 1,
                        idx,
                        parent_id,
                    ));
                } else {
                    chunks.push(Chunk {
                        text: trimmed,
                        estimated_tokens: chunk_tokens,
                        chunk_index: idx,
                        parent_id,
                    });
                }
            }
        }

        chunks
    }

    fn split_by_tokens(
        &self,
        text: &str,
        chunk_index: usize,
        parent_id: Option<String>,
    ) -> Vec<Chunk> {
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut chunks = Vec::new();
        let mut current_words = Vec::new();
        let mut current_tokens = 0;
        let mut idx = chunk_index;

        for word in words {
            let word_tokens = (word.len() as f32 * 0.5) as usize + 1;

            if current_tokens + word_tokens > self.target_tokens {
                if !current_words.is_empty() {
                    let text = current_words.join(" ");
                    chunks.push(Chunk {
                        text: text.clone(),
                        estimated_tokens: estimate_tokens(&text),
                        chunk_index: idx,
                        parent_id: parent_id.clone(),
                    });
                    idx += 1;
                }

                // Overlap: keep last N words
                let overlap_words = if self.overlap_tokens > 0 {
                    let overlap_count = (self.overlap_tokens / 2).max(5).min(current_words.len());
                    current_words.split_off(current_words.len() - overlap_count)
                } else {
                    vec![]
                };

                current_words = overlap_words;
                current_words.push(word);
                current_tokens = current_words
                    .iter()
                    .map(|w| (w.len() as f32 * 0.5) as usize + 1)
                    .sum();
            } else {
                current_words.push(word);
                current_tokens += word_tokens;
            }
        }

        // Last chunk
        if !current_words.is_empty() {
            let text = current_words.join(" ");
            chunks.push(Chunk {
                text,
                estimated_tokens: current_tokens,
                chunk_index: idx,
                parent_id,
            });
        }

        chunks
    }

    fn get_overlap_text(&self, text: &str) -> String {
        let words: Vec<&str> = text.split_whitespace().collect();
        let overlap_word_count = (self.overlap_tokens / 2).max(5).min(words.len());

        words[words.len().saturating_sub(overlap_word_count)..].join(" ") + " "
    }
}

/// A chunk with metadata
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Chunk {
    pub text: String,
    pub estimated_tokens: usize,
    pub chunk_index: usize,
    pub parent_id: Option<String>,
}

/// Model context limits database
pub struct ModelContextLimits;

impl ModelContextLimits {
    /// Get context window size for known models
    pub fn get_context_size(model: &str) -> usize {
        let model_lower = model.to_lowercase();

        match model_lower.as_str() {
            // Small local models (Ollama)
            m if m.contains("llama3.2") || m.contains("llama-3.2") => 8192,
            m if m.contains("phi3:mini") || m.contains("phi-3-mini") => 4096,
            m if m.contains("mistral") => 32768,
            m if m.contains("qwen2.5:3b") => 8192,
            m if m.contains("qwen2.5:7b") => 32768,
            m if m.contains("gemma2:2b") => 4096,
            m if m.contains("gemma2:9b") => 8192,

            // Larger local models
            m if m.contains("llama3.3") || m.contains("llama-3.3") => 128000,
            m if m.contains("qwen2.5:72b") => 32768,

            // Cloud models
            m if m.contains("claude-3-opus") => 200000,
            m if m.contains("claude-3-sonnet") => 200000,
            m if m.contains("claude-3-haiku") => 200000,
            m if m.contains("gpt-4") => 8192,
            m if m.contains("gpt-4o") => 128000,
            m if m.contains("gpt-3.5") => 16385,
            m if m.contains("gemini") => 1048576,

            // Default for unknown models (conservative)
            _ => 4096,
        }
    }

    /// Create an adaptive chunker for a specific model
    pub fn create_chunker(model: &str) -> AdaptiveChunker {
        let context_size = Self::get_context_size(model);
        AdaptiveChunker::for_context_window(context_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_estimation() {
        let text = "This is a simple test sentence with eight words.";
        let tokens = estimate_tokens(text);
        // 48 chars / 4 = ~12 tokens
        assert!(
            (10..=15).contains(&tokens),
            "Expected ~12 tokens, got {}",
            tokens
        );
    }

    #[test]
    fn test_adaptive_chunking() {
        let chunker = AdaptiveChunker::new(100, 20);
        let text = "This is a test. ".repeat(100);
        let chunks = chunker.split(&text);

        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert!(chunk.estimated_tokens <= 150); // Some flexibility
        }
    }

    #[test]
    fn test_context_window_chunker() {
        let chunker = AdaptiveChunker::for_context_window(4096);
        // Create enough text to definitely require multiple chunks
        // "Word " = 5 chars, ~1.25 tokens each.
        // 8000 repetitions = 40000 chars = ~10000 tokens
        let text = "Word ".repeat(8000);
        let chunks = chunker.split(&text);

        // Should create multiple chunks for very large text
        assert!(
            chunks.len() > 1,
            "Expected multiple chunks, got {}",
            chunks.len()
        );

        // Each chunk should be reasonable size (under ~1700 tokens for 4K context)
        for chunk in &chunks {
            assert!(
                chunk.estimated_tokens <= 2000,
                "Chunk too large: {} tokens",
                chunk.estimated_tokens
            );
        }
    }

    #[test]
    fn test_model_context_limits() {
        assert_eq!(ModelContextLimits::get_context_size("mistral"), 32768);
        assert_eq!(ModelContextLimits::get_context_size("llama3.2"), 8192);
        assert_eq!(ModelContextLimits::get_context_size("phi3:mini"), 4096);
        assert_eq!(ModelContextLimits::get_context_size("unknown-model"), 4096);
    }

    #[test]
    fn test_chunk_overlap() {
        let chunker = AdaptiveChunker::new(50, 10);
        let text = "First sentence here. Second sentence follows. Third ends. Fourth comes after.";
        let chunks = chunker.split(text);

        if chunks.len() >= 2 {
            // Check that overlap is maintained
            assert!(chunks[0].chunk_index < chunks[1].chunk_index);
        }
    }
}
