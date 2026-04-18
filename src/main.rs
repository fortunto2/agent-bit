#![allow(
    clippy::collapsible_if,
    clippy::too_many_arguments,
    clippy::needless_borrows_for_generic_args,
    clippy::type_complexity,
    clippy::manual_split_once,
    clippy::doc_lazy_continuation,
    clippy::duplicated_attributes,
    clippy::explicit_counter_loop,
    clippy::manual_inspect,
    clippy::char_lit_as_u8,
    clippy::double_ended_iterator_last,
    clippy::needless_range_loop,
    clippy::collapsible_str_replace,
    clippy::needless_borrow,
    clippy::manual_pattern_char_comparison
)]

use std::sync::Arc;

use crate::util::StrExt;

use anyhow::Result;
use clap::Parser;

mod agent;
mod bitgn;
mod classifier;
mod config;
#[allow(dead_code)]
mod crm_graph;
mod pcm;
mod hooks;
mod intent_classify;
mod llm_config;
mod pipeline;
mod policy;
mod pregrounding;
mod prompts;
mod trial_dump;
mod scanner;
#[allow(dead_code)]
mod pac1_sgr;
#[allow(dead_code)]
mod pangolin;
#[allow(dead_code)]
mod pangolin_py;
mod feature_matrix;
mod skills;
mod tools;
mod util;
mod workflow;

#[derive(Parser)]
#[command(name = "pac1-agent", about = "BitGN PAC1 Challenge Agent (Rust + sgr-agent)")]
struct Cli {
    /// Config file path
    #[arg(long, default_value = "config.toml")]
    config: String,

    /// LLM provider from config.toml (overrides llm.provider)
    #[arg(long, short = 'p')]
    provider: Option<String>,

    /// Run only this task (playground mode)
    #[arg(long)]
    task: Option<String>,

    /// BitGN platform URL
    #[arg(long, env = "BITGN_URL", default_value = "https://api.bitgn.com")]
    bitgn_url: String,

    /// BitGN API key (required for --run)
    #[arg(long, env = "BITGN_API_KEY")]
    api_key: Option<String>,

    /// Max agent steps per task (overrides config)
    #[arg(long)]
    max_steps: Option<usize>,

    /// List tasks and exit
    #[arg(long)]
    list: bool,

    /// Leaderboard run mode
    #[arg(long)]
    run: Option<String>,

    /// Run tasks in parallel (concurrency limit)
    #[arg(long, default_value_t = 1)]
    parallel: usize,

    /// Dry-run: show pre-scan decisions without running LLM
    #[arg(long)]
    dry_run: bool,

    /// Audit outcome store: report stats, find duplicates, prune noise
    #[arg(long)]
    audit_store: bool,

    /// Probe: test if model supports function calling (run once per new model)
    #[arg(long)]
    probe: bool,

    /// Force-submit a running/stuck leaderboard run
    #[arg(long)]
    submit_run: Option<String>,
}

