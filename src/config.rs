use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_provider")]
    pub default_provider: String,
    pub default_model: Option<String>,
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,
    #[serde(default = "default_chunk_overlap")]
    pub chunk_overlap: usize,
    pub providers: ProvidersConfig,
    pub neo4j: Neo4jConfig,
}

fn default_provider() -> String {
    "anthropic".to_string()
}

fn default_chunk_size() -> usize {
    1500
}

fn default_chunk_overlap() -> usize {
    150
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProvidersConfig {
    pub anthropic: Option<ProviderConfig>,
    pub openai: Option<ProviderConfig>,
    pub ollama: Option<ProviderConfig>,
    pub google: Option<ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    #[serde(default)]
    pub api_key: String,
    pub base_url: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Neo4jConfig {
    pub uri: String,
    pub user: String,
    pub password: String,
    pub database: Option<String>,
}

impl Config {
    /// Get the configuration directory path
    pub fn config_dir() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not determine config directory")?
            .join("rknowledge");
        Ok(config_dir)
    }

    /// Get the configuration file path
    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    /// Load configuration from file
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            anyhow::bail!(
                "Configuration file not found at {}. Run 'rknowledge init' first.",
                config_path.display()
            );
        }

        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file at {}", config_path.display()))?;

        let mut config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file at {}", config_path.display()))?;

        // Expand environment variables in API keys
        config.expand_env_vars();

        Ok(config)
    }

    /// Expand environment variables in configuration values
    fn expand_env_vars(&mut self) {
        if let Some(ref mut provider) = self.providers.anthropic {
            provider.api_key = expand_env_var(&provider.api_key);
        }
        if let Some(ref mut provider) = self.providers.openai {
            provider.api_key = expand_env_var(&provider.api_key);
        }
        if let Some(ref mut provider) = self.providers.ollama {
            provider.api_key = expand_env_var(&provider.api_key);
        }
        if let Some(ref mut provider) = self.providers.google {
            provider.api_key = expand_env_var(&provider.api_key);
        }
        self.neo4j.password = expand_env_var(&self.neo4j.password);
    }

    /// Get provider configuration by name
    pub fn get_provider(&self, name: &str) -> Option<&ProviderConfig> {
        match name.to_lowercase().as_str() {
            "anthropic" => self.providers.anthropic.as_ref(),
            "openai" => self.providers.openai.as_ref(),
            "ollama" => self.providers.ollama.as_ref(),
            "google" => self.providers.google.as_ref(),
            _ => None,
        }
    }
}

/// Expand environment variable references like ${VAR_NAME}
fn expand_env_var(value: &str) -> String {
    if value.starts_with("${") && value.ends_with('}') {
        let var_name = &value[2..value.len() - 1];
        std::env::var(var_name).unwrap_or_default()
    } else if let Some(var_name) = value.strip_prefix('$') {
        std::env::var(var_name).unwrap_or_default()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_env_var_braces() {
        // SAFETY: test is single-threaded
        unsafe { std::env::set_var("TEST_VAR_A", "value_a") };
        assert_eq!(expand_env_var("${TEST_VAR_A}"), "value_a");
        unsafe { std::env::remove_var("TEST_VAR_A") };
    }

    #[test]
    fn test_expand_env_var_dollar() {
        unsafe { std::env::set_var("TEST_VAR_B", "value_b") };
        assert_eq!(expand_env_var("$TEST_VAR_B"), "value_b");
        unsafe { std::env::remove_var("TEST_VAR_B") };
    }

    #[test]
    fn test_expand_env_var_literal() {
        assert_eq!(expand_env_var("literal_value"), "literal_value");
    }

    #[test]
    fn test_expand_env_var_missing_returns_empty() {
        assert_eq!(expand_env_var("${DEFINITELY_NOT_SET_XYZ_123}"), "");
    }

    #[test]
    fn test_expand_env_var_empty_string() {
        assert_eq!(expand_env_var(""), "");
    }

    #[test]
    fn test_config_from_toml() {
        let toml_str = r#"
            default_provider = "ollama"
            default_model = "mistral"
            chunk_size = 2000
            chunk_overlap = 200

            [providers.ollama]
            api_key = ""
            base_url = "http://localhost:11434"
            model = "mistral"

            [neo4j]
            uri = "bolt://localhost:7687"
            user = "neo4j"
            password = "test"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.default_provider, "ollama");
        assert_eq!(config.default_model.as_deref(), Some("mistral"));
        assert_eq!(config.chunk_size, 2000);
        assert_eq!(config.chunk_overlap, 200);
        assert_eq!(config.neo4j.uri, "bolt://localhost:7687");
    }

    #[test]
    fn test_config_default_values() {
        let toml_str = r#"
            [providers]
            [neo4j]
            uri = "bolt://localhost:7687"
            user = "neo4j"
            password = "test"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.default_provider, "anthropic");
        assert_eq!(config.chunk_size, 1500);
        assert_eq!(config.chunk_overlap, 150);
    }

    #[test]
    fn test_get_provider() {
        let toml_str = r#"
            [providers.anthropic]
            api_key = "sk-test"
            model = "claude"

            [providers.ollama]
            api_key = ""
            base_url = "http://localhost:11434"

            [neo4j]
            uri = "bolt://localhost:7687"
            user = "neo4j"
            password = "test"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.get_provider("anthropic").is_some());
        assert!(config.get_provider("ollama").is_some());
        assert!(config.get_provider("openai").is_none());
        assert!(config.get_provider("nonexistent").is_none());
        assert_eq!(config.get_provider("anthropic").unwrap().api_key, "sk-test");
    }

    #[test]
    fn test_config_roundtrip_toml() {
        let config = Config {
            default_provider: "openai".into(),
            default_model: Some("gpt-4o".into()),
            chunk_size: 1000,
            chunk_overlap: 100,
            providers: ProvidersConfig {
                anthropic: None,
                openai: Some(ProviderConfig {
                    api_key: "sk-123".into(),
                    base_url: None,
                    model: Some("gpt-4o".into()),
                }),
                ollama: None,
                google: None,
            },
            neo4j: Neo4jConfig {
                uri: "bolt://localhost:7687".into(),
                user: "neo4j".into(),
                password: "pw".into(),
                database: Some("neo4j".into()),
            },
        };

        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.default_provider, "openai");
        assert_eq!(deserialized.providers.openai.unwrap().api_key, "sk-123");
    }
}
