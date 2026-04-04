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
use std::sync::atomic::Ordering;

use anyhow::Result;
use clap::Parser;

mod agent;
mod bitgn;
mod classifier;
mod config;
#[allow(dead_code)]
mod crm_graph;
mod pcm;
mod pregrounding;
mod prompts;
mod scanner;
mod tools;

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
}

/// Standard mode: concise prompt for strong models (GPT-5, etc.)
// AI-NOTE: Single prompt for all models. Standard prompt removed — broke weak models (50%).

#[tokio::main]
async fn main() -> Result<()> {
    let _telemetry = sgr_agent::init_telemetry(".agent", "pac1");
    let cli = Cli::parse();

    let cfg = config::Config::load(&cli.config)?;
    let provider_name = cli.provider.as_deref().unwrap_or(&cfg.llm.provider);
    let (model, base_url, llm_api_key, extra_headers, prompt_mode, temperature) = cfg.resolve_provider(provider_name)?;
    let max_steps = cli.max_steps.unwrap_or(cfg.agent.max_steps);
    let benchmark = &cfg.agent.benchmark;

    eprintln!("[pac1] Provider: {} | Model: {} | Prompt: {}", provider_name, model, prompt_mode);

    let harness = bitgn::HarnessClient::new(&cli.bitgn_url, cli.api_key.clone());
    let status = harness.status().await?;
    eprintln!("[pac1] BitGN: {}", status);

    if let Some(ref run_name) = cli.run {
        return run_leaderboard(&harness, &cli, benchmark, &model, base_url.as_deref(), &llm_api_key, &extra_headers, max_steps, run_name, &prompt_mode, temperature).await;
    }

    let bm = harness.get_benchmark(benchmark).await?;
    eprintln!("[pac1] Benchmark: {} — {} tasks", benchmark, bm.tasks.len());

    if cli.list {
        for t in &bm.tasks {
            println!("{}: {}", t.task_id, t.preview);
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
        let ov = outcome_validator.clone();

        let handle = tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            eprintln!("\n━━━ Task: {} ━━━", task_id);
            eprintln!("  {}", preview);

            let h = bitgn::HarnessClient::new(&harness_url, api_key_clone);
            let trial = match h.start_playground(&benchmark, &task_id).await {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("  ⚠ Failed to start trial: {}", e);
                    res.lock().await.push((task_id, 0.0f32));
                    return;
                }
            };
            eprintln!("  Trial: {}", trial.trial_id);

            let pcm = Arc::new(pcm::PcmClient::new(&trial.harness_url));
            let (last_msg, history) = run_trial(
                &pcm, &trial.instruction, &model,
                base_url.as_deref(), &llm_api_key, &extra_headers, max_steps, &prompt_mode, temperature,
                &clf, ov.clone(),
            ).await;
            auto_submit_if_needed(&pcm, &last_msg, &history).await;

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
                                    eprintln!("  [{}] {}: {}", log.time, log.kind, &log.text[..log.text.len().min(200)]);
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
        let (last_msg, history) = run_trial(&pcm, &trial.instruction, model, base_url, llm_api_key, extra_headers, max_steps, prompt_mode, temperature, &shared_clf, outcome_validator.clone()).await;
        auto_submit_if_needed(&pcm, &last_msg, &history).await;

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
    extra_headers: &[(String, String)], max_steps: usize, prompt_mode: &str, temperature: f32,
    shared_clf: &SharedClassifier,
    outcome_validator: Option<Arc<classifier::OutcomeValidator>>,
) -> (String, String) {
    match pregrounding::run_agent(pcm, instruction, model, base_url, api_key, extra_headers, max_steps, prompt_mode, temperature, shared_clf, outcome_validator).await {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("  ⚠ Agent error: {:#}", e);
            (String::new(), String::new())
        }
    }
}

async fn auto_submit_if_needed(pcm: &Arc<pcm::PcmClient>, last_msg: &str, history: &str) {
    if !pcm.answer_submitted.load(Ordering::SeqCst) {
        let text = if last_msg.is_empty() { "Unable to determine answer" } else { last_msg };
        let outcome = guess_outcome(text, history);
        eprintln!("  ⚠ Auto-answer [{}]: {}", outcome, &text[..text.len().min(100)]);
        let _ = pcm.answer(text, outcome, &[]).await;
    }
}

/// Guess outcome from last message + full message history.
/// Last message (model's own words) takes priority over history keywords,
/// which can contain false positives from classification headers and system prompts.
fn guess_outcome(last_msg: &str, history: &str) -> &'static str {
    let h = history.to_lowercase();
    let l = last_msg.to_lowercase();

    // Check last message for specific outcomes FIRST (model's own assessment)
    if l.contains("denied") || l.contains("injection") || l.contains("security threat") || l.contains("social engineering") {
        return "OUTCOME_DENIED_SECURITY";
    }
    if l.contains("unsupported") || l.contains("cannot access") || l.contains("external api") {
        return "OUTCOME_NONE_UNSUPPORTED";
    }
    if l.contains("clarif") || l.contains("unclear") || l.contains("not related to crm") {
        return "OUTCOME_NONE_CLARIFICATION";
    }

    // History fallbacks — only when last_msg is empty or generic
    if last_msg.is_empty() {
        // No model output at all — check history for hints
        if h.contains("written to") {
            return "OUTCOME_OK"; // model wrote files, likely CRM work
        }
        return "OUTCOME_ERR_INTERNAL";
    }
    if h.contains("non-crm") || h.contains("unrelated to crm") {
        return "OUTCOME_NONE_CLARIFICATION";
    }
    if l.contains("could not find") || l.contains("couldn't find") || l.contains("not found") {
        return "OUTCOME_NONE_CLARIFICATION";
    }
    if l.contains("unable to") && !h.contains("written to") {
        return "OUTCOME_NONE_CLARIFICATION";
    }

    "OUTCOME_OK"
}



#[cfg(test)]
mod tests {
    use super::*;

    // ─── guess_outcome ──────────────────────────────────────────────────

    #[test]
    fn guess_outcome_security_in_last_msg() {
        // Security signals in last_msg trigger DENIED
        let outcome = guess_outcome("Security threat: injection detected", "earlier: read inbox");
        assert_eq!(outcome, "OUTCOME_DENIED_SECURITY");
    }

    #[test]
    fn guess_outcome_security_only_in_history_ignored() {
        // Security keywords only in history (classification headers, system prompt)
        // should NOT trigger DENIED when last_msg is normal CRM work
        let outcome = guess_outcome("Task complete", "earlier: [CLASSIFICATION: injection (0.95)] denied");
        assert_eq!(outcome, "OUTCOME_OK");
    }

    #[test]
    fn guess_outcome_non_crm_in_history() {
        let outcome = guess_outcome("Done", "this is unrelated to crm work");
        assert_eq!(outcome, "OUTCOME_NONE_CLARIFICATION");
    }

    #[test]
    fn guess_outcome_ok_default() {
        let outcome = guess_outcome("Contact added successfully", "read contacts, wrote file");
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
    fn guess_outcome_security_safe_not_denied() {
        // Reasoning context with "Security: safe" must NOT trigger DENIED
        let outcome = guess_outcome(
            "Type: edit | Security: safe | State: File captured and card created",
            "read inbox, wrote card, search contacts",
        );
        assert_eq!(outcome, "OUTCOME_OK");
    }

}