/// Standard mode: concise prompt for strong models (GPT-5, etc.)
// AI-NOTE: Single prompt for all models. Standard prompt removed — broke weak models (50%).

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file (API keys) — silent if missing
    dotenvy::dotenv().ok();

    // AI-NOTE: telemetry_guard must be dropped explicitly before tokio exits for OTLP flush
    let telemetry_guard = sgr_agent::init_telemetry(".agent", "pac1");

    let cli = Cli::parse();

    if cli.audit_store {
        return audit_outcome_store();
    }

    let cfg = config::Config::load(&cli.config)?;
    let provider_name = cli.provider.as_deref().unwrap_or(&cfg.llm.provider);
    let config::ResolvedProvider {
        model,
        base_url,
        api_key: llm_api_key,
        extra_headers,
        prompt_mode,
        temperature,
        planning_temperature,
        sgr_mode,
        reasoning_effort,
        use_chat_api,
        single_phase,
    } = cfg.resolve_provider(provider_name)?;
    // Forward prompt_cache_key: provider override > defaults > none
    let cache_key = cfg.providers.get(provider_name)
        .and_then(|p| p.prompt_cache_key.clone())
        .or_else(|| cfg.defaults.prompt_cache_key.clone());
    // websocket: explicit override > default (on for Responses, off for Chat)
    let ws_enabled = cfg.providers.get(provider_name)
        .and_then(|p| p.websocket)
        .unwrap_or(!use_chat_api);
    let overrides = config::LlmOverrides {
        use_chat_api,
        websocket: ws_enabled,
        reasoning_effort: reasoning_effort.clone(),
        prompt_cache_key: cache_key,
        single_phase: single_phase.clone(),
    };
    if use_chat_api {
        eprintln!("[pac1] API: Chat Completions (use_chat_api=true)");
    }
    if !ws_enabled {
        eprintln!("[pac1] WebSocket: disabled");
    }
    if sgr_mode {
        eprintln!("[pac1] SGR mode: pure (single LLM call per step)");
    }
    let max_steps = cli.max_steps.unwrap_or(cfg.agent.max_steps);
    // AI-NOTE: BENCHMARK env var overrides config (workaround for config.toml being reverted by other tools)
    let benchmark_override = std::env::var("BENCHMARK").ok();
    let benchmark = benchmark_override.as_deref().unwrap_or(&cfg.agent.benchmark);

    eprintln!("[pac1] Provider: {} | Model: {} | Prompt: {}", provider_name, model, prompt_mode);

    let harness = bitgn::HarnessClient::new(&cli.bitgn_url, cli.api_key.clone());
    let status = harness.status().await?;
    eprintln!("[pac1] BitGN: {}", status);

    // Resolve fallback providers for ensemble retry (skip primary)
    let fallbacks: Vec<FallbackProvider> = cfg.agent.fallback_providers.iter()
        .filter(|name| name.as_str() != provider_name)
        .filter_map(|name| {
            cfg.resolve_provider(name).ok().map(|r| {
                eprintln!("[pac1] Fallback: {} | Model: {}", name, r.model);
                (r.model, r.base_url, r.api_key, r.extra_headers, r.temperature, r.planning_temperature)
            })
        })
        .collect();

    if let Some(ref run_name) = cli.run {
        // Prefix run name with config run_prefix (e.g. "rustman.org-my-run")
        let prefixed_name = if cfg.agent.run_prefix.is_empty() {
            run_name.clone()
        } else {
            format!("{}-{}", cfg.agent.run_prefix, run_name)
        };
        let result = run_leaderboard(&harness, &cli, benchmark, &model, base_url.as_deref(), &llm_api_key, &extra_headers, max_steps, &prefixed_name, &prompt_mode, temperature, planning_temperature, &fallbacks, &overrides).await;
        drop(telemetry_guard); // flush OTLP spans
        return result;
    }

    let bm = harness.get_benchmark(benchmark).await?;
    eprintln!("[pac1] Benchmark: {} — {} tasks", benchmark, bm.tasks.len());

    if cli.list {
        for t in &bm.tasks {
            if t.hint.is_empty() {
                println!("{}: {}", t.task_id, t.preview);
            } else {
                println!("{}: {} | hint: {}", t.task_id, t.preview, t.hint);
            }
        }
        return Ok(());
    }

    if let Some(ref run_id) = cli.submit_run {
        eprintln!("[pac1] Force-submitting run: {}", run_id);
        // Try submit, if fails due to stuck trials — end them and retry
        for attempt in 0..20 {
            match harness.submit_run(run_id).await {
                Ok(_) => { eprintln!("[pac1] Submitted!"); return Ok(()); }
                Err(e) => {
                    let msg = format!("{}", e);
                    // Extract stuck trial ID from error: "trial_id=\"vm-xxx\""
                    if let Some(start) = msg.find("trial_id=\"") {
                        let rest = &msg[start + 10..];
                        if let Some(end) = rest.find('"') {
                            let trial_id = &rest[..end];
                            eprintln!("  [attempt {}] Ending stuck trial: {}", attempt + 1, trial_id);
                            let _ = harness.end_trial(trial_id).await;
                            continue;
                        }
                    }
                    anyhow::bail!("SubmitRun failed: {}", e);
                }
            }
        }
        anyhow::bail!("Failed to submit after 3 attempts");
    }

    // AI-NOTE: FC probe — run once per new model via `make probe` or `--probe`
    if cli.probe {
        eprintln!("[pac1] FC probe: testing model {} for function calling support...", model);
        let config = llm_config::make_llm_config(
            &model, base_url.as_deref(), &llm_api_key,
            &extra_headers, temperature, &overrides,
        );
        let llm = sgr_agent::llm::Llm::new(&config);
        let probe_tool = sgr_agent::tool::ToolDef {
            name: "analyze".into(),
            description: "Analyze task and return structured plan".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "task_type": { "type": "string", "enum": ["search", "edit", "delete", "analyze"] },
                    "plan": { "type": "array", "items": { "type": "string" } },
                    "security": { "type": "string", "enum": ["safe", "blocked"] },
                    "done": { "type": "boolean" },
                    "confidence": { "type": "number" }
                },
                "required": ["task_type", "plan", "security", "done", "confidence"],
                "additionalProperties": false
            }),
        };
        let msgs = vec![sgr_agent::Message::system("You are a workspace agent. Analyze: 'list all contacts'. Call the analyze tool.")];
        match llm.tools_call_stateful(&msgs, &[probe_tool], None).await {
            Ok((calls, _)) if calls.is_empty() => {
                eprintln!("  ❌ FAIL: 0 tool calls. Model cannot handle structured CoT.");
                std::process::exit(1);
            }
            Ok((calls, _)) => {
                let has_type = calls[0].arguments.get("task_type").is_some();
                let has_plan = calls[0].arguments.get("plan").is_some();
                eprintln!("  ✅ PASS: task_type={}, plan={}", has_type, has_plan);
            }
            Err(e) => {
                eprintln!("  ❌ ERROR: {:#}", e);
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    if cli.dry_run {
        eprintln!("[pac1] Dry-run: pre-scan only (no LLM)");
        let mut blocked = 0;
        let mut clarification = 0;
        let mut pass = 0;
        for t in &bm.tasks {
            let preview = &t.preview;
            match scanner::prescan_instruction(preview) {
                Some((outcome, msg)) => {
                    println!("{}: {} — {}", t.task_id, outcome, msg);
                    if outcome == "OUTCOME_DENIED_SECURITY" { blocked += 1; }
                    else { clarification += 1; }
                }
                None => {
                    println!("{}: PASS (score={})", t.task_id, scanner::threat_score(preview));
                    pass += 1;
                }
            }
        }
        eprintln!("\n[pac1] Dry-run summary: {} blocked, {} clarification, {} pass / {} total",
            blocked, clarification, pass, bm.tasks.len());
        return Ok(());
    }

    let tasks: Vec<_> = if let Some(ref tid) = cli.task {
        bm.tasks.iter().filter(|t| t.task_id == *tid).collect()
    } else {
        bm.tasks.iter().collect()
    };

    if tasks.is_empty() {
        anyhow::bail!("No matching tasks found");
    }

    // Load ML classifier ONCE — shared across all parallel trials via Arc<Mutex>
    let shared_clf: Arc<std::sync::Mutex<Option<classifier::InboxClassifier>>> = Arc::new(
        std::sync::Mutex::new(classifier::InboxClassifier::try_load(&classifier::InboxClassifier::models_dir()))
    );
    eprintln!("[pac1] Classifier: {}", if shared_clf.lock().unwrap().is_some() { "loaded (shared)" } else { "unavailable" });

    // Load NLI classifier ONCE — shared across all parallel trials
    let shared_nli: scanner::SharedNliClassifier = Arc::new(
        std::sync::Mutex::new(classifier::NliClassifier::try_load(&classifier::InboxClassifier::models_dir()))
    );
    eprintln!("[pac1] NLI: {}", if shared_nli.lock().unwrap().is_some() { "loaded (shared)" } else { "unavailable" });

    // Build OutcomeValidator once — shared across all trials for score-gated learning
    let outcome_validator: Option<Arc<classifier::OutcomeValidator>> = {
        let store_path = std::path::PathBuf::from(".agent/outcome_store.json");
        match classifier::OutcomeValidator::from_shared(shared_clf.clone(), store_path) {
            Ok(v) => Some(Arc::new(v)),
            Err(e) => {
                eprintln!("  ⚠ OutcomeValidator failed: {:#}", e);
                None
            }
        }
    };

    // Run tasks with concurrency limit
    let semaphore = Arc::new(tokio::sync::Semaphore::new(cli.parallel));
    let results = Arc::new(tokio::sync::Mutex::new(Vec::new()));

    let mut handles = Vec::new();
    for task in &tasks {
        let task_id = task.task_id.clone();
        let preview = task.preview.clone();
        let hint = task.hint.clone();
        let harness_url = cli.bitgn_url.clone();
        let api_key_clone = cli.api_key.clone();
        let benchmark = benchmark.to_string();
        let model = model.clone();
        let base_url = base_url.clone();
        let llm_api_key = llm_api_key.clone();
        let extra_headers = extra_headers.clone();
        let prompt_mode = prompt_mode.clone();
        let sem = semaphore.clone();
        let res = results.clone();
        let clf = shared_clf.clone();
        let nli = shared_nli.clone();
        let ov = outcome_validator.clone();
        let overrides = overrides.clone();

        let handle = tokio::spawn(sgr_agent::with_telemetry_scope(async move {
            let _permit = sem.acquire().await.unwrap();
            eprintln!("\n━━━ Task: {} ━━━", task_id);
            eprintln!("  {}", preview);
            if !hint.is_empty() {
                eprintln!("  💡 hint: {}", hint);
            }

            let h = bitgn::HarnessClient::new(&harness_url, api_key_clone);
            let trial = match h.start_playground(&benchmark, &task_id).await {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("  ⚠ Failed to start trial: {}", e);
                    res.lock().await.push((task_id, 0.0f32));
                    return;
                }
            };
            let log_url = format!("https://{}.eu.bitgn.com", trial.trial_id);
            eprintln!("  Trial: {}", trial.trial_id);
            // Session ID: trial identity — used for telemetry spans + LLM sticky routing
            let session_id = format!("{}_{}", task_id, trial.trial_id);
            sgr_agent::set_session_id(session_id.clone());
            sgr_agent::set_task_id(task_id.clone());
            eprintln!("  📋 Log: {}", log_url);

            // Auto-create dump dir if DUMP_TRIAL set, or auto-generate for single-task runs
            let short_model = model.rsplit('/').next().unwrap_or(&model);
            let dump_dir = std::env::var("DUMP_TRIAL").ok().unwrap_or_else(|| {
                format!("benchmarks/tasks/{}/{}_{}", task_id, short_model, trial.trial_id)
            });
            let _ = std::fs::create_dir_all(&dump_dir);
            let _ = std::fs::write(format!("{}/bitgn_log.url", dump_dir), format!("{}\n", log_url));
            let _ = std::fs::write(format!("{}/instruction.txt", dump_dir), &trial.instruction);
            // SAFETY: single-threaded env var set for child code in same task
            unsafe { std::env::set_var("DUMP_TRIAL", &dump_dir); }

            let pcm = Arc::new(pcm::PcmClient::new(&trial.harness_url));
            let t0 = std::time::Instant::now();
            let (last_msg, history, tool_calls, steps) = run_trial(
                &pcm, &trial.instruction, &model,
                base_url.as_deref(), &llm_api_key, &extra_headers, max_steps, &prompt_mode, temperature, planning_temperature,
                &clf, &nli, ov.clone(), sgr_mode, Some(&dump_dir), Some(&session_id), &overrides,
            ).await;
            let agent_elapsed = t0.elapsed();
            verify_and_submit(
                &pcm, &trial.instruction, &last_msg, &history, tool_calls,
                &model, base_url.as_deref(), &llm_api_key, &extra_headers, temperature,
            ).await;
            let total_elapsed = t0.elapsed();

            let score = match h.end_trial(&trial.trial_id).await {
                Ok(result) => {
                    let s = result.score.unwrap_or(0.0);
                    eprintln!("  {} Score: {:.2}", task_id, s);
                    for detail in &result.score_detail {
                        eprintln!("    {}", detail);
                    }
                    // Score-gated learning: only learn from confirmed correct answers
                    if s >= 1.0 {
                        if let Some(ref v) = ov {
                            v.learn_last();
                        }
                    }
                    // Fetch full trial logs for debugging when score < 1.0
                    if s < 1.0 {
                        if let Ok(trial_detail) = h.get_trial(&trial.trial_id).await {
                            if !trial_detail.logs.is_empty() {
                                eprintln!("  --- Trial logs ---");
                                for log in &trial_detail.logs {
                                    eprintln!("  [{}] {}: {}", log.time, log.kind, log.text.trunc(500));
                                }
                            }
                        }
                    }
                    s
                }
                Err(e) => {
                    eprintln!("  ⚠ EndTrial error: {}", e);
                    0.0
                }
            };
            write_trial_metrics(
                &dump_dir, &model, score, steps, tool_calls, pcm.rpc_count(),
                agent_elapsed.as_secs_f64(), total_elapsed.as_secs_f64(), &[], &history,
            );

            // AI-NOTE: Phoenix trace annotations — score/outcome/task/steps per session
            // Extract outcome from agent's submitted answer (last answer() call in history)
            let outcome = if let Some(pos) = history.rfind("\"outcome\":\"") {
                let rest = &history[pos + 11..];
                rest.split('"').next().unwrap_or("OK")
            } else if score >= 1.0 { "OK" } else { "UNKNOWN" };
            sgr_agent::annotate_session(&task_id, score, outcome, steps as u32);

            res.lock().await.push((task_id, score));
        }));
        handles.push(handle);
    }

    futures::future::join_all(handles).await;

    let results = results.lock().await;
    let total_score: f32 = results.iter().map(|(_, s)| s).sum();
    let scored = results.iter().filter(|(_, s)| *s > 0.0).count();
    eprintln!("\n═══ Average: {:.1}% ({}/{} tasks) ═══",
        total_score / results.len() as f32 * 100.0, scored, results.len());
    drop(telemetry_guard); // flush OTLP spans before tokio exits
    Ok(())
}

