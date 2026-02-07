use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};

use crate::llm::{LlmClient, Relation};
use crate::parser::{AdaptiveChunker, Chunk, ModelContextLimits};

/// Processor that handles context overflow with automatic retry
pub struct AdaptiveProcessor {
    llm_client: Arc<LlmClient>,
    chunker: AdaptiveChunker,
    max_retries: u32,
    concurrency: usize,
}

impl AdaptiveProcessor {
    /// Create a new adaptive processor for a specific model
    pub fn new(llm_client: LlmClient, model: &str, concurrency: usize) -> Self {
        let chunker = ModelContextLimits::create_chunker(model);
        
        Self {
            llm_client: Arc::new(llm_client),
            chunker,
            max_retries: 3,
            concurrency,
        }
    }

    /// Create with custom chunk size
    pub fn with_chunk_size(
        llm_client: LlmClient,
        target_tokens: usize,
        overlap_tokens: usize,
        concurrency: usize,
    ) -> Self {
        Self {
            llm_client: Arc::new(llm_client),
            chunker: AdaptiveChunker::new(target_tokens, overlap_tokens),
            max_retries: 3,
            concurrency,
        }
    }

    /// Process text with automatic overflow handling
    pub async fn process(&self, text: &str, source: &str) -> Result<Vec<Relation>> {
        let chunks = self.chunker.split(text);
        info!("Split text into {} chunks for {}", chunks.len(), source);

        let semaphore = Arc::new(Semaphore::new(self.concurrency));
        let mut tasks = Vec::new();

        for chunk in chunks {
            let permit = semaphore.clone().acquire_owned().await?;
            let client = self.llm_client.clone();
            let source = source.to_string();
            
            let task = tokio::spawn(async move {
                let _permit = permit; // Hold permit until task completes
                Self::process_chunk_with_retry(&client, chunk, &source).await
            });
            
            tasks.push(task);
        }

        let mut all_relations = Vec::new();
        let mut errors = Vec::new();

        for task in tasks {
            match task.await {
                Ok(Ok(relations)) => all_relations.extend(relations),
                Ok(Err(e)) => errors.push(e.to_string()),
                Err(e) => errors.push(format!("Task panicked: {}", e)),
            }
        }

        if !errors.is_empty() && all_relations.is_empty() {
            return Err(anyhow::anyhow!(
                "All chunks failed. Errors: {}",
                errors.join("; ")
            ));
        }

        if !errors.is_empty() {
            warn!("Some chunks failed: {}", errors.join("; "));
        }

        Ok(all_relations)
    }

    /// Process a single chunk with retry on overflow
    async fn process_chunk_with_retry(
        client: &LlmClient,
        chunk: Chunk,
        source: &str,
    ) -> Result<Vec<Relation>> {
        let mut current_chunk = chunk;
        let mut attempt = 0;
        let max_attempts = 3;

        loop {
            debug!(
                "Processing chunk {} from {} ({} tokens, attempt {})",
                current_chunk.chunk_index,
                source,
                current_chunk.estimated_tokens,
                attempt + 1
            );

            match client.extract_relations(&current_chunk.text).await {
                Ok(relations) => {
                    return Ok(relations);
                }
                Err(e) => {
                    let error_str = e.to_string().to_lowercase();
                    
                    // Check if it's a context overflow error
                    if Self::is_context_overflow(&error_str) {
                        attempt += 1;
                        
                        if attempt >= max_attempts {
                            return Err(anyhow::anyhow!(
                                "Context overflow after {} retries for chunk {} from {}: {}",
                                max_attempts,
                                current_chunk.chunk_index,
                                source,
                                e
                            ));
                        }

                        warn!(
                            "Context overflow for chunk {} from {}, retrying with smaller size (attempt {})",
                            current_chunk.chunk_index,
                            source,
                            attempt
                        );

                        // Reduce target size by 50% and re-chunk
                        let new_target = current_chunk.estimated_tokens / 2;
                        let sub_chunks = AdaptiveChunker::new(new_target, new_target / 10)
                            .split_with_target(&current_chunk.text, new_target);

                        if sub_chunks.is_empty() {
                            return Err(anyhow::anyhow!(
                                "Cannot split chunk {} further",
                                current_chunk.chunk_index
                            ));
                        }

                        // Process first sub-chunk (others are discarded on this retry)
                        if let Some(first_sub) = sub_chunks.first() {
                            current_chunk = Chunk {
                                text: first_sub.text.clone(),
                                estimated_tokens: first_sub.estimated_tokens,
                                chunk_index: current_chunk.chunk_index,
                                parent_id: Some(format!(
                                    "{}-retry-{}",
                                    current_chunk.chunk_index,
                                    attempt
                                )),
                            };
                        }
                        
                        // Continue to next attempt
                        continue;
                    }

                    // Not a context overflow, return the error
                    return Err(e);
                }
            }
        }
    }

