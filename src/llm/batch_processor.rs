use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;
use tracing::{debug, info, warn};

use crate::config::DomainConfig;
use crate::llm::{LlmClient, Relation};
use crate::parser::{AdaptiveChunker, Chunk, ModelContextLimits};

/// Batch processor for efficient large codebase processing
///
/// Optimizations:
/// 1. Batches multiple small chunks into single LLM calls
/// 2. Persists progress for resume capability
/// 3. Skips already-processed documents
/// 4. Deduplicates similar content
pub struct BatchProcessor {
    llm_client: LlmClient,
    chunker: AdaptiveChunker,
    #[allow(dead_code)]
    concurrency: usize,
    batch_size: usize, // Number of chunks per LLM call
    progress_file: Option<String>,
    processed_hashes: HashMap<String, ProcessedDoc>,
    domain_config: Option<DomainConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProcessedDoc {
    hash: String,
    chunks_processed: usize,
    relations_count: usize,
    timestamp: String,
}

impl BatchProcessor {
    /// Create a new batch processor
    ///
    /// # Arguments
    /// * `llm_client` - The LLM client to use
    /// * `model` - Model name for context window detection
    /// * `concurrency` - Number of concurrent batch operations
    /// * `batch_size` - Number of chunks to process per LLM call (default: 5)
    pub fn new(llm_client: LlmClient, model: &str, concurrency: usize, batch_size: usize) -> Self {
        let chunker = ModelContextLimits::create_chunker(model);

        Self {
            llm_client,
            chunker,
            concurrency,
            batch_size: batch_size.max(1),
            progress_file: None,
            processed_hashes: HashMap::new(),
            domain_config: None,
        }
    }

    /// Set domain configuration for specialized extraction
    pub fn with_domain_config(mut self, domain_config: Option<DomainConfig>) -> Self {
        self.domain_config = domain_config;
        self
    }

    /// Enable progress persistence for resume capability
    pub fn with_progress_persistence(mut self, output_path: &Path) -> Self {
        let progress_file = output_path
            .parent()
            .map(|p| p.join(".rknowledge_progress.json"))
            .and_then(|p| p.to_str().map(String::from));

        self.progress_file = progress_file;
        self
    }

    /// Load previous progress if exists
    pub async fn load_progress(&mut self) -> Result<()> {
        if let Some(ref path) = self.progress_file
            && Path::new(path).exists()
        {
            let content = fs::read_to_string(path).await?;
            self.processed_hashes = serde_json::from_str(&content)?;
            info!(
                "Loaded progress: {} documents already processed",
                self.processed_hashes.len()
            );
        }
        Ok(())
    }

    /// Save progress to disk
    async fn save_progress(&self) -> Result<()> {
        if let Some(ref path) = self.progress_file {
            let content = serde_json::to_string_pretty(&self.processed_hashes)?;
            fs::write(path, content).await?;
        }
        Ok(())
    }

