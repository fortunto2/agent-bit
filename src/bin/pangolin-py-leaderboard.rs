//! Pangolin-arch full leaderboard runner (experiment).
//!
//! Runs ALL trials in a BitGN leaderboard run through the one-tool agent, in
//! parallel (--parallel N), with Phoenix tracing under project `pac1-pangolin-py`.
//!
//! Usage: `cargo run --release --bin pangolin-leaderboard -- \
//!    --provider cf-gemma4 --run pangolin-v1 --parallel 10`

#![allow(dead_code)]

use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use serde_json::Value;

use sgr_agent::agent_tool::Tool;
use sgr_agent::llm::Llm;
use sgr_agent::tool::ToolDef;
use sgr_agent::types::{Message, ToolCall};
use tracing::Instrument;

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
#[path = "../pangolin_py.rs"]
mod pangolin_py;
#[path = "../util.rs"]
mod util;

use crate::bitgn::HarnessClient;
use crate::pangolin_py::{AnswerPayload, ExecutePyTool, PangolinSession};
use crate::pcm::PcmClient;

#[derive(Parser, Clone)]
struct Cli {
    #[arg(long, short = 'p')]
    provider: String,
    #[arg(long)]
    run: String,
    #[arg(long, default_value_t = 5)]
    parallel: usize,
    #[arg(long, default_value_t = 15)]
    max_iter: usize,
    #[arg(long, env = "BITGN_URL", default_value = "https://api.bitgn.com")]
    bitgn_url: String,
    #[arg(long, env = "BITGN_API_KEY")]
    api_key: Option<String>,
    #[arg(long, default_value = "config.toml")]
    config: String,
    /// Provider-specific prompt override path (default: prompts/pangolin-py.md)
    #[arg(long, default_value = "prompts/pangolin-py.md")]
    prompt: String,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 8)]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let telemetry_guard = sgr_agent::init_telemetry(".agent", "pac1-pangolin-py");
    let result = sgr_agent::with_telemetry_scope(run()).await;
    drop(telemetry_guard);
    result
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    let cfg = config::Config::load(&cli.config)?;
    let rp = cfg.resolve_provider(&cli.provider)?;

    let api_key = cli.api_key.clone()
        .ok_or_else(|| anyhow!("--api-key or BITGN_API_KEY required for leaderboard mode"))?;

    let harness = Arc::new(HarnessClient::new(&cli.bitgn_url, Some(api_key.clone())));
    let benchmark = cfg.agent.benchmark.clone();
    let prefixed = if cfg.agent.run_prefix.is_empty() {
        cli.run.clone()
    } else {
        format!("{}-{}", cfg.agent.run_prefix, cli.run)
    };
    eprintln!("[pangolin-lb] Starting run: {}", prefixed);
    let run = harness.start_run(&benchmark, &prefixed, &api_key).await?;
    eprintln!("[pangolin-lb] Run {} — {} trials", run.run_id, run.trial_ids.len());

    let system_prompt = std::fs::read_to_string(&cli.prompt)
        .with_context(|| format!("read {}", cli.prompt))?;

    let model = rp.model.clone();
    let base_url = rp.base_url.clone();
    let llm_api_key = rp.api_key.clone();
    let extra_headers = rp.extra_headers.clone();
    let overrides = config::LlmOverrides {
        use_chat_api: rp.use_chat_api,
        websocket: false,
        reasoning_effort: rp.reasoning_effort.clone(),
        prompt_cache_key: None,
        single_phase: rp.single_phase.clone(),
    };
    let temperature = rp.temperature;

    let sem = Arc::new(tokio::sync::Semaphore::new(cli.parallel));
    let results: Arc<tokio::sync::Mutex<Vec<(String, f32, String, usize)>>> =
        Arc::new(tokio::sync::Mutex::new(Vec::new()));

    let mut handles = Vec::new();
    let total = run.trial_ids.len();
    for (idx, trial_id) in run.trial_ids.iter().enumerate() {
        let sem = sem.clone();
        let harness = harness.clone();
        let trial_id = trial_id.clone();
        let system_prompt = system_prompt.clone();
        let cli = cli.clone();
        let model = model.clone();
        let base_url = base_url.clone();
        let llm_api_key = llm_api_key.clone();
        let extra_headers = extra_headers.clone();
        let overrides = overrides.clone();
        let results = results.clone();

        let handle = tokio::spawn(sgr_agent::with_telemetry_scope(async move {
            let _permit = sem.acquire().await.unwrap();
            let trial = match harness.start_trial(&trial_id).await {
                Ok(t) => t,
                Err(e) => { eprintln!("  ⚠ start_trial {}: {}", trial_id, e); return; }
            };
            eprintln!("━━━ Trial {}/{}: {} ({}) ━━━", idx + 1, total, trial.trial_id, trial.task_id);

            let session_id = format!("{}_{}", trial.task_id, trial.trial_id);
            sgr_agent::set_session_id(session_id.clone());
            sgr_agent::set_task_id(trial.task_id.clone());

            let llm_cfg = llm_config::make_llm_config(
                &model, base_url.as_deref(), &llm_api_key, &extra_headers, temperature, &overrides,
            );
            let llm = Llm::new_async(&llm_cfg).await;

            let pcm = Arc::new(PcmClient::new(&trial.harness_url));
            let t0 = Instant::now();
            let (outcome, iters, score_detail) = match solve_one(
                &llm, pcm.clone(), &system_prompt, &trial.instruction,
                &trial.task_id, &session_id, &trial.trial_id, cli.max_iter,
            ).await {
                Ok((o, i)) => (o, i, vec![]),
                Err(e) => { eprintln!("  ⚠ solve error: {e}"); ("ERR".into(), 0, vec![format!("{e}")]) }
            };
            let elapsed = t0.elapsed();

            let ended = match harness.end_trial(&trial.trial_id).await {
                Ok(r) => r,
                Err(e) => { eprintln!("  ⚠ end_trial: {e}");
                    bitgn::EndTrialResponse { trial_id: trial_id.clone(), score: Some(0.0),
                        score_detail: vec![format!("end_trial error: {e}")] } }
            };
            let score = ended.score.unwrap_or(0.0);
            eprintln!("  {} Score: {:.2} ({}  {:.1}s  {}i)",
                trial.task_id, score, outcome, elapsed.as_secs_f32(), iters);
            for d in &ended.score_detail { eprintln!("    • {d}"); }
            for d in &score_detail { eprintln!("    • {d}"); }

            // Dump trial for dashboard/analysis.
            let short_model = model.rsplit('/').next().unwrap_or(&model);
            let dump_dir = format!("benchmarks/tasks/{}/pangolin-{}_{}",
                trial.task_id, short_model, trial.trial_id);
            let _ = std::fs::create_dir_all(&dump_dir);
            let _ = std::fs::write(format!("{}/bitgn_log.url", dump_dir),
                format!("https://{}.eu.bitgn.com\n", trial.trial_id));
            let _ = std::fs::write(format!("{}/instruction.txt", dump_dir), &trial.instruction);
            let _ = std::fs::write(format!("{}/score.txt", dump_dir),
                format!("{:.2}\n{}\n{}\n",
                    score, outcome, ended.score_detail.join("\n")));

            results.lock().await.push((trial.task_id.clone(), score, outcome, iters));
        }));
        handles.push(handle);
    }

    futures::future::join_all(handles).await;

    let run_status = harness.get_run(&run.run_id).await?;
    eprintln!("\n[pangolin-lb] Run state: {}", run_status.state);
    if let Some(score) = run_status.score {
        eprintln!("[pangolin-lb] Run score: {:.1}%", score * 100.0);
    }
    let all = results.lock().await;
    let passed = all.iter().filter(|(_, s, _, _)| *s >= 1.0).count();
    eprintln!("[pangolin-lb] {} / {} trials passed", passed, all.len());

    eprintln!("[pangolin-lb] Submitting run...");
    harness.submit_run(&run.run_id).await?;
    eprintln!("[pangolin-lb] Submitted! Run ID: {}", run.run_id);
    Ok(())
}

