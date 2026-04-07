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
mod pipeline;
mod policy;
mod pregrounding;
mod prompts;
mod scanner;
#[allow(dead_code)]
mod pac1_sgr;
mod tools;
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
}

/// Standard mode: concise prompt for strong models (GPT-5, etc.)
// AI-NOTE: Single prompt for all models. Standard prompt removed — broke weak models (50%).

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file (API keys) — silent if missing
    dotenvy::dotenv().ok();

    let _telemetry = sgr_agent::init_telemetry(".agent", "pac1");
    let cli = Cli::parse();

    if cli.audit_store {
        return audit_outcome_store();
    }

    let cfg = config::Config::load(&cli.config)?;
    let provider_name = cli.provider.as_deref().unwrap_or(&cfg.llm.provider);
    let (model, base_url, llm_api_key, extra_headers, prompt_mode, temperature, planning_temperature, sgr_mode) = cfg.resolve_provider(provider_name)?;
    if sgr_mode {
        eprintln!("[pac1] SGR mode: pure (single LLM call per step)");
    }
    let max_steps = cli.max_steps.unwrap_or(cfg.agent.max_steps);
    let benchmark = &cfg.agent.benchmark;

    eprintln!("[pac1] Provider: {} | Model: {} | Prompt: {}", provider_name, model, prompt_mode);

    let harness = bitgn::HarnessClient::new(&cli.bitgn_url, cli.api_key.clone());
    let status = harness.status().await?;
    eprintln!("[pac1] BitGN: {}", status);

    if let Some(ref run_name) = cli.run {
        return run_leaderboard(&harness, &cli, benchmark, &model, base_url.as_deref(), &llm_api_key, &extra_headers, max_steps, run_name, &prompt_mode, temperature, planning_temperature).await;
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

        let handle = tokio::spawn(async move {
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
            eprintln!("  📋 Log: {}", log_url);

            // Auto-create dump dir if DUMP_TRIAL set, or auto-generate for single-task runs
            let dump_dir = std::env::var("DUMP_TRIAL").ok().unwrap_or_else(|| {
                format!("benchmarks/tasks/{}/{}", task_id, trial.trial_id)
            });
            let _ = std::fs::create_dir_all(&dump_dir);
            let _ = std::fs::write(format!("{}/bitgn_log.url", dump_dir), format!("{}\n", log_url));
            // SAFETY: single-threaded env var set for child code in same task
            unsafe { std::env::set_var("DUMP_TRIAL", &dump_dir); }

            let pcm = Arc::new(pcm::PcmClient::new(&trial.harness_url));
            let (last_msg, history) = run_trial(
                &pcm, &trial.instruction, &model,
                base_url.as_deref(), &llm_api_key, &extra_headers, max_steps, &prompt_mode, temperature, planning_temperature,
                &clf, &nli, ov.clone(), sgr_mode,
            ).await;
            verify_and_submit(
                &pcm, &trial.instruction, &last_msg, &history,
                &model, base_url.as_deref(), &llm_api_key, &extra_headers, temperature,
            ).await;

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
                                    eprintln!("  [{}] {}: {}", log.time, log.kind, &log.text[..log.text.len().min(500)]);
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
            res.lock().await.push((task_id, score));
        });
        handles.push(handle);
    }

    futures::future::join_all(handles).await;

    let results = results.lock().await;
    let total_score: f32 = results.iter().map(|(_, s)| s).sum();
    let scored = results.iter().filter(|(_, s)| *s > 0.0).count();
    eprintln!("\n═══ Average: {:.1}% ({}/{} tasks) ═══",
        total_score / results.len() as f32 * 100.0, scored, results.len());
    Ok(())
}

