//! Agent configuration — loaded from config.toml.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub agent: AgentSection,
    pub llm: LlmSection,
    #[serde(default)]
    pub defaults: DefaultsSection,
    pub providers: HashMap<String, ProviderSection>,
    #[serde(default)]
    pub embeddings: EmbeddingsSection,
}

#[derive(Debug, Deserialize)]
pub struct EmbeddingsSection {
    #[serde(default = "default_embed_model")]
    pub model: String,
    #[serde(default = "default_embed_base_url")]
    pub base_url: String,
    /// Env var name for API key
    #[serde(default = "default_embed_api_key_env")]
    pub api_key_env: String,
}

impl Default for EmbeddingsSection {
    fn default() -> Self {
        Self {
            model: default_embed_model(),
            base_url: default_embed_base_url(),
            api_key_env: default_embed_api_key_env(),
        }
    }
}

fn default_embed_model() -> String { "workers-ai/@cf/baai/bge-m3".to_string() }
fn default_embed_base_url() -> String { "https://gateway.ai.cloudflare.com/v1/33dec9645c443eef5859b1e10ce71e01/superapi/compat".to_string() }
fn default_embed_api_key_env() -> String { "CF_AI_API_KEY".to_string() }

#[derive(Debug, Deserialize, Default)]
pub struct DefaultsSection {
    pub temperature: Option<f32>,
    pub planning_temperature: Option<f32>,
    pub prompt_cache_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AgentSection {
    #[serde(default = "default_max_steps")]
    pub max_steps: usize,
    #[serde(default = "default_benchmark")]
    pub benchmark: String,
    /// Prefix for leaderboard run names (e.g. "rustman.org").
    #[serde(default)]
    pub run_prefix: String,
    /// Fallback providers for ensemble retry, in priority order.
    /// Primary provider is auto-excluded from this list at runtime.
    #[serde(default)]
    pub fallback_providers: Vec<String>,
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
    /// Reasoning effort for reasoning models: "none", "low", "medium", "high".
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    /// Server-side prompt prefix caching key.
    #[serde(default)]
    pub prompt_cache_key: Option<String>,
    /// Use Chat Completions API instead of Responses API.
    #[serde(default)]
    pub use_chat_api: Option<bool>,
    /// Enable WebSocket for Responses API (lower latency, persistent connection).
    /// Default: true for Responses API, ignored for Chat Completions.
    #[serde(default)]
    pub websocket: Option<bool>,
    /// Agent phase mode: "off" (two-phase), "simple" (single-phase), "hybrid", "auto" (model-detect).
    /// Default: auto — single-phase for most, two-phase for gpt-5.4-mini.
    #[serde(default)]
    pub single_phase: Option<String>,
    /// Pricing per million tokens (for budget estimation).
    #[serde(default)]
    pub pricing: Option<PricingSection>,
}

/// Per-trial overrides for LLM config — explicit parameters instead of env vars.
#[derive(Debug, Clone, Default)]
pub struct LlmOverrides {
    pub use_chat_api: bool,
    pub websocket: bool,
    pub reasoning_effort: Option<String>,
    pub prompt_cache_key: Option<String>,
    /// Raw single_phase config string (off/simple/hybrid/auto). None = auto.
    pub single_phase: Option<String>,
}

/// Resolved provider — all values needed for LLM setup.
#[derive(Debug, Clone)]
pub struct ResolvedProvider {
    pub model: String,
    pub base_url: Option<String>,
    pub api_key: String,
    pub extra_headers: Vec<(String, String)>,
    pub prompt_mode: String,
    pub temperature: f32,
    pub planning_temperature: f32,
    pub sgr_mode: bool,
    pub reasoning_effort: Option<String>,
    pub use_chat_api: bool,
    /// Raw `single_phase` config value — resolved to `SinglePhaseMode` at agent construction.
    pub single_phase: Option<String>,
}

/// Per-provider pricing in $/M tokens.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct PricingSection {
    /// Input tokens price per million.
    #[serde(default)]
    pub input: f64,
    /// Output tokens price per million.
    #[serde(default)]
    pub output: f64,
    /// Cache read tokens price per million.
    #[serde(default)]
    pub cache_read: f64,
    /// Cache write tokens price per million.
    #[serde(default)]
    pub cache_write: f64,
}

fn default_max_steps() -> usize { 20 }
fn default_benchmark() -> String { "bitgn/pac1-dev".into() }

impl Config {
    pub fn load(path: &str) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .context(format!("reading config from {}", path))?;
        toml::from_str(&text).context("parsing config.toml")
    }

    /// Resolve provider by name into a typed struct.
    pub fn resolve_provider(&self, name: &str) -> Result<ResolvedProvider> {
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

        let extra_headers: Vec<(String, String)> = p.headers.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        // AI-NOTE: V2 default — was "explicit" before 2026-04-10. All non-Nemotron models got wrong
        //   prompt causing 30pp gap (81% vs 95%). V2 is annotation-driven, explicit was numbered tree.
        let prompt_mode = p.prompt_mode.clone().unwrap_or_else(|| "v2".into());
        // AI-NOTE: temp 0.05 / planning 0.15 — Nemotron-tuned. Was 0.2/0.4, all models tested worse.
        let temperature = p.temperature.or(self.defaults.temperature).unwrap_or(0.05);
        let planning_temperature = p.planning_temperature.or(self.defaults.planning_temperature).unwrap_or(0.15);

        Ok(ResolvedProvider {
            model: p.model.clone(),
            base_url: p.base_url.clone(),
            api_key,
            extra_headers,
            prompt_mode,
            temperature,
            planning_temperature,
            sgr_mode: p.sgr_mode.unwrap_or(false),
            reasoning_effort: p.reasoning_effort.clone(),
            use_chat_api: p.use_chat_api.unwrap_or(false),
            single_phase: p.single_phase.clone(),
        })
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
        let resolved = cfg.resolve_provider("test").unwrap();
        let (temp, plan_temp) = (resolved.temperature, resolved.planning_temperature);
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
        let resolved = cfg.resolve_provider("test").unwrap();
        let (temp, plan_temp) = (resolved.temperature, resolved.planning_temperature);
        assert!((temp - 0.05).abs() < 0.001); // default temperature
        assert!((plan_temp - 0.15).abs() < 0.001); // default planning_temperature
    }
}
