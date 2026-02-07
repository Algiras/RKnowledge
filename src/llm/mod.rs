pub mod adaptive_processor;
pub mod batch_processor;
mod anthropic;
mod google;
mod ollama;
mod openai;
pub(crate) mod parsing;
mod prompts;

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::cli::LlmProvider;
use crate::config::Config;

/// A relation extracted from text
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relation {
    pub node_1: String,
    #[serde(default)]
    pub node_1_type: Option<String>,
    pub node_2: String,
    #[serde(default)]
    pub node_2_type: Option<String>,
    pub edge: String,
}

/// Trait for LLM providers
#[async_trait]
pub trait LlmProviderTrait: Send + Sync {
    /// Extract relations from text using the LLM
    async fn extract_relations(&self, text: &str) -> Result<Vec<Relation>>;

    /// Get the provider name
    #[allow(dead_code)]
    fn name(&self) -> &'static str;
}

/// Main LLM client that abstracts over providers
pub struct LlmClient {
    provider: Box<dyn LlmProviderTrait>,
}

impl LlmClient {
    /// Create a new LLM client for the specified provider
    pub fn new(
        provider: LlmProvider,
        config: &Config,
        model_override: Option<&str>,
    ) -> Result<Self> {
        let provider_impl: Box<dyn LlmProviderTrait> = match provider {
            LlmProvider::Anthropic => {
                let provider_config = config
                    .get_provider("anthropic")
                    .context("Anthropic provider not configured")?;
                let model = model_override
                    .map(String::from)
                    .or_else(|| provider_config.model.clone())
                    .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());
                Box::new(anthropic::AnthropicProvider::new(
                    &provider_config.api_key,
                    &model,
                    provider_config.base_url.as_deref(),
                )?)
            }
            LlmProvider::OpenAI => {
                let provider_config = config
                    .get_provider("openai")
                    .context("OpenAI provider not configured")?;
                let model = model_override
                    .map(String::from)
                    .or_else(|| provider_config.model.clone())
                    .unwrap_or_else(|| "gpt-4o".to_string());
                Box::new(openai::OpenAIProvider::new(
                    &provider_config.api_key,
                    &model,
                    provider_config.base_url.as_deref(),
                )?)
            }
            LlmProvider::Ollama => {
                let provider_config = config
                    .get_provider("ollama")
                    .context("Ollama provider not configured")?;
                let model = model_override
                    .map(String::from)
                    .or_else(|| provider_config.model.clone())
                    .unwrap_or_else(|| "mistral".to_string());
                let base_url = provider_config
                    .base_url
                    .as_deref()
                    .unwrap_or("http://localhost:11434");
                Box::new(ollama::OllamaProvider::new(base_url, &model))
            }
            LlmProvider::Google => {
                let provider_config = config
                    .get_provider("google")
                    .context("Google provider not configured")?;
                let model = model_override
                    .map(String::from)
                    .or_else(|| provider_config.model.clone())
                    .unwrap_or_else(|| "gemini-2.0-flash".to_string());
                Box::new(google::GoogleProvider::new(
                    &provider_config.api_key,
                    &model,
                    provider_config.base_url.as_deref(),
                )?)
            }
        };

        Ok(Self {
            provider: provider_impl,
        })
    }

    /// Extract relations from text
    pub async fn extract_relations(&self, text: &str) -> Result<Vec<Relation>> {
        self.provider.extract_relations(text).await
    }

    /// Get the provider name
    #[allow(dead_code)]
    pub fn provider_name(&self) -> &'static str {
        self.provider.name()
    }
}