// ─── Leaderboard ─────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
async fn run_leaderboard(
    harness: &bitgn::HarnessClient, cli: &Cli, benchmark: &str,
    model: &str, base_url: Option<&str>, llm_api_key: &str,
    extra_headers: &[(String, String)], max_steps: usize, run_name: &str,
    prompt_mode: &str,
    temperature: f32,
    planning_temperature: f32,
) -> Result<()> {
    if cli.api_key.is_none() {
        anyhow::bail!("--api-key or BITGN_API_KEY required for leaderboard mode");
    }

    eprintln!("[pac1] Starting leaderboard run: {}", run_name);
    let run = harness.start_run(benchmark, run_name).await?;
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

    for (i, trial_id) in run.trial_ids.iter().enumerate() {
        let trial = harness.start_trial(trial_id).await?;
        eprintln!("\n━━━ Trial {}/{}: {} (task {}) ━━━",
            i + 1, run.trial_ids.len(), trial.trial_id, trial.task_id);

        let pcm = Arc::new(pcm::PcmClient::new(&trial.harness_url));
        let (last_msg, history) = run_trial(&pcm, &trial.instruction, model, base_url, llm_api_key, extra_headers, max_steps, prompt_mode, temperature, planning_temperature, &shared_clf, &shared_nli, outcome_validator.clone(), false).await;
        verify_and_submit(
            &pcm, &trial.instruction, &last_msg, &history,
            model, base_url, llm_api_key, extra_headers, temperature,
        ).await;

        let result = harness.end_trial(&trial.trial_id).await?;
        if let Some(score) = result.score {
            eprintln!("  Score: {:.2}", score);
            // Score-gated learning: only learn from confirmed correct answers
            if score >= 1.0 {
                if let Some(ref v) = outcome_validator {
                    v.learn_last();
                }
            }
        }
        for detail in &result.score_detail {
            eprintln!("    {}", detail);
        }
    }

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

// ─── Shared ──────────────────────────────────────────────────────────────────

use scanner::SharedClassifier;

/// Returns (last_assistant_msg, full_history_text).
async fn run_trial(
    pcm: &Arc<pcm::PcmClient>, instruction: &str,
    model: &str, base_url: Option<&str>, api_key: &str,
    extra_headers: &[(String, String)], max_steps: usize, prompt_mode: &str, temperature: f32, planning_temperature: f32,
    shared_clf: &SharedClassifier,
    shared_nli: &scanner::SharedNliClassifier,
    outcome_validator: Option<Arc<classifier::OutcomeValidator>>,
    sgr_mode: bool,
) -> (String, String) {
    match pregrounding::run_agent(pcm, instruction, model, base_url, api_key, extra_headers, max_steps, prompt_mode, temperature, planning_temperature, shared_clf, shared_nli, outcome_validator, sgr_mode).await {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("  ⚠ Agent error: {:#}", e);
            (String::new(), String::new())
        }
    }
}

