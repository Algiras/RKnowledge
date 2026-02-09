use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::parsing::parse_relations_json;
use super::prompts::{domain_aware_extraction_prompt, graph_extraction_user_prompt};
use super::{LlmProviderTrait, Relation};
use crate::config::DomainConfig;

pub struct OllamaProvider {
    client: Client,
    base_url: String,
    model: String,
}

#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<i32>,
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

impl OllamaProvider {
    pub fn new(base_url: &str, model: &str) -> Self {
        // Create client with optimized settings for local models
        let client = Client::builder()
            .timeout(Duration::from_secs(300)) // 5 min timeout for slow local models
            .connect_timeout(Duration::from_secs(10))
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(10) // Reuse connections for parallel requests
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
        }
    }

    async fn complete(&self, system: &str, user_message: &str) -> Result<String> {
        let request = OllamaChatRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system.to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: user_message.to_string(),
                },
            ],
            stream: false,
            options: OllamaOptions {
                // Small temperature for more focused output while avoiding repetition
                temperature: 0.1,
                // Let model decide when to stop (no forced token limit)
                num_predict: None,
                // Nucleus sampling for better quality
                top_p: Some(0.9),
                // Reduce vocabulary for faster sampling
                top_k: Some(40),
            },
        };

        let response = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context(
                "Failed to send request to Ollama API. Is Ollama running? (try: ollama serve)",
            )?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Ollama API error ({}): {}", status, error_text);
        }

        let response: OllamaChatResponse = response
            .json()
            .await
            .context("Failed to parse Ollama response")?;

        Ok(response.message.content)
    }
}

#[async_trait]
impl LlmProviderTrait for OllamaProvider {
    async fn extract_relations(
        &self,
        text: &str,
        domain: Option<&DomainConfig>,
    ) -> Result<Vec<Relation>> {
        let system_prompt = domain_aware_extraction_prompt(domain);
        let user_prompt = graph_extraction_user_prompt(text);
        let response = self.complete(&system_prompt, &user_prompt).await?;

        // Parse JSON response
        parse_relations_json(&response)
    }

    fn name(&self) -> &'static str {
        "ollama"
    }
}
