use std::sync::Arc;

use anyhow::Result;
use sgr_agent::agent_loop::{LoopConfig, LoopEvent, run_loop};
// AI-NOTE: PlanTool, Plan, PlanningAgent removed — planning phase eliminated
use sgr_agent::context::AgentContext;
use sgr_agent::evolution::{self, EvolutionEntry, RunStats};
use sgr_agent::registry::ToolRegistry;
use sgr_agent::types::{LlmConfig, Message, Role};
use sgr_agent::Llm;
use sgr_agent::client::LlmClient;

use crate::agent;
use crate::classifier;
use crate::crm_graph;
use crate::pcm;
use crate::prompts;
use crate::scanner::{self, SharedClassifier, SharedNliClassifier};
use crate::tools;

// AI-NOTE: extract_mentioned_names + resolve_contact_hints moved to src/legacy.rs
// They were part of CRM-specific pre-grounding (contact disambiguation).
// Now agent resolves contacts via search — no pre-computed hints needed.

/// Quick LLM intent classification — 1 function call when ML confidence is low.
/// Returns None on error (structural fallback handles it).
async fn classify_intent_via_llm(
    instruction: &str,
    model: &str,
    base_url: Option<&str>,
    api_key: &str,
    extra_headers: &[(String, String)],
    temperature: f32,
) -> Option<String> {
    use sgr_agent::tool::ToolDef;

    let cfg = make_llm_config(model, base_url, api_key, extra_headers, temperature);
    let llm = Llm::new(&cfg);

    let td = ToolDef {
        name: "classify".to_string(),
        description: "Classify the task intent".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "intent": {
                    "type": "string",
                    "enum": ["intent_inbox", "intent_email", "intent_delete", "intent_query", "intent_edit", "intent_capture"],
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

/// Run a planning phase: read-only exploration → structured Plan.
pub(crate) fn make_llm_config(
    model: &str,
    base_url: Option<&str>,
    api_key: &str,
    extra_headers: &[(String, String)],
    temperature: f32,
) -> LlmConfig {
    if let Some(url) = base_url {
        let mut cfg = LlmConfig::endpoint(api_key, url, model).temperature(temperature as f64).max_tokens(4096);
        cfg.use_chat_api = true;
        cfg.extra_headers = extra_headers.to_vec();
        cfg.reasoning_effort = std::env::var("LLM_REASONING_EFFORT").ok();
        cfg.prompt_cache_key = std::env::var("LLM_PROMPT_CACHE_KEY").ok();
        cfg
    } else if !api_key.is_empty() {
        let mut cfg = LlmConfig::with_key(api_key, model).temperature(temperature as f64).max_tokens(4096);
        cfg.extra_headers = extra_headers.to_vec();
        // Native API providers (Anthropic, Gemini) need genai backend
        cfg.use_genai = model.starts_with("claude") || model.starts_with("gemini");
        cfg
    } else {
        let mut cfg = LlmConfig::auto(model).temperature(temperature as f64).max_tokens(4096);
        cfg.extra_headers = extra_headers.to_vec();
        cfg
    }
}

/// Dump trial debug data to disk.
fn dump_trial_data(
    dump_dir: &str, tree_out: &str, agents_md: &str, crm_schema: &str,
    ready: &crate::pipeline::Ready, model: &str, intent_confidence: f32,
) {
    let _ = std::fs::create_dir_all(dump_dir);
    let _ = std::fs::write(format!("{dump_dir}/tree.txt"), tree_out);
    if !agents_md.is_empty() { let _ = std::fs::write(format!("{dump_dir}/agents.md"), agents_md); }
    if !crm_schema.is_empty() { let _ = std::fs::write(format!("{dump_dir}/crm_schema.txt"), crm_schema); }
    let contacts = ready.crm_graph.contacts_summary();
    if !contacts.is_empty() { let _ = std::fs::write(format!("{dump_dir}/contacts.txt"), &contacts); }
    let accounts = ready.crm_graph.accounts_summary();
    if !accounts.is_empty() { let _ = std::fs::write(format!("{dump_dir}/accounts.txt"), &accounts); }
    for (i, f) in ready.inbox_files.iter().enumerate() {
        let sender = f.security.sender.as_ref().map(|s| format!("{}", s.trust)).unwrap_or_default();
        let _ = std::fs::write(
            format!("{dump_dir}/inbox_{i:02}_{}.txt", f.path.replace('/', "_")),
            format!("[{} ({:.2}) | sender: {sender} | {}]\n\n{}", f.security.ml_label, f.security.ml_conf, f.security.recommendation, f.content),
        );
    }
    let per_inbox: Vec<String> = ready.inbox_files.iter().enumerate().map(|(i, f)| {
        let sender = f.security.sender.as_ref().map(|s| format!("{}", s.trust)).unwrap_or_else(|| "?".into());
        format!("  [{i}] {} ({:.2}) sender={sender} {}", f.security.ml_label, f.security.ml_conf, f.path)
    }).collect();
    let _ = std::fs::write(format!("{dump_dir}/pipeline.txt"), format!(
        "model: {model}\ninstruction: {}\nintent: {} ({intent_confidence:.2})\nlabel: {}\ninbox_files: {}\ncrm_nodes: {}\n\nper_inbox:\n{}\n",
        ready.instruction, ready.intent, ready.instruction_label, ready.inbox_files.len(), ready.crm_graph.node_count(), per_inbox.join("\n"),
    ));
    eprintln!("  📁 Trial data dumped to {dump_dir}");
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_agent(
    pcm: &Arc<pcm::PcmClient>,
    instruction: &str,
    model: &str,
    base_url: Option<&str>,
    api_key: &str,
    extra_headers: &[(String, String)],
    max_steps: usize,
    prompt_mode: &str,
    temperature: f32,
    planning_temperature: f32,
    shared_clf: &SharedClassifier,
    shared_nli: &SharedNliClassifier,
    outcome_validator: Option<Arc<classifier::OutcomeValidator>>,
    sgr_mode: bool,
    dump_dir: Option<&str>,
) -> Result<(String, String, usize, usize)> {
    use crate::pipeline;

    // ── Pipeline Stage 1: Classify instruction ──────────────────────
    let trial = pipeline::New { instruction: instruction.to_string() };
    let classified = match trial.classify(shared_clf) {
        Ok(c) => c,
        Err(block) => {
            eprintln!("  ⛔ [STAGE:{}] {}", block.stage, block.message);
            pcm.answer(&block.message, block.outcome, &[]).await.ok();
            return Ok((block.message, String::new(), 0, 0));
        }
    };
    let instruction_label = classified.instruction_label.clone();
    let instruction_intent = classified.intent.clone();
    let intent_confidence = classified.intent_confidence;

    // ── Context assembly (tree, agents.md, CRM schema) — parallel IO ──
    let (tree_out, agents_md, ctx_time) = tokio::join!(
        async { pcm.tree("/", 2).await.unwrap_or_else(|e| format!("(error: {})", e)) },
        async { pcm.read("AGENTS.md", false, 0, 0).await.unwrap_or_default() },
        async { pcm.context().await.unwrap_or_default() },
    );

    let crm_schema = {
        let mut readmes = String::new();
        for line in tree_out.lines() {
            let trimmed = line.trim().trim_start_matches(|c: char| c == '│' || c == '├' || c == '└' || c == '─' || c == ' ' || c == '|');
            if trimmed.ends_with('/') {
                let dir = trimmed.trim_end_matches('/');
                if !dir.is_empty() {
                    // AI-NOTE: prod uses AGENTS.MD not README. Try AGENTS.MD first, then README variants.
                    let candidates = [
                        format!("{}/AGENTS.MD", dir),
                        format!("{}/AGENTS.md", dir),
                        format!("{}/README.md", dir),
                        format!("{}/README.MD", dir),
                    ];
                    for path in &candidates {
                        if let Ok(content) = pcm.read(path, false, 0, 0).await {
                            if !content.is_empty() {
                                readmes.push_str(&format!("# {}\n{}\n\n", path, content));
                                if readmes.len() > 2000 { break; }
                                break; // found one, skip other case variant
                            }
                        }
                    }
                }
            }
        }
        let trunc = readmes.floor_char_boundary(2000);
        readmes.truncate(trunc);
        readmes
    };

    eprintln!("  Grounding: tree={} bytes, agents.md={} bytes, crm_schema={} bytes",
        tree_out.len(), agents_md.len(), crm_schema.len());

    // ── Pipeline Stage 2: Build CRM graph + scan inbox ──────────────
    // Parallel: build CRM graph and collect account domains concurrently
    let (mut crm_graph, account_domains) = tokio::join!(
        crm_graph::CrmGraph::build_from_pcm(pcm),
        scanner::collect_account_domains(pcm),
    );
    eprintln!("  CRM graph: {} nodes", crm_graph.node_count());
    // Pre-compute account embeddings for semantic cross-account detection
    crm_graph.compute_account_embeddings(shared_clf);
    let scanned = match classified.scan_inbox(pcm, shared_clf, shared_nli, crm_graph, &account_domains).await {
        Ok(s) => s,
        Err(block) => {
            eprintln!("  ⛔ [STAGE:{}] {}", block.stage, block.message);
            pcm.answer(&block.message, block.outcome, &[]).await.ok();
            return Ok((block.message, String::new(), 0, 0));
        }
    };

    // ── Pipeline Stage 3: Security check ────────────────────────────
    let checked = match scanned.check_security() {
        Ok(c) => c,
        Err(block) => {
            eprintln!("  ⛔ [STAGE:{}] {}", block.stage, block.message);
            pcm.answer(&block.message, block.outcome, &[]).await.ok();
            return Ok((block.message, String::new(), 0, 0));
        }
    };

    // ── Pipeline Stage 4: Ready ─────────────────────────────────────
    let mut ready = checked.ready();

    // Low-confidence intent: ML unsure → ask LLM → structural fallback
    // Skip fallback for intent_delete — structural forcing already handles it via detect_forced_task_type
    if intent_confidence < 0.30 && ready.intent != "intent_delete" {
        // Layer 2: quick LLM classify (1 function call)
        let llm_intent = classify_intent_via_llm(
            instruction, model, base_url, api_key, extra_headers, temperature,
        ).await;

        if let Some(ref llm_label) = llm_intent {
            let normalized = if llm_label == "intent_capture" { "intent_inbox".to_string() } else { llm_label.clone() };
            eprintln!("  ↳ LLM intent classify: {} ({:.2}) → {}", ready.intent, intent_confidence, normalized);
            ready.intent = normalized;
        } else if !ready.inbox_files.is_empty() && ready.intent != "intent_inbox" {
            // Layer 3: structural fallback (inbox files exist → inbox task)
            eprintln!("  ↳ Structural intent fallback: {} ({:.2}) → intent_inbox (inbox_files={})",
                ready.intent, intent_confidence, ready.inbox_files.len());
            ready.intent = "intent_inbox".to_string();
        }
    }

    // Dump trial data for debugging
    let dump_dir_resolved = dump_dir.map(|s| s.to_string())
        .or_else(|| std::env::var("DUMP_TRIAL").ok());
    if let Some(ref d) = dump_dir_resolved {
        dump_trial_data(d, &tree_out, &agents_md, &crm_schema, &ready, model, intent_confidence);
    }

    // AI-NOTE: system prompt loaded from prompts/system.md at runtime (hot-reload, no rebuild).
    //   Fallback to compiled-in SYSTEM_PROMPT_V2 if file not found.
    //   Enables ShinkaEvolve optimization without cargo build cycle.
    let template = std::fs::read_to_string("prompts/system.md")
        .unwrap_or_else(|_| prompts::SYSTEM_PROMPT_V2.to_string());
    let template = template.as_str();
    // Skill-based prompt injection (replaces examples_for_class)
    let skill_registry = crate::skills::load(std::path::Path::new("."));
    let effective_label = if ready.intent == "intent_query" && instruction_label == "non_work" {
        eprintln!("  ↳ Skill override: non_work → crm (intent_query)");
        "crm"
    } else {
        &instruction_label
    };
    // Wrap in Arc early — needed for both prompt injection and agent tools
    let skill_registry = std::sync::Arc::new(skill_registry);
    let skill_body = crate::skills::select_body(&skill_registry, effective_label, &instruction_intent, instruction);
    let hint = std::env::var("HINT").unwrap_or_default();
    // System prompt is now STATIC (cacheable) — dynamic parts go into user messages
    let mut system_prompt = template.to_string();
    if !hint.is_empty() {
        system_prompt.push_str(&format!("\n\n{}", hint));
    }
    eprintln!("  Prompt: {} bytes (skill: {})", system_prompt.len(), effective_label);

    let config = make_llm_config(model, base_url, api_key, extra_headers, temperature);
    let llm = Llm::new(&config);

    // AI-NOTE: FC probe moved to --probe CLI flag (not per-trial). Use `make probe` to test new models.

    // AI-NOTE: agents_md + skill_body moved from system prompt to user messages — enables
    //   server-side prompt prefix caching (DeepInfra auto-cache, OpenAI 93% hit).
    //   System prompt is now STATIC across all trials. Dynamic content = user messages.
    let mut messages = Vec::new();
    if !agents_md.is_empty() {
        messages.push(Message::user(&format!("AGENTS.MD (workspace rules):\n{}", agents_md)));
    }
    if !skill_body.is_empty() {
        messages.push(Message::user(&format!("SKILL WORKFLOW (auto-selected — if this doesn't match your task, call list_skills() then get_skill(name) to switch):\n{}", skill_body)));
    }
    messages.push(Message::user(&tree_out));
    messages.push(Message::user(&format!("$ date\n{}", ctx_time)));

    // AI-NOTE: Codex-style minimal pre-grounding. Agent explores workspace itself via tools.
    // Removed: contacts_summary, accounts_summary, crm_schema, channel reads, channel trust,
    // feature matrix, contact resolution, channel stats, OTP hints (all handled by skills now).
    // Kept: inbox with ML classification headers (security advantage over raw injection).

    // AI-NOTE: Feature matrix disabled for A/B comparison experiment — testing without threat scores
    // // Feature matrix: batch-score inbox for threat probability (sigmoid gate)
    // Feature matrix: batch-score inbox for threat probability
    let empty_channel_trust = crate::policy::ChannelTrust::new();
    let inbox_scores = if !ready.inbox_files.is_empty() {
        let fm = crate::feature_matrix::InboxFeatureMatrix::from_inbox_files(
            &ready.inbox_files, &ready.crm_graph, shared_clf, &empty_channel_trust,
        );
        let scores = fm.score_all(&crate::feature_matrix::threat_weights());
        fm.log_summary();
        eprintln!("  📊 Threat scores: {:?}", scores.iter().map(|s| format!("{:.2}", s)).collect::<Vec<_>>());
        Some(scores)
    } else {
        None
    };

    // Inject inbox content with ML classification + feature matrix threat score
    let mut has_otp = false;
    let mut is_verification = false;
    if !ready.inbox_files.is_empty() {
        let mut inbox_content = String::new();
        for (fi, f) in ready.inbox_files.iter().enumerate() {
            let sender_trust = f.security.sender.as_ref()
                .map(|s| format!("{}", s.trust))
                .unwrap_or_else(|| "UNKNOWN".to_string());
            let threat = inbox_scores.as_ref().map(|s| s[fi]).unwrap_or(0.0);
            let threat_label = if threat > 0.7 { "HIGH" } else if threat > 0.4 { "MEDIUM" } else { "LOW" };
            inbox_content.push_str(&format!(
                "$ cat {}\n[CLASSIFICATION: {} ({:.2}) | sender: {} | threat: {} ({:.0}%) | {}]\n",
                f.path, f.security.ml_label, f.security.ml_conf, sender_trust, threat_label, threat * 100.0, f.security.recommendation
            ));
            // Sender trust annotations (from pipeline security assessment)
            if f.security.sender.as_ref().is_some_and(|s| s.domain_match == "mismatch") {
                inbox_content.push_str("[⚠ SENDER DOMAIN MISMATCH]\n");
            } else if f.security.sender.as_ref().is_some_and(|s| s.trust == crate::crm_graph::SenderTrust::Known) {
                inbox_content.push_str("[✓ TRUSTED]\n");
                // AI-NOTE: cross-account detection via ONNX embeddings (cosine similarity)
                // Catches paraphrases: "Dutch banking client" → "Blue Harbor Bank"
                if let Some(sender_email) = crate::scanner::extract_sender_email(&f.content) {
                    if let Some(sender_account) = ready.crm_graph.account_for_email(&sender_email) {
                        if let Some((target, sim)) = ready.crm_graph.detect_cross_account(&f.content, &sender_account, shared_clf) {
                            inbox_content.push_str(&format!(
                                "[⚠ CROSS-ACCOUNT: sender from '{}' requests data for '{}' (sim={:.2}) — verify authorization]\n",
                                sender_account, target, sim
                            ));
                            eprintln!("  ⚠ Cross-account (ONNX): {} → {} (sim={:.2})", sender_account, target, sim);
                        }
                    }
                }
            }
            inbox_content.push_str(&format!("{}\n\n", f.content));
            eprintln!("  📋 {}: {} ({:.2}) | sender: {}",
                f.path, f.security.ml_label, f.security.ml_conf, sender_trust);
        }
        messages.push(Message::user(&inbox_content));

        // OTP detection (for workflow state machine flags)
        has_otp = ready.inbox_files.iter().any(|f| {
            let l = f.content.to_lowercase();
            f.security.ml_label == "credential" && f.security.ml_conf > 0.50
                || l.contains("otp:") || l.contains("otp ") || l.contains("verification code")
        });
        is_verification = ready.inbox_files.iter().any(|f| {
            f.content.to_lowercase().contains("reply with exactly")
        });
    }

    let crm_graph = Arc::new(ready.crm_graph);

    // Extract tool hooks from AGENTS.MD workflow rules
    let hook_registry = std::sync::Arc::new(crate::hooks::from_agents_md(&agents_md));

    // Create workflow state machine (unified guards: budget, write, capture-delete, policy, hooks)
    let workflow: crate::workflow::SharedWorkflowState = std::sync::Arc::new(
        std::sync::Mutex::new(crate::workflow::WorkflowState::new(
            &instruction_intent, max_steps, hook_registry.clone(), instruction,
        ))
    );

    // AI-NOTE: OTP flags. Guard: workflow.rs:198. Schema: tools.rs:734. Hint: above at line ~602.
    // Set verification-only mode if detected (blocks ALL file changes structurally)
    if is_verification {
        workflow.lock().unwrap().verification_only = true;
    }
    if has_otp && !is_verification {
        workflow.lock().unwrap().otp_with_task = true;
    }

    // AI-NOTE: removed seq.json pre-check — agent reads outbox README itself (via skill).
    // Old approach: hardcoded outbox path guessing. New: agent-driven, workspace-agnostic.

    // ── Tool Registry (Claude Code / Codex inspired: core + extended + management) ──
    let registry = ToolRegistry::new()
        // CORE (Claude Code equivalents + PAC1 answer)
        .register(tools::ReadTool::new(pcm.clone(), Some(workflow.clone())))
        .register(tools::WriteTool::new(pcm.clone(), hook_registry.clone(), Some(workflow.clone())))
        .register(tools::DeleteTool::new(pcm.clone(), Some(workflow.clone())))
        .register(tools::SearchTool(pcm.clone(), Some(crm_graph.clone())))
        .register(sgr_agent_tools::ListTool(pcm.clone()))                        // → sgr-agent-tools
        .register(sgr_agent_tools::TreeTool(pcm.clone()))                        // → sgr-agent-tools
        .register(sgr_agent_tools::EvalTool(pcm.clone()))                         // → sgr-agent-tools
        .register(tools::AnswerTool::new(pcm.clone(), outcome_validator.clone(), Some(workflow.clone()))) // PAC1-specific
        .register(tools::ContextTool(pcm.clone()))                               // PAC1-specific
        // EXTENDED
        .register(sgr_agent_tools::ReadAllTool(pcm.clone()))                     // → sgr-agent-tools
        // DEFERRED
        .register_deferred(sgr_agent_tools::MkDirTool(pcm.clone()))              // → sgr-agent-tools
        .register_deferred(sgr_agent_tools::MoveTool(pcm.clone()))               // → sgr-agent-tools
        .register_deferred(sgr_agent_tools::FindTool(pcm.clone()))
        .register_deferred(sgr_agent_tools::ApplyPatchTool(pcm.clone()))         // Codex diff DSL
        .register_deferred(tools::ListSkillsTool(skill_registry.clone()))
        .register_deferred(tools::GetSkillTool(skill_registry.clone()));

    let agent = agent::Pac1Agent::with_config(llm, &system_prompt, max_steps as u32, prompt_mode, Some(workflow.clone()));
    agent.set_intent(&instruction_intent);
    let mut ctx = AgentContext::new();

    // AI-NOTE: planning phase removed — agent plans in Phase 1 reasoning (plan field in CoT).
    // Old approach: separate PlanningAgent loop (up to 5 steps, read-only tools) → generated plan →
    // injected as message → agent re-read the same files during execution = double work.
    // New approach: tree + AGENTS.MD + skill + inbox already in context. Agent's Phase 1
    // reasoning has a `plan` field for structuring its approach. No extra LLM call needed.

    messages.push(Message::user(instruction));

    // AI-NOTE: Codex-style — all hints removed. Skills provide workflow guidance.
    // Removed: capture hints, intent hints, inbox processing hints, external URL hints,
    // capture pre-execute. Agent navigates via tree + AGENTS.MD + skill.

    // Scale max_steps for multi-inbox: 5+ messages need more room
    // AI-NOTE: skip scaling for intent_delete — t01 delete tasks ignore inbox, don't need extra steps
    let effective_max_steps = if ready.inbox_files.len() > 3 && ready.intent != "intent_delete" {
        let scaled = max_steps + (ready.inbox_files.len() * 4);
        eprintln!("  📬 Multi-inbox ({} files): max_steps {}→{}", ready.inbox_files.len(), max_steps, scaled);
        scaled
    } else {
        max_steps
    };

    // Update workflow with scaled max_steps
    if effective_max_steps != max_steps {
        workflow.lock().unwrap().set_max_steps(effective_max_steps);
    }

    // ── SGR Mode: pure single-call loop (4x faster on weak models) ────
    if sgr_mode {
        let sgr_llm = sgr_agent::llm::Llm::new(&config);
        let sgr_agent = crate::pac1_sgr::Pac1SgrAgent::new(
            pcm.clone(), sgr_llm, system_prompt.clone(), instruction_intent.clone(),
        ).with_hooks(hook_registry.clone());
        let sgr_config = sgr_agent::app_loop::LoopConfig {
            max_steps: effective_max_steps,
            loop_abort_threshold: 6,
        };
        // Convert messages to SGR Msg format
        let mut session = sgr_agent::session::Session::<crate::pac1_sgr::Msg>::new("/tmp/pac1-sgr", 80)
            .map_err(|e| anyhow::anyhow!("SGR session: {}", e))?;
        // Inject system prompt + pre-grounding messages
        session.push(crate::pac1_sgr::Role::System, system_prompt.clone());
        for m in &messages {
            session.push(crate::pac1_sgr::Role::User, m.content.clone());
        }

        let steps = sgr_agent::app_loop::run_loop(
            &sgr_agent, &mut session, &sgr_config,
            |event| {
                use sgr_agent::app_loop::LoopEvent;
                match event {
                    LoopEvent::StepStart(n) => eprintln!("  [SGR step {}/{}]", n, effective_max_steps),
                    LoopEvent::Completed => eprintln!("  ✓ SGR done"),
                    LoopEvent::MaxStepsReached(n) => eprintln!("  ⚠ SGR max steps: {}", n),
                    LoopEvent::LoopAbort(n) => eprintln!("  ⚠ SGR loop abort: {}", n),
                    _ => {}
                }
            },
        ).await.map_err(|e| anyhow::anyhow!("SGR loop: {}", e))?;

        eprintln!("  📊 SGR completed in {} steps", steps);

        // Extract last assistant message for verify_and_submit
        use sgr_agent::session::AgentMessage as _;
        let msgs = session.messages();
        let last_msg = msgs.iter().rev()
            .find(|m| *m.role() == crate::pac1_sgr::Role::Assistant)
            .map(|m| m.content().to_string())
            .unwrap_or_default();
        let history: String = msgs.iter()
            .map(|m| m.content().to_string())
            .collect::<Vec<_>>()
            .join("\n");

        return Ok((last_msg, history, 0, 0)); // SGR mode — no step tracking
    }

    let loop_config = LoopConfig {
        max_steps: effective_max_steps,
        loop_abort_threshold: 25,
        max_messages: 80,
        auto_complete_threshold: 5,
    };

    // Collect RunStats + step trace for evolution tracking
    let mut run_stats = RunStats::default();
    let step_trace: std::sync::Arc<std::sync::Mutex<Vec<(u32, String, String)>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let trace_ref = step_trace.clone();
    let mut current_step: u32 = 0;

    let result = run_loop(
        &agent, &registry, &mut ctx, &mut messages, &loop_config,
        |event| match event {
            LoopEvent::StepStart { step } => {
                run_stats.steps = step;
                current_step = step as u32;
            }
            LoopEvent::Decision(ref d) => {
                for tc in &d.tool_calls {
                    let key_arg = tc.arguments.as_object()
                        .and_then(|o| o.get("path").or(o.get("pattern")).or(o.get("root")))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    eprintln!("    → {}({})", tc.name, key_arg);
                }
            }
            LoopEvent::ToolResult { name, output } => {
                // Compact result summary
                let summary = if output.starts_with("$ cat ") || output.starts_with("$ ls ") {
                    let lines = output.lines().count();
                    format!("{} lines", lines)
                } else if output.starts_with("Written to ") {
                    output[..output.len().min(60)].to_string()
                } else if output.starts_with("Deleted ") {
                    output.to_string()
                } else if output.starts_with("Answer submitted") {
                    "✓ submitted".to_string()
                } else {
                    let p = &output[..output.len().min(50)];
                    p.replace('\n', " ")
                };
                eprintln!("    {} = {}", name, summary);

                // Record for trace table
                if let Ok(mut trace) = trace_ref.lock() {
                    trace.push((current_step, name.to_string(), summary.clone()));
                }

                run_stats.successful_calls += 1;
                run_stats.cost_chars += output.len();
            }
            LoopEvent::Completed { steps } => {
                run_stats.completed = true;
                run_stats.steps = steps;
            }
            LoopEvent::LoopDetected { count } => {
                run_stats.loop_warnings += 1;
                if count >= loop_config.loop_abort_threshold {
                    run_stats.loop_aborts += 1;
                }
                eprintln!("  ⚠ Loop detected ({}x)", count);
            }
            LoopEvent::Error(e) => {
                run_stats.tool_errors += 1;
                eprintln!("  ⚠ Error: {}", e);
            }
            _ => {}
        },
    )
    .await;

    // Print step trace table
    if let Ok(trace) = step_trace.lock() {
        if !trace.is_empty() {
            eprintln!("  ┌─────┬────────────┬─────────────────────────────────────────────────┐");
            eprintln!("  │ Step│ Tool       │ Result                                          │");
            eprintln!("  ├─────┼────────────┼─────────────────────────────────────────────────┤");
            for (step, tool, result) in trace.iter() {
                let t = if tool.len() > 10 { &tool[..10] } else { tool };
                let r = if result.len() > 47 { &result[..result.floor_char_boundary(47)] } else { result };
                eprintln!("  │{:4} │ {:10} │ {:47} │", step, t, r);
            }
            eprintln!("  └─────┴────────────┴─────────────────────────────────────────────────┘");
        }
    }
    let status = if run_stats.completed { "✓ Done" } else { "⚠ Max steps" };
    eprintln!("  {} in {} steps | {} tool calls | {} errors",
        status, run_stats.steps, run_stats.successful_calls, run_stats.tool_errors);

    // Evolution: score + evaluate + log
    let eff_score = evolution::score(&run_stats);
    let improvements = evolution::evaluate(&run_stats);
    eprintln!("  📊 Efficiency: {:.2}", eff_score);
    for imp in &improvements {
        eprintln!("  💡 [P{}] {}: {}", imp.priority, imp.title, imp.reason);
    }

    let tool_call_count = run_stats.successful_calls;
    let step_count = run_stats.steps;

    // Log to evolution.jsonl
    let _ = evolution::log_evolution(".agent", &EvolutionEntry {
        ts: {
            use std::time::SystemTime;
            let secs = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs();
            format!("{}", secs)
        },
        commit: String::new(), // filled by /evolve skill if running
        title: instruction[..instruction.len().min(80)].to_string(),
        score_before: evolution::baseline_score(".agent"),
        score_after: eff_score,
        status: if run_stats.completed { "complete" } else { "incomplete" }.into(),
        stats: run_stats,
    });

    // Extract history BEFORE error check — auto_submit_if_needed needs it even on error
    let last_assistant = messages
        .iter().rev()
        .find(|m| m.role == Role::Assistant && !m.content.is_empty())
        .map(|m| m.content.clone())
        .unwrap_or_default();

    let history: String = messages
        .iter()
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    if let Err(e) = result {
        eprintln!("  ⚠ Agent error: agent loop: {:#}", e);
        return Ok((last_assistant, history, tool_call_count, step_count));
    }

    Ok((last_assistant, history, tool_call_count, step_count))
}

/// Build a compact execution summary for the verifier from message history.
/// Extracts the last N tool-related lines (tool calls + results) — enough
/// context for 4-way classification without overwhelming the verifier.
/// EXCLUDES pre-grounding annotations ([CLASSIFICATION], [SENDER]) to avoid
/// biasing the verifier toward DENIED_SECURITY on legitimate tasks.
pub(crate) fn build_execution_summary(history: &str, max_lines: usize) -> String {
    let relevant: Vec<&str> = history.lines()
        .filter(|l| {
            let t = l.trim();
            // Exclude pre-grounding annotations and agent security reasoning —
            // they bias the verifier toward false DENIED_SECURITY
            if t.contains("[CLASSIFICATION") || t.contains("[SENDER")
                || t.contains("Security threat") || t.contains("OUTCOME_DENIED")
                || t.contains("injection") || t.contains("exfiltration")
            {
                return false;
            }
            t.starts_with("→ ") || t.starts_with("answer(") || t.contains("= Answer submitted")
                || t.contains("Written to") || t.contains("Deleted")
                || t.contains("OUTCOME_")
        })
        .collect();
    let start = relevant.len().saturating_sub(max_lines);
    relevant[start..].join("\n")
}

/// Extract channel handle from inbox message content.
/// Looks for "Handle: X" or "Handle: @X" patterns.
pub(crate) fn extract_channel_handle(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(_rest) = trimmed.strip_prefix("Channel:") {
            // Next line or same line might have Handle:
            continue;
        }
        if let Some(handle) = trimmed.strip_prefix("Handle:") {
            return Some(handle.trim().to_string());
        }
        // Combined: "Channel: Discord, Handle: MeridianOps"
        if let Some(pos) = trimmed.find("Handle:") {
            return Some(trimmed[pos + 7..].trim().to_string());
        }
    }
    None
}

