//! Agent configuration — loaded from config.toml.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub agent: AgentSection,
    pub llm: LlmSection,
    pub providers: HashMap<String, ProviderSection>,
}

#[derive(Debug, Deserialize)]
pub struct AgentSection {
    #[serde(default = "default_max_steps")]
    pub max_steps: usize,
    #[serde(default = "default_benchmark")]
    pub benchmark: String,
}

#[derive(Debug, Deserialize)]
pub struct LlmSection {
    pub provider: String,
}

#[derive(Debug, Deserialize)]
pub struct ProviderSection {
    pub model: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub api_key_env: Option<String>,
}

fn default_max_steps() -> usize { 20 }
fn default_benchmark() -> String { "bitgn/pac1-dev".into() }

impl Config {
    pub fn load(path: &str) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .context(format!("reading config from {}", path))?;
        toml::from_str(&text).context("parsing config.toml")
    }

    /// Resolve provider by name, return (model, base_url, api_key).
    pub fn resolve_provider(&self, name: &str) -> Result<(String, Option<String>, String)> {
        let p = self.providers.get(name)
            .ok_or_else(|| anyhow::anyhow!("provider '{}' not found in config", name))?;

        let api_key = if let Some(ref key) = p.api_key {
            key.clone()
        } else if let Some(ref env_var) = p.api_key_env {
            std::env::var(env_var)
                .ok()
                .filter(|v| !v.is_empty())
                .ok_or_else(|| anyhow::anyhow!("env var {} not set for provider {}", env_var, name))?
        } else {
            // Default: try OPENAI_API_KEY
            std::env::var("OPENAI_API_KEY").unwrap_or_default()
        };

        Ok((p.model.clone(), p.base_url.clone(), api_key))
    }
}
