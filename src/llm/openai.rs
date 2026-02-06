use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::parsing::parse_relations_json;
use super::prompts::{graph_extraction_user_prompt, GRAPH_EXTRACTION_SYSTEM_PROMPT};
use super::{LlmProviderTrait, Relation};

pub struct OpenAIProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAIResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: Option<String>,
}

impl OpenAIProvider {
    pub fn new(api_key: &str, model: &str, base_url: Option<&str>) -> Result<Self> {
        if api_key.is_empty() {
            anyhow::bail!("OpenAI API key is required. Set OPENAI_API_KEY environment variable.");
        }

        Ok(Self {
            client: Client::new(),
            api_key: api_key.to_string(),
            model: model.to_string(),
            base_url: base_url
                .unwrap_or("https://api.openai.com/v1")
                .to_string(),
        })
    }

    async fn complete(&self, system: &str, user_message: &str) -> Result<String> {
        let request = OpenAIRequest {
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
            max_tokens: 4096,
            temperature: 0.0,
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to OpenAI API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI API error ({}): {}", status, error_text);
        }

        let response: OpenAIResponse = response
            .json()
            .await
            .context("Failed to parse OpenAI response")?;

        response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .context("No content in OpenAI response")
    }
}

#[async_trait]
impl LlmProviderTrait for OpenAIProvider {
    async fn extract_relations(&self, text: &str) -> Result<Vec<Relation>> {
        let user_prompt = graph_extraction_user_prompt(text);
        let response = self.complete(GRAPH_EXTRACTION_SYSTEM_PROMPT, &user_prompt).await?;

        // Parse JSON response
        parse_relations_json(&response)
    }

    fn name(&self) -> &'static str {
        "openai"
    }
}