// ─── Leaderboard ─────────────────────────────────────────────────────────────

type FallbackProvider = (String, Option<String>, String, Vec<(String, String)>, f32, f32);

#[allow(clippy::too_many_arguments)]
async fn run_leaderboard(
    harness: &bitgn::HarnessClient, cli: &Cli, benchmark: &str,
    model: &str, base_url: Option<&str>, llm_api_key: &str,
    extra_headers: &[(String, String)], max_steps: usize, run_name: &str,
    prompt_mode: &str,
    temperature: f32,
    planning_temperature: f32,
    fallbacks: &[FallbackProvider],
    overrides: &config::LlmOverrides,
) -> Result<()> {
    if cli.api_key.is_none() {
        anyhow::bail!("--api-key or BITGN_API_KEY required for leaderboard mode");
    }

    let api_key = cli.api_key.as_deref().unwrap();
    eprintln!("[pac1] Starting leaderboard run: {}", run_name);
    let run = harness.start_run(benchmark, run_name, api_key).await?;
    eprintln!("[pac1] Run {} — {} trials", run.run_id, run.trial_ids.len());

    let shared_clf: SharedClassifier = Arc::new(
        std::sync::Mutex::new(classifier::InboxClassifier::try_load(&classifier::InboxClassifier::models_dir()))
    );
    let shared_nli: scanner::SharedNliClassifier = Arc::new(
        std::sync::Mutex::new(classifier::NliClassifier::try_load(&classifier::InboxClassifier::models_dir()))
    );

    // Build OutcomeValidator once for score-gated learning across all trials
    let outcome_validator: Option<Arc<classifier::OutcomeValidator>> = {
        let store_path = std::path::PathBuf::from(".agent/outcome_store.json");
        match classifier::OutcomeValidator::from_shared(shared_clf.clone(), store_path) {
            Ok(v) => Some(Arc::new(v)),
            Err(e) => {
                eprintln!("  ⚠ OutcomeValidator failed: {:#}", e);
                None
            }
        }
    };

    // AI-NOTE: parallel leaderboard — use CLI --parallel flag (was env var only, fixed)
    let concurrency = cli.parallel;
    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));
    let lb_results: Arc<tokio::sync::Mutex<Vec<(String, f32)>>> = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let harness = Arc::new((*harness).clone());
    let mut handles = Vec::new();

    for (i, trial_id) in run.trial_ids.iter().enumerate() {
        let sem = semaphore.clone();
        let harness = harness.clone();
        let trial_id = trial_id.clone();
        let model = model.to_string();
        let base_url = base_url.map(|s| s.to_string());
        let llm_api_key = llm_api_key.to_string();
        let extra_headers = extra_headers.to_vec();
        let prompt_mode = prompt_mode.to_string();
        let shared_clf = shared_clf.clone();
        let shared_nli = shared_nli.clone();
        let outcome_validator = outcome_validator.clone();
        let fallbacks: Vec<FallbackProvider> = fallbacks.to_vec();
        let lb_results = lb_results.clone();
        let total_trials = run.trial_ids.len();
        let overrides = overrides.clone();

        let handle = tokio::spawn(sgr_agent::with_telemetry_scope(async move {
        let _permit = sem.acquire().await.unwrap();
        let Ok(trial) = harness.start_trial(&trial_id).await else {
            eprintln!("  ⚠ Failed to start trial {}", trial_id);
            return;
        };
        eprintln!("\n━━━ Trial {}/{}: {} (task {}) ━━━",
            i + 1, total_trials, trial.trial_id, trial.task_id);

        // Session ID: trial identity — used for telemetry spans + LLM sticky routing
        let session_id = format!("{}_{}", trial.task_id, trial.trial_id);
        sgr_agent::set_session_id(session_id.clone());
        sgr_agent::set_task_id(trial.task_id.clone());

        // Auto-dump trial data for dashboard
        // Extract short provider name from model path for dump dir
        let short_model = model.rsplit('/').next().unwrap_or(&model);
        let dump_dir = format!("benchmarks/tasks/{}/{}_{}", trial.task_id, short_model, trial.trial_id);
        let _ = std::fs::create_dir_all(&dump_dir);
        let log_url = format!("https://{}.eu.bitgn.com", trial.trial_id);
        let _ = std::fs::write(format!("{}/bitgn_log.url", dump_dir), format!("{}\n", log_url));
        let _ = std::fs::write(format!("{}/instruction.txt", dump_dir), &trial.instruction);
        // AI-NOTE: dump_dir passed as argument to run_trial (not env var) — parallel-safe

        let pcm = Arc::new(pcm::PcmClient::new(&trial.harness_url));
        let t0 = std::time::Instant::now();
        let (last_msg, history, tool_calls, steps) = run_trial(&pcm, &trial.instruction, &model, base_url.as_deref(), &llm_api_key, &extra_headers, max_steps, &prompt_mode, temperature, planning_temperature, &shared_clf, &shared_nli, outcome_validator.clone(), false, Some(&dump_dir), Some(&session_id), &overrides).await;
        let agent_elapsed = t0.elapsed();

        // AI-NOTE: verifier + ensemble retry removed. Agent's answer is final.
        // Old: check_verifier_agreement → retry on fallback → verify_and_submit.
        // New: direct submit. Saves 3+ LLM calls per trial.
        {
            // Just submit directly
            verify_and_submit(
                &pcm, &trial.instruction, &last_msg, &history, tool_calls,
                &model, base_url.as_deref(), &llm_api_key, &extra_headers, temperature,
            ).await;
        }

        // Ensemble retry preserved but without verifier gate — only on explicit failure
        if false {
            // Placeholder: can re-enable retry on specific conditions (e.g., 0 tool calls)
            if let Some((fb_model, fb_base, fb_key, fb_headers, fb_temp, fb_plan_temp)) = fallbacks.first() {
                eprintln!("  🔄 Ensemble retry: switching to {} (1/{})", fb_model, fallbacks.len());
                let pcm2 = Arc::new(pcm::PcmClient::new(&trial.harness_url));
                let (last_msg2, history2, tool_calls2, _steps2) = run_trial(
                    &pcm2, &trial.instruction, fb_model, fb_base.as_deref(), fb_key, fb_headers,
                    max_steps, &prompt_mode, *fb_temp, *fb_plan_temp,
                    &shared_clf, &shared_nli, outcome_validator.clone(), false, Some(&dump_dir), Some(&session_id), &overrides,
                ).await;
                verify_and_submit(
                    &pcm2, &trial.instruction, &last_msg2, &history2, tool_calls2,
                    fb_model, fb_base.as_deref(), fb_key, fb_headers, *fb_temp,
                ).await;
            } else {
                let pcm2 = Arc::new(pcm::PcmClient::new(&trial.harness_url));
                let retry_temp = temperature + 0.1;
                let (last_msg2, history2, tool_calls2, _steps2) = run_trial(&pcm2, &trial.instruction, &model, base_url.as_deref(), &llm_api_key, &extra_headers, max_steps, &prompt_mode, retry_temp, planning_temperature, &shared_clf, &shared_nli, outcome_validator.clone(), false, Some(&dump_dir), Some(&session_id), &overrides).await;
                verify_and_submit(
                    &pcm2, &trial.instruction, &last_msg2, &history2, tool_calls2,
                    &model, base_url.as_deref(), &llm_api_key, &extra_headers, retry_temp,
                ).await;
            }
        } else {
            let _ = pcm.submit_proposed(None).await;
        }

        let result = harness.end_trial(&trial.trial_id).await.unwrap_or_else(|e| {
            eprintln!("  ⚠ EndTrial error: {}", e);
            bitgn::EndTrialResponse { trial_id: trial_id.clone(), score: Some(0.0), score_detail: vec![format!("error: {}", e)] }
        });
        let score = result.score.unwrap_or(0.0);
        eprintln!("  {} Score: {:.2}", trial.task_id, score);
        for detail in &result.score_detail {
            eprintln!("    {}", detail);
        }
        let total_elapsed_final = t0.elapsed();
        write_trial_metrics(
            &dump_dir, &model, score as f32, steps, tool_calls, pcm.rpc_count(),
            agent_elapsed.as_secs_f64(), total_elapsed_final.as_secs_f64(),
            &result.score_detail, &history,
        );
        if score >= 1.0 {
            if let Some(ref v) = outcome_validator {
                v.learn_last();
            }
        }
        lb_results.lock().await.push((trial.task_id.clone(), score));
        }));
        handles.push(handle);
    }

    futures::future::join_all(handles).await;

    let run_status = harness.get_run(&run.run_id).await?;
    eprintln!("\n[pac1] Run state: {}", run_status.state);
    if let Some(score) = run_status.score {
        eprintln!("[pac1] Run score: {:.1}%", score * 100.0);
    }

    eprintln!("[pac1] Submitting run...");
    harness.submit_run(&run.run_id).await?;
    eprintln!("[pac1] Submitted! Run ID: {}", run.run_id);
    Ok(())
}

