//! Pangolin-arch selective benchmark runner (experiment).
//!
//! Runs a single task through the Pangolin-style one-tool agent to sanity-check
//! the scaffold. NOT a production runner — uses only --provider and --task.
//!
//! Usage: `cargo run --release --bin pangolin-bench -- --provider or-haiku --task t014`

#![allow(dead_code)]

use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use serde_json::Value;

use sgr_agent::llm::Llm;
use sgr_agent::tool::ToolDef;
use sgr_agent::types::{Message, ToolCall};
use sgr_agent::agent_tool::Tool;
use tracing::Instrument;

// Reuse the agent-bit modules by path.
#[path = "../bitgn.rs"]
mod bitgn;
#[path = "../config.rs"]
mod config;
#[path = "../llm_config.rs"]
mod llm_config;
#[path = "../pcm.rs"]
mod pcm;
#[path = "../policy.rs"]
mod policy;
#[path = "../pangolin.rs"]
mod pangolin;
#[path = "../util.rs"]
mod util;

use crate::bitgn::HarnessClient;
use crate::pangolin::{AnswerPayload, ExecuteCodeTool, PangolinSession};
use crate::pcm::PcmClient;

#[derive(Parser)]
struct Cli {
    #[arg(long, short = 'p')]
    provider: String,
    #[arg(long)]
    task: String,
    #[arg(long, env = "BITGN_URL", default_value = "https://api.bitgn.com")]
    bitgn_url: String,
    #[arg(long, env = "BITGN_API_KEY")]
    api_key: Option<String>,
    #[arg(long, default_value_t = 12)]
    max_iter: usize,
    #[arg(long, default_value = "config.toml")]
    config: String,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    // Phoenix: separate project "pac1-pangolin" to keep traces distinct from main agent.
    let telemetry_guard = sgr_agent::init_telemetry(".agent", "pac1-pangolin");
    // All work must run inside with_telemetry_scope so set_session_id/set_task_id succeed.
    let result = sgr_agent::with_telemetry_scope(run()).await;
    drop(telemetry_guard); // flush OTLP spans before tokio exits
    result
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    let cfg = config::Config::load(&cli.config)?;
    let rp = cfg.resolve_provider(&cli.provider)?;
    let overrides = config::LlmOverrides {
        use_chat_api: rp.use_chat_api,
        websocket: false,
        reasoning_effort: rp.reasoning_effort.clone(),
        prompt_cache_key: None,
        single_phase: rp.single_phase.clone(),
    };
    let llm_cfg = llm_config::make_llm_config(
        &rp.model, rp.base_url.as_deref(), &rp.api_key, &rp.extra_headers,
        rp.temperature, &overrides,
    );
    let llm = Llm::new_async(&llm_cfg).await;

    let harness = HarnessClient::new(&cli.bitgn_url, cli.api_key.clone());
    let benchmark = &cfg.agent.benchmark;
    eprintln!("[pangolin] Benchmark: {}, task: {}", benchmark, cli.task);
    let trial = harness.start_playground(benchmark, &cli.task).await
        .with_context(|| format!("start_playground {}", cli.task))?;
    // Phoenix session binding: every LLM call from here on carries these IDs.
    let session_id = format!("{}_{}", cli.task, trial.trial_id);
    sgr_agent::set_session_id(session_id.clone());
    sgr_agent::set_task_id(cli.task.clone());
    eprintln!("[pangolin] Trial: {}", trial.trial_id);
    eprintln!("[pangolin] URL:   {}", trial.harness_url);
    eprintln!("[pangolin] Instruction: {}", trial.instruction);

    let pcm = Arc::new(PcmClient::new(&trial.harness_url));

    // Pre-fetch workspace context: tree (level 2) + date.
    let tree = pcm.tree("/", 3).await.unwrap_or_else(|e| format!("(tree error: {e})"));
    let ctx_raw = pcm.context().await.unwrap_or_default();
    let ctx_json = pangolin::parse_context_output(&ctx_raw);

    let session = PangolinSession::with_context(pcm.clone(), ctx_json);
    let tool = ExecuteCodeTool { session: session.clone() };

    let system_prompt = std::fs::read_to_string("prompts/pangolin.md")
        .unwrap_or_else(|_| "Solve the task via execute_code.".to_string());

    let mut messages: Vec<Message> = vec![
        Message::system(format!(
            "{system_prompt}\n\n<task-instruction>\n{}\n</task-instruction>\n\n<workspace-tree>\n{}\n</workspace-tree>",
            trial.instruction, tree
        )),
    ];

    let tool_defs = vec![ToolDef {
        name: tool.name().to_string(),
        description: tool.description().to_string(),
        parameters: tool.parameters_schema(),
    }];

    // Root span — single trace per trial. sgr-agent LLM calls inherit parent
    // automatically via tracing subscriber; annotate_session (trial.result) is
    // attached below via root_span.in_scope(...).
    let root_span = tracing::info_span!(
        "pangolin_trial",
        task.id = %cli.task,
        session.id = %session_id,
        trial.id = %trial.trial_id,
    );
    {
        use tracing_opentelemetry::OpenTelemetrySpanExt;
        root_span.set_attribute("openinference.span.kind", "AGENT");
        root_span.set_attribute("input.value", trial.instruction.clone());
    }