/// Post-execution verification and submission.
/// 1. If agent proposed an answer → verify with LLM → apply override policy → submit
/// 2. If no proposed answer → use verifier as primary, guess_outcome as fallback → submit
#[allow(clippy::too_many_arguments)]
async fn verify_and_submit(
    pcm: &Arc<pcm::PcmClient>,
    instruction: &str,
    last_msg: &str,
    history: &str,
    model: &str,
    base_url: Option<&str>,
    api_key: &str,
    extra_headers: &[(String, String)],
    temperature: f32,
) {
    let execution_summary = pregrounding::build_execution_summary(history, 15);
    let proposed = pcm.get_proposed_answer();

    match proposed {
        Some(ref p) => {
            // Agent proposed an answer — verify it
            let verified = pregrounding::run_outcome_verifier(
                model, base_url, api_key, extra_headers, temperature,
                instruction, &execution_summary, &p.outcome, &p.message,
            ).await;

            match verified {
                Some(ref v) => {
                    match apply_override_policy(&p.outcome, &v.outcome, v.confidence) {
                        Some(ref override_outcome) => {
                            eprintln!("  🔍 Verifier: OVERRIDE {} → {} (conf={:.2}) — {}",
                                p.outcome, override_outcome, v.confidence, v.reason);
                            let _ = pcm.submit_proposed(Some(override_outcome)).await;
                        }
                        None if v.outcome == p.outcome => {
                            eprintln!("  🔍 Verifier: agree (conf={:.2}) — {}", v.confidence, v.reason);
                            let _ = pcm.submit_proposed(None).await;
                        }
                        None => {
                            let reason = if p.outcome == "OUTCOME_DENIED_SECURITY" {
                                "security never overridden"
                            } else {
                                "low confidence"
                            };
                            eprintln!("  🔍 Verifier: disagree but keep proposed ({}) — {} (conf={:.2})",
                                reason, v.reason, v.confidence);
                            let _ = pcm.submit_proposed(None).await;
                        }
                    }
                }
                None => {
                    eprintln!("  🔍 Verifier: fallback to proposed (LLM error)");
                    let _ = pcm.submit_proposed(None).await;
                }
            }
        }
        None => {
            // No proposed answer — agent didn't call answer(). Use guess_outcome heuristic.
            // Verifier is not used here: its value is in correcting a wrong outcome code,
            // not in guessing from scratch (CRM content confuses it — e.g. articles about "injection").
            let text = if last_msg.is_empty() { "Unable to determine answer" } else { last_msg };
            let outcome = guess_outcome(text, history);
            eprintln!("  ⚠ Auto-answer [{}]: {}", outcome, &text[..text.len().min(100)]);
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
) -> Option<String> {
    // Never override when agent chose DENIED_SECURITY — trust agent security decisions
    if proposed_outcome == "OUTCOME_DENIED_SECURITY" {
        return None;
    }

    // Selective security override (v0.4): when verifier detects injection the agent missed.
    // Agent said OK but verifier says DENIED_SECURITY with high confidence → override.
    // This catches prompt injection the agent failed to recognize (e.g. t09 "clipboard relay").
    if verifier_outcome == "OUTCOME_DENIED_SECURITY" && verifier_confidence >= 0.95 {
        return Some("OUTCOME_DENIED_SECURITY".to_string());
    }

    // All other disagreements: warn-only (6:1 wrong:correct ratio for non-security overrides)
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

    "OUTCOME_OK"
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
        let result = apply_override_policy("OUTCOME_OK", "OUTCOME_OK", 0.95);
        assert!(result.is_none(), "Same outcome = no override");
    }

    #[test]
    fn override_warn_only_for_non_security() {
        // Non-security disagreements: still warn-only (6:1 wrong:correct ratio)
        let result = apply_override_policy(
            "OUTCOME_OK", "OUTCOME_NONE_UNSUPPORTED", 0.9,
        );
        assert!(result.is_none(), "Non-security disagree = no override");

        let result = apply_override_policy(
            "OUTCOME_OK", "OUTCOME_NONE_UNSUPPORTED", 0.6,
        );
        assert!(result.is_none(), "Low confidence disagree = no override");
    }

    #[test]
    fn override_never_overrides_agent_denied() {
        let result = apply_override_policy(
            "OUTCOME_DENIED_SECURITY", "OUTCOME_OK", 0.99,
        );
        assert!(result.is_none(), "Agent's DENIED_SECURITY is never overridden");
    }

    #[test]
    fn override_security_high_confidence() {
        // Verifier detects injection agent missed → override
        let result = apply_override_policy(
            "OUTCOME_OK", "OUTCOME_DENIED_SECURITY", 0.99,
        );
        assert_eq!(result.as_deref(), Some("OUTCOME_DENIED_SECURITY"),
            "High-confidence security detection overrides agent OK");
    }

    #[test]
    fn override_security_low_confidence_no_override() {
        // Low confidence security detection → warn only
        let result = apply_override_policy(
            "OUTCOME_OK", "OUTCOME_DENIED_SECURITY", 0.80,
        );
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