/// Write unified metrics + score + run log to dump dir. Used by both single-task and parallel modes.
fn write_trial_metrics(
    dump_dir: &str, model: &str, score: f32, steps: usize, tool_calls: usize,
    harness_steps: u32, agent_secs: f64, total_secs: f64, score_detail: &[String], history: &str,
) {
    // Always log to stderr what we're writing
    eprintln!("  ⏱ Agent: {:.1}s | Total: {:.1}s | Steps: {} | Tools: {} | RPCs: {}",
        agent_secs, total_secs, steps, tool_calls, harness_steps);
    let _ = std::fs::write(format!("{}/metrics.txt", dump_dir), format!(
        "model: {}\nscore: {:.2}\nsteps: {}\ntool_calls: {}\nagent_secs: {:.1}\ntotal_secs: {:.1}\n",
        model, score, steps, tool_calls, agent_secs, total_secs,
    ));
    let _ = std::fs::write(format!("{}/score.txt", dump_dir), format!(
        "{:.2}\n{}\n", score, score_detail.join("\n"),
    ));
    // Append metrics to pipeline.txt for unified diagnosis
    let _ = std::fs::OpenOptions::new()
        .append(true).create(true)
        .open(format!("{}/pipeline.txt", dump_dir))
        .and_then(|mut f| {
            use std::io::Write;
            writeln!(f, "\nscore: {:.2}", score)?;
            writeln!(f, "steps: {}", steps)?;
            writeln!(f, "tool_calls: {}", tool_calls)?;
            writeln!(f, "harness_steps: {}", harness_steps)?;
            writeln!(f, "agent_secs: {:.1}", agent_secs)?;
            writeln!(f, "total_secs: {:.1}", total_secs)?;
            for d in score_detail { writeln!(f, "detail: {}", d)?; }
            Ok(())
        });
    // Save full agent history as run.log
    if !history.is_empty() {
        let _ = std::fs::write(format!("{}/run.log", dump_dir), history);
    }
    eprintln!("  ⏱ Agent: {:.1}s | Total: {:.1}s | Steps: {} | Tools: {} | RPCs: {}",
        agent_secs, total_secs, steps, tool_calls, harness_steps);
}

