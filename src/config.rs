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
    /// Auth method: "keychain" for Claude Code subscription (macOS Keychain OAuth token)
    #[serde(default)]
    pub auth: Option<String>,
    /// Extra HTTP headers (e.g. cf-aig-request-timeout for CF Gateway)
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    /// Prompt mode: "explicit" (numbered decision tree for weak models) or "standard" (default)
    #[serde(default)]
    pub prompt_mode: Option<String>,
    /// LLM temperature (default 0.2). Use 0.0 for deterministic output.
    #[serde(default)]
    pub temperature: Option<f32>,
    /// Separate temperature for planning phase (default 0.4). Higher = more exploration.
    #[serde(default)]
    pub planning_temperature: Option<f32>,
    /// Pure SGR mode: single LLM call per step (reasoning + tool in one schema).
    /// Faster on weak models (Nemotron, Gemma). Two-phase FC better on strong models (GPT-5.4).
    #[serde(default)]
    pub sgr_mode: Option<bool>,
}

fn default_max_steps() -> usize { 20 }
fn default_benchmark() -> String { "bitgn/pac1-dev".into() }

impl Config {
    pub fn load(path: &str) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .context(format!("reading config from {}", path))?;
        toml::from_str(&text).context("parsing config.toml")
    }

    /// Resolve provider by name, return (model, base_url, api_key, extra_headers, prompt_mode, temperature, planning_temperature, sgr_mode).
    #[allow(clippy::type_complexity)]
    pub fn resolve_provider(
        &self,
        name: &str,
    ) -> Result<(String, Option<String>, String, Vec<(String, String)>, String, f32, f32, bool)> {
        let p = self
            .providers
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("provider '{}' not found in config", name))?;

        let api_key = if p.auth.as_deref() == Some("keychain") {
            sgr_agent::providers::load_claude_keychain_token()
                .map_err(|e| anyhow::anyhow!("Claude keychain auth failed: {}", e))?
        } else if let Some(ref key) = p.api_key {
            key.clone()
        } else if let Some(ref env_var) = p.api_key_env {
            std::env::var(env_var)
                .ok()
                .filter(|v| !v.is_empty())
                .ok_or_else(|| anyhow::anyhow!("env var {} not set for provider {}", env_var, name))?
        } else {
            std::env::var("OPENAI_API_KEY").unwrap_or_default()
        };

        let headers: Vec<(String, String)> = p.headers.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let prompt_mode = p.prompt_mode.clone().unwrap_or_else(|| "explicit".into());
        let temperature = p.temperature.unwrap_or(0.2);
        let planning_temperature = p.planning_temperature.unwrap_or(0.4);

        let sgr_mode = p.sgr_mode.unwrap_or(false);

        Ok((p.model.clone(), p.base_url.clone(), api_key, headers, prompt_mode, temperature, planning_temperature, sgr_mode))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planning_temperature_parsed_when_present() {
        let toml_str = r#"
[agent]
max_steps = 10
benchmark = "test"

[llm]
provider = "test"

[providers.test]
model = "test-model"
api_key = "sk-test"
temperature = 0.1
planning_temperature = 0.4
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        let p = cfg.providers.get("test").unwrap();
        assert_eq!(p.planning_temperature, Some(0.4));
        let (_, _, _, _, _, temp, plan_temp, _) = cfg.resolve_provider("test").unwrap();
        assert!((temp - 0.1).abs() < 0.001);
        assert!((plan_temp - 0.4).abs() < 0.001);
    }

    #[test]
    fn planning_temperature_defaults_when_absent() {
        let toml_str = r#"
[agent]
max_steps = 10
benchmark = "test"

[llm]
provider = "test"

[providers.test]
model = "test-model"
api_key = "sk-test"
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        let p = cfg.providers.get("test").unwrap();
        assert_eq!(p.planning_temperature, None);
        let (_, _, _, _, _, temp, plan_temp, _) = cfg.resolve_provider("test").unwrap();
        assert!((temp - 0.2).abs() < 0.001); // default temperature
        assert!((plan_temp - 0.4).abs() < 0.001); // default planning_temperature
    }
}
