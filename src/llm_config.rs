//! LLM config construction from provider info + runtime overrides.

use sgr_agent::types::LlmConfig;

use crate::config::LlmOverrides;

/// Build `LlmConfig` for a trial, given resolved provider parameters + per-trial overrides.
/// Dispatches between endpoint (OpenRouter-like), keyed (direct OpenAI/Anthropic), or auto (env-driven).
pub(crate) fn make_llm_config(
    model: &str,
    base_url: Option<&str>,
    api_key: &str,
    extra_headers: &[(String, String)],
    temperature: f32,
    overrides: &LlmOverrides,
) -> LlmConfig {
    if let Some(url) = base_url {
        let mut cfg = LlmConfig::endpoint(api_key, url, model).temperature(temperature as f64).max_tokens(4096);
        cfg.use_chat_api = true;
        cfg.websocket = false; // WS not supported on OpenRouter/3rd party
        cfg.extra_headers = extra_headers.to_vec();
        cfg.reasoning_effort = overrides.reasoning_effort.clone();
        cfg.prompt_cache_key = overrides.prompt_cache_key.clone();
        cfg
    } else if !api_key.is_empty() {
        let mut cfg = LlmConfig::with_key(api_key, model).temperature(temperature as f64).max_tokens(4096);
        cfg.extra_headers = extra_headers.to_vec();
        // Native API providers (Anthropic, Gemini) need genai backend
        cfg.use_genai = model.starts_with("claude") || model.starts_with("gemini");
        cfg.use_chat_api = overrides.use_chat_api;
        cfg.websocket = overrides.websocket && !overrides.use_chat_api; // WS only for Responses API
        cfg.reasoning_effort = overrides.reasoning_effort.clone();
        cfg.prompt_cache_key = overrides.prompt_cache_key.clone();
        cfg
    } else {
        let mut cfg = LlmConfig::auto(model).temperature(temperature as f64).max_tokens(4096);
        cfg.extra_headers = extra_headers.to_vec();
        cfg
    }
}