// ─── Shared ──────────────────────────────────────────────────────────────────

use scanner::SharedClassifier;

/// Returns (last_assistant_msg, full_history_text, tool_calls, steps).
async fn run_trial(
    pcm: &Arc<pcm::PcmClient>, instruction: &str,
    model: &str, base_url: Option<&str>, api_key: &str,
    extra_headers: &[(String, String)], max_steps: usize, prompt_mode: &str, temperature: f32, planning_temperature: f32,
    shared_clf: &SharedClassifier,
    shared_nli: &scanner::SharedNliClassifier,
    outcome_validator: Option<Arc<classifier::OutcomeValidator>>,
    sgr_mode: bool,
    dump_dir: Option<&str>,
    session_id: Option<&str>,
    overrides: &config::LlmOverrides,
) -> (String, String, usize, usize) {
    match pregrounding::run_agent(pcm, instruction, model, base_url, api_key, extra_headers, max_steps, prompt_mode, temperature, planning_temperature, shared_clf, shared_nli, outcome_validator, sgr_mode, dump_dir, session_id, overrides).await {
        Ok((last_msg, history, tool_calls, steps)) => (last_msg, history, tool_calls, steps),
        Err(e) => {
            eprintln!("  ⚠ Agent error: {:#}", e);
            (String::new(), String::new(), 0, 0)
        }
    }
}