    let loop_body = async {
        let mut answer: Option<AnswerPayload> = None;
        let mut iter: usize = 0;
        while iter < cli.max_iter && answer.is_none() {
            iter += 1;
            let sp = serde_json::to_string_pretty(&session.scratchpad_snapshot()).unwrap_or_default();
            messages.push(Message::user(format!("<scratchpad>\n{sp}\n</scratchpad>")));

            eprintln!("\n[pangolin] ── iter {} ──", iter);
            // `agent_step` matches sgr-agent app_loop convention — Phoenix already
            // knows this span shape; no custom attributes needed here.
            let step_span = tracing::info_span!("agent_step", step = iter);
            let (calls, text) = match llm.tools_call_with_text(&messages, &tool_defs)
                .instrument(step_span.clone()).await
            {
                Ok(v) => v,
                Err(e) => { eprintln!("[pangolin] LLM error: {e}"); break; }
            };

            if !text.is_empty() {
                eprintln!("  🧠 {}", text.chars().take(220).collect::<String>());
            }
            if calls.is_empty() {
                eprintln!("  ⚠ no tool calls — nudging");
                messages.push(Message::assistant(text.clone()));
                messages.push(Message::user(
                    "You must call execute_code. Populate scratchpad.{answer,outcome,refs} and call ws_answer() as the terminal line.",
                ));
                continue;
            }

            messages.push(Message::assistant_with_tool_calls(text, calls.clone()));

            for call in calls {
                let ToolCall { id, name, arguments } = call;
                if name != "execute_code" { continue; }
                let code_preview = arguments.get("code").and_then(|v| v.as_str()).unwrap_or("").to_string();
                eprintln!("  → execute_code: {}", code_preview.chars().take(180).collect::<String>());
                let exec_span = tracing::info_span!(
                    parent: &step_span,
                    "execute_code",
                    iter = iter,
                );
            
                {
                    use tracing_opentelemetry::OpenTelemetrySpanExt;
                    exec_span.set_attribute("openinference.span.kind", "TOOL");
                    exec_span.set_attribute("tool.name", "execute_code");
                    exec_span.set_attribute("input.value", code_preview.clone());
                }
                let mut actx = sgr_agent::context::AgentContext::default();
                let result = match tool.execute(arguments, &mut actx)
                    .instrument(exec_span.clone()).await
                {
                    Ok(r) => r,
                    Err(e) => { eprintln!("[pangolin] tool error: {e}"); return (None, iter); }
                };
                let out = result.content.clone();
                // Attach output + scratchpad to the span so Phoenix shows the full trail.
            
                {
                    use tracing_opentelemetry::OpenTelemetrySpanExt;
                    // Phoenix renders these in the Info tab:
                    exec_span.set_attribute("output.value", out.clone());
                    exec_span.set_attribute("output.mime_type", "text/plain");
                    exec_span.set_attribute(
                        "scratchpad",
                        serde_json::to_string(&session.scratchpad_snapshot()).unwrap_or_default(),
                    );
                }
                eprintln!("  ← {}", out.chars().take(220).collect::<String>());
                messages.push(Message::tool(id, out));

                if let Some(a) = session.take_answer() {
                    eprintln!("\n[pangolin] ✓ ws_answer captured: outcome={}, refs={}", a.outcome, a.refs.len());
                    answer = Some(a);
                    break;
                }
            }
        }
        (answer, iter)
    };

    let (answer, iter) = loop_body.instrument(root_span.clone()).await;

    {
        use tracing_opentelemetry::OpenTelemetrySpanExt;
        if let Some(ref a) = answer {
            root_span.set_attribute(
                "output.value",
                serde_json::to_string(&serde_json::json!({
                    "outcome": a.outcome, "answer": a.message, "refs": a.refs, "iterations": iter
                })).unwrap_or_default(),
            );
            root_span.set_attribute("output.mime_type", "application/json");
        } else {
            root_span.set_attribute("output.value", format!("no answer in {iter} iterations"));
        }
    }

    let Some(a) = answer else {
        eprintln!("[pangolin] ✗ no answer in {} iterations", cli.max_iter);
        let _ = harness.end_trial(&trial.trial_id).await;
        return Err(anyhow!("no answer submitted"));
    };

    // Submit and end trial.
    pcm.answer(&a.message, &a.outcome, &a.refs).await
        .with_context(|| "submit answer to PCM")?;
    let ended = harness.end_trial(&trial.trial_id).await?;
    let score = ended.score.unwrap_or(0.0);
    // Phoenix annotation: child of root_span so trial.result shares the trace
    // with pangolin_trial (otherwise it's a sibling ROOT — Session UI splits).
    root_span.in_scope(|| {
        sgr_agent::annotate_session(&cli.task, score, &a.outcome, iter as u32);
    });
    eprintln!("\n[pangolin] Score: {score:.2}");
    for d in &ended.score_detail { eprintln!("  • {d}"); }
    Ok(())
}
