use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::parsing::parse_relations_json;
use super::prompts::{graph_extraction_user_prompt, GRAPH_EXTRACTION_SYSTEM_PROMPT};
use super::{LlmProviderTrait, Relation};

pub struct OllamaProvider {
    client: Client,
    base_url: String,
    model: String,
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    system: String,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
    num_predict: i32,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

impl OllamaProvider {
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
        }
    }

    async fn complete(&self, system: &str, user_message: &str) -> Result<String> {
        let request = OllamaRequest {
            model: self.model.clone(),
            prompt: user_message.to_string(),
            system: system.to_string(),
            stream: false,
            options: OllamaOptions {
                temperature: 0.0,
                num_predict: 4096,
            },
        };

        let response = self
            .client
            .post(format!("{}/api/generate", self.base_url))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Ollama API. Is Ollama running?")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Ollama API error ({}): {}", status, error_text);
        }

        let response: OllamaResponse = response
            .json()
            .await
            .context("Failed to parse Ollama response")?;

        Ok(response.response)
    }
}

#[async_trait]
impl LlmProviderTrait for OllamaProvider {
    async fn extract_relations(&self, text: &str) -> Result<Vec<Relation>> {
        let user_prompt = graph_extraction_user_prompt(text);
        let response = self.complete(GRAPH_EXTRACTION_SYSTEM_PROMPT, &user_prompt).await?;

        // Parse JSON response
        parse_relations_json(&response)
    }

    fn name(&self) -> &'static str {
        "ollama"
    }
}