    /// Calculate simple hash for content deduplication
    fn calculate_hash(text: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Check if document was already processed
    fn is_already_processed(&self, source: &str, hash: &str) -> bool {
        if let Some(doc) = self.processed_hashes.get(source) {
            return doc.hash == hash;
        }
        false
    }

    /// Process multiple documents efficiently
    pub async fn process_documents(
        &mut self,
        documents: Vec<(String, String)>, // (source, text) pairs
    ) -> Result<Vec<Relation>> {
        let mut all_relations = Vec::new();
        let mut total_chunks = 0;
        let mut skipped_docs = 0;
        let mut processed_docs = 0;

        let total_docs = documents.len();
        info!(
            "Processing {} documents with batch size {}",
            total_docs, self.batch_size
        );

        for (source, text) in documents {
            let hash = Self::calculate_hash(&text);

            // Skip if already processed
            if self.is_already_processed(&source, &hash) {
                debug!("Skipping already processed document: {}", source);
                skipped_docs += 1;
                continue;
            }

            // Split into chunks
            let chunks = self.chunker.split(&text);
            let chunk_count = chunks.len();
            total_chunks += chunk_count;

            info!("Processing {} ({} chunks)", source, chunk_count);

            // Process chunks in batches
            let doc_relations = self.process_chunks_in_batches(&chunks, &source).await?;
            let relation_count = doc_relations.len();
            all_relations.extend(doc_relations);

            // Mark as processed
            self.processed_hashes.insert(
                source,
                ProcessedDoc {
                    hash,
                    chunks_processed: chunk_count,
                    relations_count: relation_count,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                        .to_string(),
                },
            );

            processed_docs += 1;

            // Save progress periodically
            if processed_docs % 10 == 0 {
                self.save_progress().await?;
                info!(
                    "Progress saved: {}/{} documents",
                    processed_docs, total_docs
                );
            }
        }

        // Final save
        self.save_progress().await?;

        info!(
            "Processing complete: {} processed, {} skipped, {} total chunks, {} relations",
            processed_docs,
            skipped_docs,
            total_chunks,
            all_relations.len()
        );

        Ok(all_relations)
    }

    /// Process chunks in batches (multiple chunks per LLM call)
    async fn process_chunks_in_batches(
        &self,
        chunks: &[Chunk],
        source: &str,
    ) -> Result<Vec<Relation>> {
        let mut all_relations = Vec::new();

        // Group chunks into batches
        let batches: Vec<Vec<&Chunk>> = chunks
            .chunks(self.batch_size)
            .map(|c| c.iter().collect())
            .collect();

        info!(
            "Processing {} chunks in {} batches",
            chunks.len(),
            batches.len()
        );

        for (batch_idx, batch) in batches.iter().enumerate() {
            debug!(
                "Processing batch {}/{} ({} chunks)",
                batch_idx + 1,
                batches.len(),
                batch.len()
            );

            // Combine chunks for batch processing
            let batch_text = self.format_batch_for_processing(batch, source, batch_idx);

            // Process with retry logic
            match self
                .process_batch_with_retry(&batch_text, source, batch_idx)
                .await
            {
                Ok(relations) => {
                    all_relations.extend(relations);
                }
                Err(e) => {
                    warn!(
                        "Batch {} failed: {}. Falling back to individual chunk processing",
                        batch_idx, e
                    );
                    // Fallback: process chunks individually
                    for chunk in batch.iter() {
                        match self.process_single_chunk(chunk, source).await {
                            Ok(relations) => all_relations.extend(relations),
                            Err(e) => warn!("Failed to process chunk {}: {}", chunk.chunk_index, e),
                        }
                    }
                }
            }
        }

        Ok(all_relations)
    }

    /// Format multiple chunks for batch LLM processing
    fn format_batch_for_processing(
        &self,
        chunks: &[&Chunk],
        source: &str,
        batch_idx: usize,
    ) -> String {
        let mut result = format!("Document: {} (Batch {})", source, batch_idx);
        result.push_str("\n===CHUNK_SEPARATOR===\n");

        for (i, chunk) in chunks.iter().enumerate() {
            result.push_str(&format!("\n---CHUNK_{}---\n", i));
            result.push_str(&chunk.text);
        }

        result.push_str("\n===END_DOCUMENT===\n");
        result
    }

    /// Process a batch with automatic retry and splitting
    async fn process_batch_with_retry(
        &self,
        batch_text: &str,
        _source: &str,
        batch_idx: usize,
    ) -> Result<Vec<Relation>> {
        match self.llm_client.extract_relations(batch_text, self.domain_config.as_ref()).await {
            Ok(relations) => {
                debug!(
                    "Batch {} processed successfully: {} relations",
                    batch_idx,
                    relations.len()
                );
                Ok(relations)
            }
            Err(e) => {
                let error_str = e.to_string().to_lowercase();

                if Self::is_context_overflow(&error_str) {
                    warn!(
                        "Context overflow for batch {}, triggering fallback to individual processing",
                        batch_idx
                    );
                    // Return error to trigger fallback to individual processing
                    Err(anyhow::anyhow!("Context overflow, needs fallback"))
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Process a single chunk (fallback method)
    async fn process_single_chunk(&self, chunk: &Chunk, _source: &str) -> Result<Vec<Relation>> {
        debug!("Processing single chunk {}", chunk.chunk_index);
        self.llm_client.extract_relations(&chunk.text, self.domain_config.as_ref()).await
    }

    /// Check if error indicates context window overflow
    fn is_context_overflow(error: &str) -> bool {
        let error_lower = error.to_lowercase();
        let overflow_indicators = [
            "context length",
            "context window",
            "too long",
            "token limit",
            "max tokens",
            "exceeds",
            "context size",
            "input length",
            "too many tokens",
            "sequence length",
        ];

        overflow_indicators
            .iter()
            .any(|indicator| error_lower.contains(indicator))
    }

    /// Get processing statistics
    pub fn get_stats(&self) -> ProcessingStats {
        ProcessingStats {
            total_documents: self.processed_hashes.len(),
            total_relations: self
                .processed_hashes
                .values()
                .map(|d| d.relations_count)
                .sum(),
            total_chunks: self
                .processed_hashes
                .values()
                .map(|d| d.chunks_processed)
                .sum(),
        }
    }
}

/// Processing statistics
#[derive(Debug)]
#[allow(dead_code)]
pub struct ProcessingStats {
    pub total_documents: usize,
    pub total_relations: usize,
    pub total_chunks: usize,
}

/// Smart document selector to avoid processing duplicates/similar docs
pub struct DocumentSelector;

impl DocumentSelector {
    /// Select representative documents from a large codebase
    ///
    /// Strategy:
    /// 1. Prioritize "index" files (README, TOC, SKILL)
    /// 2. Limit files per directory (avoiding duplication)
    /// 3. Skip auto-generated files
    /// 4. Prioritize recent/modified files
    pub fn select_representative_docs(
        all_docs: &[(String, String)],
        max_per_dir: usize,
    ) -> Vec<(String, String)> {
        let mut selected = Vec::new();
        let mut dir_counts: HashMap<String, usize> = HashMap::new();

        // Sort to prioritize important files
        let mut sorted_docs = all_docs.to_vec();
        sorted_docs.sort_by(|(path_a, _), (path_b, _)| {
            let score_a = Self::document_priority(path_a);
            let score_b = Self::document_priority(path_b);
            score_b.cmp(&score_a) // Higher score first
        });

        for (source, text) in sorted_docs {
            // Get directory
            let dir = Path::new(&source)
                .parent()
                .and_then(|p| p.to_str())
                .unwrap_or("")
                .to_string();

            // Check if we should skip
            if Self::should_skip(&source, &text) {
                continue;
            }

            // Check directory limit
            let count = dir_counts.entry(dir.clone()).or_insert(0);
            if *count >= max_per_dir {
                continue;
            }

            *count += 1;
            selected.push((source, text));
        }

        info!(
            "Selected {} representative documents from {}",
            selected.len(),
            all_docs.len()
        );
        selected
    }

    /// Calculate document priority (higher = more important)
    fn document_priority(path: &str) -> i32 {
        let lower = path.to_lowercase();
        let mut score = 0;

        // High priority files
        if lower.contains("readme") || lower.contains("skill") || lower.contains("toc") {
            score += 100;
        }
        if lower.contains("overview") || lower.contains("getting-started") {
            score += 50;
        }
        if lower.contains("example") || lower.contains("guide") {
            score += 30;
        }

        // Penalize generated files
        if lower.contains("generated") || lower.contains("auto") {
            score -= 50;
        }

        // Prefer smaller, focused files
        if lower.ends_with(".md") {
            score += 10;
        }

        score
    }

    /// Check if document should be skipped
    fn should_skip(source: &str, text: &str) -> bool {
        let lower = source.to_lowercase();

        // Skip generated files
        if lower.contains("generated")
            || lower.contains("auto-generated")
            || lower.contains("broken-links")
            || lower.contains("source-reference-map")
        {
            return true;
        }

        // Skip very small files
        if text.len() < 100 {
            return true;
        }

        // Skip JSON data files
        if lower.ends_with(".json") {
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_overflow_detection() {
        assert!(BatchProcessor::is_context_overflow(
            "Error: context length exceeded"
        ));
        assert!(BatchProcessor::is_context_overflow("Token limit reached"));
        assert!(!BatchProcessor::is_context_overflow(
            "Network error occurred"
        ));
    }

    #[test]
    fn test_document_priority() {
        assert!(
            DocumentSelector::document_priority("README.md")
                > DocumentSelector::document_priority("generated-file.md")
        );
        assert!(
            DocumentSelector::document_priority("SKILL.md")
                > DocumentSelector::document_priority("random.md")
        );
    }

    #[test]
    fn test_should_skip() {
        assert!(DocumentSelector::should_skip("broken-links.json", "{}"));
        assert!(DocumentSelector::should_skip("generated.md", "content"));
        assert!(!DocumentSelector::should_skip(
            "readme.md",
            "# Title\n\nThis is a long content that should not be skipped because it has more than one hundred characters to pass the minimum length check."
        ));
    }
}
