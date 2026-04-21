//! LLM-based intent classification fallback (when ML confidence is low).
//! Also provides teacher-label persistence for re-training the ONNX classifier.

use sgr_agent::Llm;
use sgr_agent::types::Message;

use crate::config::LlmOverrides;
use crate::llm_config::make_llm_config;

/// Quick LLM intent classification — 1 function call when ML confidence is low.
/// Returns None on error (structural fallback handles it).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn classify_intent_via_llm(
    instruction: &str,
    model: &str,
    base_url: Option<&str>,
    api_key: &str,
    extra_headers: &[(String, String)],
    temperature: f32,
    overrides: &LlmOverrides,
) -> Option<String> {
    use sgr_agent::tool::ToolDef;

    let cfg = make_llm_config(model, base_url, api_key, extra_headers, temperature, overrides);
    let llm = Llm::new(&cfg);

    let td = ToolDef {
        name: "classify".to_string(),
        description: "Classify the task intent".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "intent": {
                    "type": "string",
                    "enum": crate::intent::Intent::wire_values(),
                    "description": "inbox=process/review/handle inbox messages or queue, email=send/write/compose email, delete=remove/discard/clean up files, query=lookup/find/count/list data, edit=update/create/modify files, capture=capture/distill from inbox into cards"
                }
            },
            "required": ["intent"]
        }),
    };

    let messages = vec![
        Message::system("Classify this CRM task instruction into one intent. Just call classify()."),
        Message::user(instruction),
    ];

    match llm.tools_call_stateful(&messages, &[td], None).await {
        Ok((calls, _)) if !calls.is_empty() => {
            calls[0].arguments.get("intent").and_then(|v| v.as_str()).map(|s| s.to_string())
        }
        Ok(_) => None,
        Err(e) => {
            eprintln!("  ⚠ LLM intent classify failed: {}", e);
            None
        }
    }
}

/// Teacher-student: save high-confidence embedding classification for ONNX retraining.
/// Appends to .agent/teacher_labels.jsonl — used by export_model.py as extra training data.
pub(crate) fn save_teacher_label(instruction: &str, label: &str, confidence: f32, kind: &str) {
    use std::io::Write;
    let path = ".agent/teacher_labels.jsonl";
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{{\"text\":{},\"label\":{},\"confidence\":{:.3},\"kind\":{}}}",
            serde_json::json!(instruction), serde_json::json!(label), confidence, serde_json::json!(kind));
    }
}
