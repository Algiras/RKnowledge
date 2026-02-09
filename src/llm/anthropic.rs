use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::parsing::parse_relations_json;
use super::prompts::{domain_aware_extraction_prompt, graph_extraction_user_prompt};
use super::{LlmProviderTrait, Relation};
use crate::config::DomainConfig;

pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<Message>,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: Option<String>,
}

impl AnthropicProvider {
    pub fn new(api_key: &str, model: &str, base_url: Option<&str>) -> Result<Self> {
        if api_key.is_empty() {
            anyhow::bail!(
                "Anthropic API key is required. Set ANTHROPIC_API_KEY environment variable."
            );
        }

        Ok(Self {
            client: Client::new(),
            api_key: api_key.to_string(),
            model: model.to_string(),
            base_url: base_url
                .unwrap_or("https://api.anthropic.com")
                .trim_end_matches('/')
                .to_string(),
        })
    }

    async fn complete(&self, system: &str, user_message: &str) -> Result<String> {
        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            system: system.to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: user_message.to_string(),
            }],
        };

        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Anthropic API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error ({}): {}", status, error_text);
        }

        let response: AnthropicResponse = response
            .json()
            .await
            .context("Failed to parse Anthropic response")?;

        response
            .content
            .first()
            .and_then(|c| c.text.clone())
            .context("No text content in Anthropic response")
    }
}

#[async_trait]
impl LlmProviderTrait for AnthropicProvider {
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
        "anthropic"
    }
}

#[cfg(test)]
mod tests {
    use crate::llm::parsing;

    #[test]
    fn test_parse_relations_from_anthropic_style_response() {
        // Anthropic often returns clean JSON
        let json = r#"[{"node_1": "Test", "node_2": "Test2", "edge": "relates to"}]"#;
        let relations = parsing::parse_relations_json(json).unwrap();
        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].node_1, "test");
        assert_eq!(relations[0].node_2, "test2");
    }
}