/// Run one pangolin trial to completion (or max_iter), submit answer to PCM.
/// Returns (outcome, iterations).
async fn solve_one(
    llm: &Llm,
    pcm: Arc<PcmClient>,
    system_prompt: &str,
    instruction: &str,
    task_id: &str,
    session_id: &str,
    trial_id: &str,
    max_iter: usize,
) -> Result<(String, usize)> {
    let tree = pcm.tree("/", 3).await.unwrap_or_else(|e| format!("(tree error: {e})"));
    let ctx_raw = pcm.context().await.unwrap_or_default();
    // Pangolin-original parses context server-side into {time, unixTime}. Do the same.
    let ctx_json = pangolin_py::parse_context_output(&ctx_raw);

    let session = PangolinSession::with_context(pcm.clone(), ctx_json, task_id);
    let tool = ExecutePyTool { session: session.clone() };

    let mut messages: Vec<Message> = vec![
        Message::system(format!(
            "{system_prompt}\n\n<task-instruction>\n{}\n</task-instruction>\n\n<workspace-tree>\n{}\n</workspace-tree>",
            instruction, tree
        )),
    ];
    let tool_defs = vec![ToolDef {
        name: tool.name().to_string(),
        description: tool.description().to_string(),
        parameters: tool.parameters_schema(),
    }];

    let root_span = tracing::info_span!(
        "pangolin_trial", task.id = %task_id, session.id = %session_id, trial.id = %trial_id,
    );
    {
        use tracing_opentelemetry::OpenTelemetrySpanExt;
        root_span.set_attribute("openinference.span.kind", "AGENT");
        root_span.set_attribute("input.value", instruction.to_string());
    }

    let loop_body = async {
        let mut answer: Option<AnswerPayload> = None;
        let mut iter = 0;
        while iter < max_iter && answer.is_none() {
            iter += 1;
            let sp = serde_json::to_string_pretty(&session.scratchpad_snapshot()).unwrap_or_default();
            messages.push(Message::user(format!("<scratchpad>\n{sp}\n</scratchpad>")));
            let step_span = tracing::info_span!("agent_step", step = iter);
            let (calls, text) = match llm.tools_call_with_text(&messages, &tool_defs)
                .instrument(step_span.clone()).await
            {
                Ok(v) => v,
                Err(e) => { eprintln!("  {} LLM error: {e}", task_id); break; }
            };
            if calls.is_empty() {
                messages.push(Message::assistant(text.clone()));
                messages.push(Message::user(
                    "Call execute_code. Populate scratchpad and call ws_answer() when done.",
                ));
                continue;
            }
            messages.push(Message::assistant_with_tool_calls(text, calls.clone()));
            for call in calls {
                let ToolCall { id, name, arguments } = call;
                if name != "execute_code" { continue; }
                let code = arguments.get("code").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let exec_span = tracing::info_span!(parent: &step_span, "execute_code", iter = iter);
                {
                    use tracing_opentelemetry::OpenTelemetrySpanExt;
                    exec_span.set_attribute("openinference.span.kind", "TOOL");
                    exec_span.set_attribute("tool.name", "execute_code");
                    exec_span.set_attribute("input.value", code.clone());
                }
                let mut actx = sgr_agent::context::AgentContext::default();
                let res = match tool.execute(arguments, &mut actx).instrument(exec_span.clone()).await {
                    Ok(r) => r,
                    Err(e) => { eprintln!("  {} tool error: {e}", task_id); return (None, iter); }
                };
                let out = res.content;
                {
                    use tracing_opentelemetry::OpenTelemetrySpanExt;
                    exec_span.set_attribute("output.value", out.clone());
                }
                messages.push(Message::tool(id, out));
                if let Some(a) = session.take_answer() { answer = Some(a); break; }
            }
        }
        (answer, iter)
    };

    let (answer, iter) = loop_body.instrument(root_span.clone()).await;

    // Auto-submit fallback: if agent looped past max_iter without ws_answer, submit
    // CLARIFICATION with whatever refs we auto-tracked — still 0.00 but produces a
    // finalized trial, lets Phoenix show the run, and on CLARIFICATION-expected tasks
    // it may even pass.
    let a = match answer {
        Some(a) => a,
        None => {
            let refs: Vec<String> = session.refs_tracking.lock().unwrap().clone();
            eprintln!("  {task_id} auto-submit CLARIFICATION (no ws_answer after {iter} iters)");
            AnswerPayload {
                message: format!("unable to determine action within {iter} iterations"),
                outcome: "OUTCOME_NONE_CLARIFICATION".into(),
                refs,
            }
        }
    };
    pcm.answer(&a.message, &a.outcome, &a.refs).await
        .with_context(|| "submit answer to PCM")?;

    {
        use tracing_opentelemetry::OpenTelemetrySpanExt;
        root_span.set_attribute("output.value",
            serde_json::to_string(&serde_json::json!({
                "outcome": a.outcome, "answer": a.message, "refs": a.refs, "iterations": iter
            })).unwrap_or_default());
    }

    Ok((a.outcome, iter))
}