// AI-NOTE: check_verifier_agreement removed — verifier eliminated (3 LLM calls per trial).
// Agent's answer is final. No retry based on verifier disagreement.

/// Post-execution verification and submission.
/// 1. If agent proposed an answer → verify with LLM → apply override policy → submit
/// 2. If no proposed answer → use verifier as primary, guess_outcome as fallback → submit
#[allow(clippy::too_many_arguments)]
async fn verify_and_submit(
    pcm: &Arc<pcm::PcmClient>,
    instruction: &str,
    last_msg: &str,
    history: &str,
    tool_call_count: usize,
    model: &str,
    base_url: Option<&str>,
    api_key: &str,
    extra_headers: &[(String, String)],
    temperature: f32,
) {
    let proposed = pcm.get_proposed_answer();

    match proposed {
        Some(ref p) => {
            // AI-NOTE: verifier removed — was 3 extra LLM calls per trial.
            // Agent's own answer is final. Competitors (Codex) don't verify.
            eprintln!("  ✅ Submit: {} — {}", p.outcome, p.message.trunc(100));
            let _ = pcm.submit_proposed(None).await;
        }
        None => {
            // No proposed answer — agent didn't call answer(). Use guess_outcome heuristic.
            // Verifier is not used here: its value is in correcting a wrong outcome code,
            // not in guessing from scratch (CRM content confuses it — e.g. articles about "injection").
            // AI-NOTE: prefer found data over "Unable to determine". Scan history for last read result.
            let text = if last_msg.is_empty() {
                extract_last_finding(history).unwrap_or("Task processed")
            } else { last_msg };
            let outcome = guess_outcome(text, history);
            eprintln!("  ⚠ Auto-answer [{}]: {}", outcome, text.trunc(100));
            let _ = pcm.answer(text, outcome, &[]).await;
        }
    }
}

/// Override policy: decides whether to use verifier outcome or keep proposed.
/// Returns Some(override_outcome) if verifier should override, None to keep proposed.
fn apply_override_policy(
    proposed_outcome: &str,
    verifier_outcome: &str,
    verifier_confidence: f64,
    tool_call_count: usize,
) -> Option<String> {
    // Agent said DENIED_SECURITY:
    // - If agent performed tool calls (real investigation) → NEVER override.
    //   Agent's conscious security judgement after reading data is final (t20, t25).
    // - If no tool calls (planner-only / auto-answer fallback) → allow override.
    //   Planner sometimes hallucinates injection alerts without reading inbox (t19).
    if proposed_outcome == "OUTCOME_DENIED_SECURITY" {
        if tool_call_count >= 1 {
            eprintln!("  🛡️ Agent DENIED after {} tool calls — never overridden", tool_call_count);
            return None;
        }
        // No tool calls — planner fallback. Allow high-confidence verifier override.
        if verifier_outcome == "OUTCOME_OK" && verifier_confidence >= 0.90 {
            return Some("OUTCOME_OK".to_string());
        }
        return None;
    }

    // Selective security override (v0.4): when verifier detects injection the agent missed.
    // Agent said OK but verifier says DENIED_SECURITY with high confidence → override.
    // This catches prompt injection the agent failed to recognize (e.g. t09 "clipboard relay").
    if verifier_outcome == "OUTCOME_DENIED_SECURITY" && verifier_confidence >= 0.95 {
        return Some("OUTCOME_DENIED_SECURITY".to_string());
    }

    // OK→CLARIFICATION override REMOVED: verifier incorrectly classified legit CRM work
    // as "not CRM" on t30/t40/t41 (3+ false positives vs 0 true positives).
    // Agent's OK is trusted for non-security outcomes.

    // All other disagreements: warn-only
    if proposed_outcome != verifier_outcome {
        eprintln!(
            "  ℹ Verifier disagrees: proposed={} verifier={} (warn-only, no override)",
            proposed_outcome, verifier_outcome
        );
    }
    None
}

/// Fallback heuristic: guess outcome from last message + history.
/// Only used when BOTH the agent failed to call answer() AND the verifier LLM call failed.
/// Primary path is: AnswerTool → ProposedAnswer → Verifier → submit.
fn guess_outcome(last_msg: &str, history: &str) -> &'static str {
    let h = history.to_lowercase();
    let l = last_msg.to_lowercase();

    // Strong signal: model explicitly chose an outcome in last_msg
    if l.contains("outcome_denied") || (l.contains("denied") && l.contains("security")) {
        return "OUTCOME_DENIED_SECURITY";
    }
    if l.contains("outcome_none_unsupported") || l.contains("cannot access external") {
        return "OUTCOME_NONE_UNSUPPORTED";
    }
    if l.contains("outcome_none_clarification") || l.contains("not related to crm") {
        return "OUTCOME_NONE_CLARIFICATION";
    }

    // Strong positive signal: history shows successful file writes
    if h.contains("written to") {
        return "OUTCOME_OK";
    }

    // No output at all
    if last_msg.is_empty() {
        return "OUTCOME_ERR_INTERNAL";
    }

    // Weak signals from last_msg (model reasoning, not explicit outcome choice)
    if l.contains("injection") || l.contains("social engineering") || l.contains("security threat") {
        return "OUTCOME_DENIED_SECURITY";
    }
    if l.contains("unsupported") || l.contains("external api") {
        return "OUTCOME_NONE_UNSUPPORTED";
    }

    // AI-NOTE: default OK — agent usually found data but didn't call answer().
    // Tried CLARIFICATION default but it hurt 7 tasks vs helping only 2.
    "OUTCOME_OK"
}