    /// Check if error indicates context window overflow
    pub fn is_context_overflow(error: &str) -> bool {
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

        overflow_indicators.iter().any(|indicator| error_lower.contains(indicator))
    }
}

/// Builder for hierarchical processing
/// 
/// Processes chunks, then creates summary nodes to link related chunks
pub struct HierarchicalProcessor {
    processor: AdaptiveProcessor,
    batch_size: usize,
}

impl HierarchicalProcessor {
    pub fn new(processor: AdaptiveProcessor, batch_size: usize) -> Self {
        Self {
            processor,
            batch_size,
        }
    }

    /// Process in batches and create hierarchical links
    pub async fn process_hierarchical(
        &self,
        text: &str,
        source: &str,
    ) -> Result<HierarchicalResult> {
        let chunks = self.processor.chunker.split(text);
        let mut all_relations = Vec::new();
        let mut batch_summaries = Vec::new();

        // Process in batches
        for (batch_idx, batch) in chunks.chunks(self.batch_size).enumerate() {
            let batch_text: String = batch
                .iter()
                .map(|c| c.text.clone())
                .collect::<Vec<_>>()
                .join("\n\n");

            let batch_relations = self.processor.process(&batch_text, source).await?;
            
            // Create a batch summary node
            let summary_node = format!("{}_batch_{}", source, batch_idx);
            
            // Link chunks in this batch to the summary
            for chunk in batch {
                all_relations.push(Relation {
                    node_1: summary_node.clone(),
                    node_1_type: Some("BatchSummary".to_string()),
                    node_2: format!("{}_chunk_{}", source, chunk.chunk_index),
                    node_2_type: Some("Chunk".to_string()),
                    edge: "contains".to_string(),
                });
            }

            batch_summaries.push(summary_node);
            all_relations.extend(batch_relations);
        }

        // Link batches together
        for i in 0..batch_summaries.len().saturating_sub(1) {
            all_relations.push(Relation {
                node_1: batch_summaries[i].clone(),
                node_1_type: Some("BatchSummary".to_string()),
                node_2: batch_summaries[i + 1].clone(),
                node_2_type: Some("BatchSummary".to_string()),
                edge: "followed_by".to_string(),
            });
        }

        Ok(HierarchicalResult {
            relations: all_relations,
            batch_count: batch_summaries.len(),
            total_chunks: chunks.len(),
        })
    }
}

/// Result from hierarchical processing
#[derive(Debug)]
pub struct HierarchicalResult {
    pub relations: Vec<Relation>,
    pub batch_count: usize,
    pub total_chunks: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_overflow_detection() {
        assert!(AdaptiveProcessor::is_context_overflow(
            "Error: context length exceeded"
        ));
        assert!(AdaptiveProcessor::is_context_overflow(
            "The input is too long for the model"
        ));
        assert!(AdaptiveProcessor::is_context_overflow(
            "Token limit reached"
        ));
        assert!(!AdaptiveProcessor::is_context_overflow(
            "Network error occurred"
        ));
    }
}
