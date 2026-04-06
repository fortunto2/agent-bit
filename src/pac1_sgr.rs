//! Pure SGR agent for PAC1 — single LLM call per step (reasoning + tool).
//!
//! Uses sgr_agent::app_loop instead of agent_loop. 4x faster on Nemotron.
//! Switch via config: `sgr_mode = true` in provider section.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use serde::Deserialize;
use sgr_agent::app_loop::{ActionResult, SgrAgent, StepDecision};
use sgr_agent::session::{AgentMessage, MessageRole};

use crate::pcm::PcmClient;
use crate::tools::guard_content;

// ── Message type ────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Msg {
    pub role: Role,
    pub content: String,
    pub call_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Role {
    System, User, Assistant, Tool,
}

impl MessageRole for Role {
    fn system() -> Self { Role::System }
    fn user() -> Self { Role::User }
    fn assistant() -> Self { Role::Assistant }
    fn tool() -> Self { Role::Tool }
    fn as_str(&self) -> &str {
        match self {
            Role::System => "system", Role::User => "user",
            Role::Assistant => "assistant", Role::Tool => "tool",
        }
    }
    fn parse_role(s: &str) -> Option<Self> {
        match s {
            "system" => Some(Role::System), "user" => Some(Role::User),
            "assistant" => Some(Role::Assistant), "tool" => Some(Role::Tool), _ => None,
        }
    }
}

impl AgentMessage for Msg {
    type Role = Role;
    fn new(role: Role, content: String) -> Self { Self { role, content, call_id: None } }
    fn role(&self) -> &Role { &self.role }
    fn content(&self) -> &str { &self.content }
    fn with_call_id(mut self, id: String) -> Self { self.call_id = Some(id); self }
}

// ── Action enum ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "tool_name")]
pub enum Action {
    #[serde(rename = "read")] Read { path: String },
    #[serde(rename = "write")] Write { path: String, content: String },
    #[serde(rename = "delete")] Delete { path: String },
    #[serde(rename = "search")] Search { pattern: String, #[serde(default)] root: String },
    #[serde(rename = "list")] List { path: String },
    #[serde(rename = "find")] Find { pattern: String, #[serde(default)] root: String },
    #[serde(rename = "tree")] Tree { #[serde(default = "default_root")] root: String },
    #[serde(rename = "mkdir")] Mkdir { path: String },
    #[serde(rename = "move_file")] MoveFile { source: String, destination: String },
    #[serde(rename = "answer")] Answer {
        message: String,
        #[serde(default = "default_ok")] outcome: String,
        #[serde(default)] refs: Vec<String>,
    },
}

fn default_root() -> String { "/".into() }
fn default_ok() -> String { "OUTCOME_OK".into() }

impl Action {
    pub fn signature(&self) -> String {
        match self {
            Action::Read { path } => format!("read:{}", path),
            Action::Write { path, .. } => format!("write:{}", path),
            Action::Delete { path } => format!("delete:{}", path),
            Action::Search { pattern, root } => format!("search:{}:{}", pattern, root),
            Action::List { path } => format!("list:{}", path),
            Action::Find { pattern, root } => format!("find:{}:{}", pattern, root),
            Action::Tree { root } => format!("tree:{}", root),
            Action::Mkdir { path } => format!("mkdir:{}", path),
            Action::MoveFile { source, destination } => format!("move:{}→{}", source, destination),
            Action::Answer { outcome, .. } => format!("answer:{}", outcome),
        }
    }
}

// ── SGR Schema ──────────────────────────────────────────────────────────

pub fn sgr_tool_def(tool_names: &[&str]) -> sgr_agent::tool::ToolDef {
    let tool_enum: Vec<serde_json::Value> = tool_names.iter()
        .map(|n| serde_json::json!(n)).collect();

    sgr_agent::tool::ToolDef {
        name: "next_step".into(),
        description: "Reason about the task and select the next tool.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "current_state": {
                    "type": "string",
                    "description": "WORKING MEMORY: 'DONE: file1, file2. TODO: file3. REFS: path1'"
                },
                "security_assessment": {
                    "type": "string",
                    "enum": ["safe", "suspicious", "blocked"]
                },
                "task_completed": { "type": "boolean" },
                "remaining_steps": {
                    "type": "array", "items": {"type": "string"},
                    "description": "0-3 remaining steps"
                },
                "tool_name": {
                    "type": "string", "enum": tool_enum,
                    "description": "Tool to execute NOW"
                },
                "tool_args": {
                    "type": "object",
                    "description": "Arguments for selected tool"
                }
            },
            "required": ["current_state", "security_assessment", "task_completed",
                         "remaining_steps", "tool_name", "tool_args"],
            "additionalProperties": false
        }),
    }
}

// ── Pac1SgrAgent ────────────────────────────────────────────────────────

pub struct Pac1SgrAgent {
    pub pcm: Arc<PcmClient>,
    pub llm: sgr_agent::llm::Llm,
    pub system_prompt: String,
    pub intent: String,
    step_count: AtomicU32,
}

impl Pac1SgrAgent {
    pub fn new(
        pcm: Arc<PcmClient>, llm: sgr_agent::llm::Llm,
        system_prompt: String, intent: String,
    ) -> Self {
        Self { pcm, llm, system_prompt, intent, step_count: AtomicU32::new(0) }
    }