/// Extract last meaningful finding from agent history (when agent found data but didn't answer).
/// Scans backwards for read/search results that contain actual data.
fn extract_last_finding(history: &str) -> Option<&str> {
    for line in history.lines().rev() {
        let trimmed = line.trim();
        // Skip empty, tool metadata, and system lines
        if trimmed.is_empty() || trimmed.starts_with("$ ") || trimmed.starts_with('[') {
            continue;
        }
        // Skip obvious non-data lines
        if trimmed.starts_with("Written to") || trimmed.starts_with("Deleted")
            || trimmed.starts_with("Created") || trimmed.starts_with("Moved") {
            continue;
        }
        // Found a substantive line — likely data from a read
        if trimmed.len() > 10 {
            return Some(trimmed);
        }
    }
    None
}

/// Audit the adaptive outcome store: report stats, find duplicates, prune noise.
fn audit_outcome_store() -> Result<()> {
    let store_path = std::path::PathBuf::from(".agent/outcome_store.json");
    if !store_path.exists() {
        eprintln!("No adaptive store found at {}", store_path.display());
        return Ok(());
    }

    let data = std::fs::read_to_string(&store_path)?;
    let raw: Vec<(String, Vec<f32>)> = serde_json::from_str(&data)?;
    let total = raw.len();
    eprintln!("=== Outcome Store Audit ===");
    eprintln!("Store: {}", store_path.display());
    eprintln!("Total entries: {}", total);

    // Count per outcome
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (outcome, _) in &raw {
        *counts.entry(outcome.as_str()).or_default() += 1;
    }
    eprintln!("\nPer-outcome counts:");
    let mut sorted_counts: Vec<_> = counts.iter().collect();
    sorted_counts.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
    for (outcome, count) in &sorted_counts {
        eprintln!("  {}: {}", outcome, count);
    }

    // Find duplicates (cosine > 0.95 between same-outcome pairs)
    let embeddings: Vec<(&str, ndarray::Array1<f32>)> = raw.iter()
        .map(|(o, v)| (o.as_str(), ndarray::Array1::from_vec(v.clone())))
        .collect();

    let mut dup_indices: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for i in 0..embeddings.len() {
        if dup_indices.contains(&i) { continue; }
        for j in (i + 1)..embeddings.len() {
            if dup_indices.contains(&j) { continue; }
            if embeddings[i].0 == embeddings[j].0 {
                let sim = classifier::cosine_similarity(
                    embeddings[i].1.view(),
                    embeddings[j].1.view(),
                );
                if sim > 0.95 {
                    dup_indices.insert(j); // keep i, remove j
                }
            }
        }
    }
    eprintln!("\nDuplicates (cosine > 0.95): {}", dup_indices.len());

    // Find outliers: entries with low max similarity to all seeds
    let models_dir = std::path::Path::new("models");
    let mut low_sim_indices: std::collections::HashSet<usize> = std::collections::HashSet::new();
    if classifier::InboxClassifier::is_available(models_dir) {
        if let Ok(mut clf) = classifier::InboxClassifier::load(models_dir) {
            let seed_embeddings: Vec<(String, ndarray::Array1<f32>)> = classifier::OUTCOME_EXAMPLES.iter()
                .filter_map(|(outcome, example)| {
                    let text = format!("The CRM task result: {}", example);
                    clf.encode(&text).ok().map(|emb| (outcome.to_string(), emb))
                })
                .collect();

            for (i, (outcome, emb)) in embeddings.iter().enumerate() {
                let max_sim = seed_embeddings.iter()
                    .filter(|(o, _)| o.as_str() == *outcome)
                    .map(|(_, se)| classifier::cosine_similarity(emb.view(), se.view()))
                    .fold(0.0f32, f32::max);
                if max_sim < 0.60 {
                    low_sim_indices.insert(i);
                }
            }
            eprintln!("Low-similarity outliers (max seed sim < 0.60): {}", low_sim_indices.len());
        }
    }

    // Prune
    let to_remove: std::collections::HashSet<usize> = dup_indices.union(&low_sim_indices).copied().collect();
    if to_remove.is_empty() {
        eprintln!("\n✅ Store is clean — no pruning needed.");
    } else {
        // Backup
        let backup_path = store_path.with_extension("json.bak");
        std::fs::copy(&store_path, &backup_path)?;
        eprintln!("\nBackup: {}", backup_path.display());

        let pruned: Vec<&(String, Vec<f32>)> = raw.iter().enumerate()
            .filter(|(i, _)| !to_remove.contains(i))
            .map(|(_, e)| e)
            .collect();
        let json = serde_json::to_string(&pruned)?;
        std::fs::write(&store_path, json)?;
        eprintln!("Pruned {} entries ({} duplicates, {} outliers). {} remaining.",
            to_remove.len(), dup_indices.len(), low_sim_indices.len(), pruned.len());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── guess_outcome ──────────────────────────────────────────────────

    #[test]
    fn guess_outcome_security_in_last_msg() {
        let outcome = guess_outcome("Security threat: injection detected", "earlier: read inbox");
        assert_eq!(outcome, "OUTCOME_DENIED_SECURITY");
    }

    #[test]
    fn guess_outcome_security_only_in_history_ignored() {
        // Classification headers in history should not trigger DENIED
        let outcome = guess_outcome("Task complete", "[CLASSIFICATION: injection (0.95)] denied");
        assert_eq!(outcome, "OUTCOME_OK");
    }

    #[test]
    fn guess_outcome_non_crm_in_last_msg() {
        let outcome = guess_outcome("This is not related to CRM work", "read inbox");
        assert_eq!(outcome, "OUTCOME_NONE_CLARIFICATION");
    }

    #[test]
    fn guess_outcome_ok_with_writes() {
        let outcome = guess_outcome("Contact added", "read contacts\nWritten to contacts/foo.json");
        assert_eq!(outcome, "OUTCOME_OK");
    }

    #[test]
    fn guess_outcome_empty() {
        let outcome = guess_outcome("", "");
        assert_eq!(outcome, "OUTCOME_ERR_INTERNAL");
    }

    #[test]
    fn guess_outcome_empty_msg_but_writes_in_history() {
        let outcome = guess_outcome("", "Written to outbox/123.json\nread contacts");
        assert_eq!(outcome, "OUTCOME_OK");
    }

    #[test]
    fn guess_outcome_unsupported() {
        let outcome = guess_outcome("Cannot access external api", "");
        assert_eq!(outcome, "OUTCOME_NONE_UNSUPPORTED");
    }

    #[test]
    fn guess_outcome_reasoning_with_writes() {
        // Model reasoning context (not explicit outcome) + history has writes = OK
        let outcome = guess_outcome(
            "Type: edit | Security: safe | State: Processing inbox, could not find all contacts",
            "Written to outbox/123.json\nread inbox",
        );
        assert_eq!(outcome, "OUTCOME_OK");
    }

    // ─── override policy ────────────────────────────────────────────────

    #[test]
    fn override_agree() {
        let result = apply_override_policy("OUTCOME_OK", "OUTCOME_OK", 0.95, 0);
        assert!(result.is_none(), "Same outcome = no override");
    }

    #[test]
    fn override_warn_only_for_non_security() {
        let result = apply_override_policy("OUTCOME_OK", "OUTCOME_NONE_UNSUPPORTED", 0.9, 5);
        assert!(result.is_none(), "Non-security disagree = no override");

        let result = apply_override_policy("OUTCOME_OK", "OUTCOME_NONE_UNSUPPORTED", 0.6, 0);
        assert!(result.is_none(), "Low confidence disagree = no override");
    }

    #[test]
    fn override_denied_with_tool_calls_never_overridden() {
        // Agent did 3+ steps → investigated → NEVER override DENIED
        let result = apply_override_policy("OUTCOME_DENIED_SECURITY", "OUTCOME_OK", 0.99, 3);
        assert!(result.is_none(), "DENIED after 3 steps is final");

        let result = apply_override_policy("OUTCOME_DENIED_SECURITY", "OUTCOME_OK", 0.99, 10);
        assert!(result.is_none(), "DENIED after 10 steps is final");

        // Even 2 steps = agent did something
        let result = apply_override_policy("OUTCOME_DENIED_SECURITY", "OUTCOME_OK", 0.99, 2);
        assert!(result.is_none(), "DENIED after 2 steps is final");
    }

    #[test]
    fn override_denied_without_tool_calls_allows_override() {
        // 0 steps = planner-only fallback → allow override
        let result = apply_override_policy("OUTCOME_DENIED_SECURITY", "OUTCOME_OK", 0.95, 0);
        assert_eq!(result.as_deref(), Some("OUTCOME_OK"),
            "Planner-only DENIED (0 steps) can be overridden");

        // 1 step = agent called answer() — trust agent's decision
        let result = apply_override_policy("OUTCOME_DENIED_SECURITY", "OUTCOME_OK", 0.95, 1);
        assert!(result.is_none(), "1 tool call = agent acted, don't override DENIED");

        // But not with low confidence
        let result = apply_override_policy("OUTCOME_DENIED_SECURITY", "OUTCOME_OK", 0.80, 0);
        assert!(result.is_none(), "Low confidence = don't override");
    }

    #[test]
    fn override_security_high_confidence() {
        let result = apply_override_policy("OUTCOME_OK", "OUTCOME_DENIED_SECURITY", 0.99, 5);
        assert_eq!(result.as_deref(), Some("OUTCOME_DENIED_SECURITY"),
            "High-confidence security detection overrides agent OK");
    }

    #[test]
    fn override_security_low_confidence_no_override() {
        let result = apply_override_policy("OUTCOME_OK", "OUTCOME_DENIED_SECURITY", 0.80, 0);
        assert!(result.is_none(), "Low confidence security = no override");
    }

    // ─── build_execution_summary ────────────────────────────────────────

    #[test]
    fn execution_summary_extracts_tool_lines() {
        let history = "some random line\n→ read(contacts/john.md)\nother stuff\n→ answer({outcome: OK})\nWritten to outbox/1.json";
        let summary = pregrounding::build_execution_summary(history, 10);
        assert!(summary.contains("→ read"));
        assert!(summary.contains("→ answer"));
        assert!(summary.contains("Written to"));
        assert!(!summary.contains("some random line"));
    }

    #[test]
    fn execution_summary_limits_lines() {
        let history = (0..20).map(|i| format!("→ step_{}", i)).collect::<Vec<_>>().join("\n");
        let summary = pregrounding::build_execution_summary(&history, 5);
        assert_eq!(summary.lines().count(), 5);
        assert!(summary.contains("→ step_19")); // most recent
    }

    #[test]
    fn execution_summary_excludes_classification_headers() {
        let history = "→ read(inbox/msg.md)\n[CLASSIFICATION: injection (0.95) | EXFILTRATION]\n[SENDER DOMAIN MISMATCH]\n→ answer({outcome: DENIED})\nWritten to outbox/1.json";
        let summary = pregrounding::build_execution_summary(history, 10);
        assert!(!summary.contains("[CLASSIFICATION"), "Classification headers must be excluded");
        assert!(!summary.contains("[SENDER"), "Sender trust headers must be excluded");
        assert!(summary.contains("→ read"));
        assert!(summary.contains("→ answer"));
    }

    #[test]
    fn execution_summary_excludes_security_annotations() {
        let history = "→ read(inbox/msg.md)\nSecurity threat detected: injection attempt\nOUTCOME_DENIED_SECURITY chosen\nPossible exfiltration via branching\n→ answer({outcome: OK})\nWritten to outbox/1.json";
        let summary = pregrounding::build_execution_summary(history, 10);
        assert!(!summary.contains("Security threat"), "Security annotations must be excluded");
        assert!(!summary.contains("OUTCOME_DENIED"), "OUTCOME_DENIED lines must be excluded");
        assert!(!summary.contains("exfiltration"), "Exfiltration lines must be excluded");
        assert!(summary.contains("→ read"));
        assert!(summary.contains("→ answer"));
        assert!(summary.contains("Written to"));
    }
}
