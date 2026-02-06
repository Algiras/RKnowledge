use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::parsing::parse_relations_json;
use super::prompts::{graph_extraction_user_prompt, GRAPH_EXTRACTION_SYSTEM_PROMPT};
use super::{LlmProviderTrait, Relation};

pub struct GoogleProvider {
    client: Client,
    api_key: String,
    model: String,
}

#[derive(Serialize)]
struct GoogleRequest {
    contents: Vec<Content>,
    #[serde(rename = "systemInstruction")]
    system_instruction: Option<SystemInstruction>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
}

#[derive(Serialize)]
struct SystemInstruction {
    parts: Vec<Part>,
}

#[derive(Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Serialize)]
struct Part {
    text: String,
}

#[derive(Serialize)]
struct GenerationConfig {
    temperature: f32,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: u32,
}

#[derive(Deserialize)]
struct GoogleResponse {
    candidates: Option<Vec<Candidate>>,
    error: Option<GoogleError>,
}

#[derive(Deserialize)]
struct Candidate {
    content: CandidateContent,
}

#[derive(Deserialize)]
struct CandidateContent {
    parts: Vec<ResponsePart>,
}

#[derive(Deserialize)]
struct ResponsePart {
    text: Option<String>,
}

#[derive(Deserialize)]
struct GoogleError {
    message: String,
}

impl GoogleProvider {
    pub fn new(api_key: &str, model: &str) -> Result<Self> {
        if api_key.is_empty() {
            anyhow::bail!("Google API key is required. Set GOOGLE_API_KEY environment variable.");
        }

        Ok(Self {
            client: Client::new(),
            api_key: api_key.to_string(),
            model: model.to_string(),
        })
    }

    async fn complete(&self, system: &str, user_message: &str) -> Result<String> {
        let request = GoogleRequest {
            contents: vec![Content {
                parts: vec![Part {
                    text: user_message.to_string(),
                }],
            }],
            system_instruction: Some(SystemInstruction {
                parts: vec![Part {
                    text: system.to_string(),
                }],
            }),
            generation_config: GenerationConfig {
                temperature: 0.0,
                max_output_tokens: 4096,
            },
        };

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
        );

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Google API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Google API error ({}): {}", status, error_text);
        }

        let response: GoogleResponse = response
            .json()
            .await
            .context("Failed to parse Google response")?;

        if let Some(error) = response.error {
            anyhow::bail!("Google API error: {}", error.message);
        }

        response
            .candidates
            .and_then(|c| c.into_iter().next())
            .and_then(|c| c.content.parts.into_iter().next())
            .and_then(|p| p.text)
            .context("No content in Google response")
    }
}

#[async_trait]
impl LlmProviderTrait for GoogleProvider {
    async fn extract_relations(&self, text: &str) -> Result<Vec<Relation>> {
        let user_prompt = graph_extraction_user_prompt(text);
        let response = self.complete(GRAPH_EXTRACTION_SYSTEM_PROMPT, &user_prompt).await?;

        // Parse JSON response
        parse_relations_json(&response)
    }

    fn name(&self) -> &'static str {
        "google"
    }
}