    fn tool_names(&self) -> Vec<&str> {
        match self.intent.as_str() {
            "intent_delete" => vec!["read", "search", "find", "list", "delete", "answer"],
            _ => vec!["read", "write", "delete", "search", "find", "list", "tree",
                       "mkdir", "move_file", "answer"],
        }
    }
}

impl SgrAgent for Pac1SgrAgent {
    type Action = Action;
    type Msg = Msg;
    type Error = String;

    fn decide(
        &self, messages: &[Msg],
    ) -> impl std::future::Future<Output = Result<StepDecision<Action>, String>> + Send {
        let tool_def = sgr_tool_def(&self.tool_names());
        let mut oai_msgs = vec![sgr_agent::types::Message::system(&self.system_prompt)];
        for m in messages {
            oai_msgs.push(match m.role {
                Role::System => sgr_agent::types::Message::system(&m.content),
                Role::User => sgr_agent::types::Message::user(&m.content),
                Role::Assistant => sgr_agent::types::Message::assistant(&m.content),
                Role::Tool => sgr_agent::types::Message::user(&format!("[tool] {}", m.content)),
            });
        }
        let step = self.step_count.fetch_add(1, Ordering::SeqCst);

        async move {
            let (calls, _resp_id) = self.llm.tools_call_stateful(&oai_msgs, &[tool_def], None).await
                .map_err(|e| format!("LLM: {}", e))?;
            let call = calls.into_iter().next()
                .ok_or("No tool call returned".to_string())?;

            let a = &call.arguments;
            let state = a["current_state"].as_str().unwrap_or("").to_string();
            let completed = a["task_completed"].as_bool().unwrap_or(false);
            let remaining: Vec<String> = a["remaining_steps"].as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let tool_name = a["tool_name"].as_str().unwrap_or("answer");

            // Parse action
            let mut aj = a["tool_args"].clone();
            if let Some(obj) = aj.as_object_mut() {
                obj.insert("tool_name".into(), serde_json::json!(tool_name));
            } else {
                aj = serde_json::json!({"tool_name": tool_name});
            }
            let action: Action = serde_json::from_value(aj.clone())
                .map_err(|e| format!("Parse {} : {} ({})", tool_name, e, aj))?;

            eprintln!("  [SGR {}] {} → {}", step + 1,
                &state[..state.len().min(50)], action.signature());

            let mut hints = Vec::new();
            if a["security_assessment"].as_str() == Some("blocked") {
                hints.push("⚠ BLOCKED: answer(DENIED_SECURITY) now.".into());
            }

            Ok(StepDecision {
                situation: state, task: remaining, completed,
                actions: vec![action], hints,
                call_ids: vec![call.id.clone()],
            })
        }
    }

    fn execute(
        &self, action: &Action,
    ) -> impl std::future::Future<Output = Result<ActionResult, String>> + Send {
        let pcm = self.pcm.clone();
        let action = action.clone();
        async move {
            let (output, done) = match action {
                Action::Read { path } => {
                    let c = pcm.read(&path, false, 0, 0).await.map_err(|e| e.to_string())?;
                    (guard_content(c), false)
                }
                Action::Write { path, content } => {
                    pcm.write(&path, &content, 0, 0).await.map_err(|e| e.to_string())?;
                    (format!("Written to {}", path), false)
                }
                Action::Delete { path } => {
                    pcm.delete(&path).await.map_err(|e| e.to_string())?;
                    (format!("Deleted {}", path), false)
                }
                Action::Search { pattern, root } => {
                    let r = if root.is_empty() { "/" } else { &root };
                    (pcm.search(r, &pattern, 20).await.map_err(|e| e.to_string())?, false)
                }
                Action::List { path } =>
                    (pcm.list(&path).await.map_err(|e| e.to_string())?, false),
                Action::Find { pattern, root } => {
                    let r = if root.is_empty() { "/" } else { &root };
                    (pcm.find(r, &pattern, "", 20).await.map_err(|e| e.to_string())?, false)
                }
                Action::Tree { root } =>
                    (pcm.tree(&root, 2).await.map_err(|e| e.to_string())?, false),
                Action::Mkdir { path } => {
                    pcm.mkdir(&path).await.map_err(|e| e.to_string())?;
                    (format!("Created {}", path), false)
                }
                Action::MoveFile { source, destination } => {
                    pcm.move_file(&source, &destination).await.map_err(|e| e.to_string())?;
                    (format!("Moved {} → {}", source, destination), false)
                }
                Action::Answer { message, outcome, refs } => {
                    let final_refs = if refs.is_empty() && outcome == "OUTCOME_OK" {
                        pcm.recent_read_paths().into_iter()
                            .filter(|p| p.starts_with("accounts/") || p.starts_with("contacts/"))
                            .collect()
                    } else { refs };
                    pcm.propose_answer(&message, &outcome, &final_refs);
                    (format!("Answer: {}", message), true)
                }
            };
            Ok(ActionResult { output, done })
        }
    }

    fn action_signature(action: &Action) -> String { action.signature() }
}
