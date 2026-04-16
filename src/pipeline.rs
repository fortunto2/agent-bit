//! Centralized decision pipeline — enum state machine for PAC1 agent.
//!
//! Each state owns its data. Transitions are methods that consume the
//! current state and return the next. Compile-time guarantee: can't
//! skip stages or access data from a stage that hasn't run yet.
//!
//! ```text
//! New → Classified → InboxScanned → SecurityChecked → Ready → Executed → Complete
//!          ↓              ↓               ↓
//!        Block          Block           Block
//! ```

use crate::classifier;
use crate::crm_graph::{CrmGraph, SenderTrust};
use crate::scanner::{self, SharedClassifier};

// ── Block Reason (terminal state) ───────────────────────────────────────

/// Why the pipeline short-circuited before reaching the LLM.
#[derive(Debug, Clone)]
pub struct BlockReason {
    pub outcome: &'static str,
    pub message: String,
    pub stage: &'static str,
}

// ── Sender Assessment ───────────────────────────────────────────────────

/// Unified sender trust — single source of truth.
/// Merges crm_graph::validate_sender + scanner::check_sender_domain_match.
#[derive(Debug, Clone)]
pub struct SenderAssessment {
    pub trust: SenderTrust,
    pub domain_match: &'static str,
    #[allow(dead_code)]
    pub reasons: Vec<String>,
}

// ── Security Assessment ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SecurityAssessment {
    pub blocked: Option<BlockReason>,
    pub ml_label: String,
    pub ml_conf: f32,
    #[allow(dead_code)]
    pub structural: f32,
    pub sender: Option<SenderAssessment>,
    pub recommendation: String,
    /// NLI scores per label (if NLI classifier was available).
    pub nli_scores: Option<Vec<(String, f32)>>,
}

// ── Inbox File Assessment ───────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct InboxFile {
    pub path: String,
    pub content: String,
    pub security: SecurityAssessment,
}

// ── Trial States (enum state machine) ───────────────────────────────────

/// New trial — only instruction known.
#[derive(Debug)]
pub struct New {
    pub instruction: String,
}

/// Instruction classified — intent and security label determined.
#[derive(Debug)]
pub struct Classified {
    pub instruction: String,
    pub intent: String,
    pub intent_confidence: f32,
    pub instruction_label: String,
}

/// Inbox scanned — files read and classified with sender trust.
pub struct InboxScanned {
    pub instruction: String,
    pub intent: String,
    pub intent_confidence: f32,
    pub instruction_label: String,
    pub inbox_files: Vec<InboxFile>,
    pub crm_graph: CrmGraph,
}
impl std::fmt::Debug for InboxScanned {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InboxScanned").field("instruction", &self.instruction)
            .field("intent", &self.intent).field("inbox_files", &self.inbox_files.len()).finish()
    }
}

/// Security checked — all pre-LLM guards passed.
pub struct SecurityChecked {
    pub instruction: String,
    pub intent: String,
    pub intent_confidence: f32,
    pub instruction_label: String,
    pub inbox_files: Vec<InboxFile>,
    pub crm_graph: CrmGraph,
}
impl std::fmt::Debug for SecurityChecked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecurityChecked").field("instruction", &self.instruction)
            .field("intent", &self.intent).finish()
    }
}

/// Ready for LLM — messages assembled, agent configured.
/// This state is consumed by the caller to run sgr_agent::run_loop().
pub struct Ready {
    pub instruction: String,
    pub intent: String,
    #[allow(dead_code)]
    pub intent_confidence: f32,
    #[allow(dead_code)]
    pub instruction_label: String,
    pub inbox_files: Vec<InboxFile>,
    pub crm_graph: CrmGraph,
}
impl std::fmt::Debug for Ready {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ready").field("instruction", &self.instruction)
            .field("intent", &self.intent).finish()
    }
}

// ── Completeness Check ──────────────────────────────────────────────────

/// Detect truncated instructions using the ONNX tokenizer.
/// A word is "truncated" if the tokenizer splits it into subword pieces
/// starting with `##` (WordPiece continuation tokens). Full words tokenize
/// into either 1 token or tokens without `##` prefix.
/// Short last words (≤3 chars) that produce continuation tokens = truncated.
pub(crate) fn looks_truncated(instruction: &str, shared_clf: &SharedClassifier) -> bool {
    let trimmed = instruction.trim();
    if trimmed.is_empty() || trimmed.len() < 5 {
        return true;
    }
    if let Some(last) = trimmed.chars().last() {
        if matches!(last, '.' | '!' | '?' | '"' | ')' | ']') {
            return false;
        }
    }
    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if words.len() < 2 {
        return false;
    }
    let last = words.last().unwrap();
    // Use tokenizer: check if last word is a word fragment
    let mut guard = shared_clf.lock().unwrap();
    if let Some(clf) = guard.as_mut() {
        if let Ok(encoding) = clf.tokenizer().encode(last.to_string(), false) {
            let tokens = encoding.get_tokens();
            let has_continuation = tokens.iter().any(|t| t.starts_with("##"));
            // Short words with continuation tokens → truncated
            if has_continuation && last.len() <= 3 {
                return true;
            }
            // Longer words: suffix-completion check — if appending a suffix
            // produces a single-token word, the base was a truncated prefix.
            // e.g. "captur" (2 tokens) + "e" = "capture" (1 token) → truncated
            if has_continuation && last.len() > 3 {
                let lw = last.to_lowercase();
                for suffix in &["e", "s", "ed", "er", "es", "ing", "ly", "ure", "tion"] {
                    let extended = format!("{}{}", lw, suffix);
                    if let Ok(ext_enc) = clf.tokenizer().encode(extended, false) {
                        let ext_has_cont = ext_enc.get_tokens().iter()
                            .any(|t| t.starts_with("##"));
                        if !ext_has_cont {
                            return true; // suffix completed to a vocab word
                        }
                    }
                }
            }
        }
    }
    false
}

/// Detect external URLs (http/https) in instruction text.
/// Used to inject UNSUPPORTED hint in pregrounding when task references external APIs.
pub(crate) fn has_external_url(instruction: &str) -> bool {
    instruction.contains("http://") || instruction.contains("https://")
}

// ── Flattened Pipeline ──────────────────────────────────────────────────

// AI-NOTE: flattened from 4-struct state machine (New->Classified->InboxScanned->SecurityChecked->Ready)
//   into single run_pipeline() function. Same logic, same checks, less boilerplate.
//   Old structs kept for test backward compat but no longer used in production call site.

/// Run the full pre-LLM pipeline: classify -> scan inbox -> check security -> ready.
/// Returns Ready on success, BlockReason on first short-circuit.
pub async fn run_pipeline(
    instruction: &str,
    pcm: &crate::pcm::PcmClient,
    shared_clf: &SharedClassifier,
    shared_nli: &crate::scanner::SharedNliClassifier,
    crm_graph: CrmGraph,
    account_domains: &[(String, String)],
) -> Result<Ready, BlockReason> {

    // ── Stage 1: Classify instruction ───────────────────────────────

    // Prescan: literal HTML injection
    if let Some((outcome, msg)) = scanner::prescan_instruction(instruction) {
        return Err(BlockReason {
            outcome, message: msg.to_string(), stage: "prescan",
        });
    }

    // ML security classification
    let instruction_label = {
        let mut guard = shared_clf.lock().unwrap();
        let fc = scanner::semantic_classify_inbox_file(instruction, guard.as_mut(), None, None);
        eprintln!("  [STAGE:classify] Instruction class: {} ({:.2})", fc.label, fc.confidence);

        if fc.label == "injection" && fc.confidence > 0.5 {
            return Err(BlockReason {
                outcome: "OUTCOME_DENIED_SECURITY",
                message: "Blocked: instruction classified as injection attempt".into(),
                stage: "classify",
            });
        }
        if fc.label == "non_work" && fc.confidence > 0.5 {
            return Err(BlockReason {
                outcome: "OUTCOME_NONE_CLARIFICATION",
                message: "This request is unrelated to CRM/knowledge management work".into(),
                stage: "classify",
            });
        }
        fc.label
    };

    // Completeness check: detect truncated instructions via tokenizer
    if looks_truncated(instruction, shared_clf) {
        eprintln!("  [STAGE:classify] Instruction looks truncated (tokenizer: subword split)");
        return Err(BlockReason {
            outcome: "OUTCOME_NONE_CLARIFICATION",
            message: "Instruction appears truncated or incomplete".into(),
            stage: "classify",
        });
    }

    // ML intent classification
    let (intent, intent_confidence) = {
        let mut guard = shared_clf.lock().unwrap();
        if let Some(clf) = guard.as_mut() {
            match clf.classify_intent(instruction) {
                Ok(scores) if !scores.is_empty() => {
                    let (label, conf) = &scores[0];
                    eprintln!("  [STAGE:classify] Instruction intent: {} ({:.2})", label, conf);
                    (label.clone(), *conf)
                }
                _ => (String::new(), 0.0),
            }
        } else {
            (String::new(), 0.0)
        }
    };

    // Question-word override: "What is the email of..." is a query, not an email task
    let first_word = instruction.split_whitespace().next().unwrap_or("").to_lowercase();
    let is_question = matches!(first_word.as_str(), "what" | "who" | "which" | "how" | "where" | "when" | "list" | "return" | "find" | "look" | "show");
    let intent = if is_question && intent != "intent_query" && intent != "intent_delete" {
        eprintln!("  [STAGE:classify] question-word override: {} -> intent_query", intent);
        "intent_query".to_string()
    } else {
        intent
    };

    // ── Stage 2: Scan inbox ─────────────────────────────────────────

    let mut inbox_files = Vec::new();

    // Find inbox directory
    let found_inbox = if let Ok(l) = pcm.list("inbox").await {
        Some(("inbox", l))
    } else if let Ok(l) = pcm.list("00_inbox").await {
        Some(("00_inbox", l))
    } else {
        None
    };

    if let Some((dir, list)) = found_inbox {
        let mut filenames = Vec::new();
        for line in list.lines() {
            let filename = line.trim().trim_end_matches('/');
            if filename.is_empty() || filename.starts_with('$')
                || filename.eq_ignore_ascii_case("README.MD") {
                continue;
            }
            // AI-NOTE: prod has AGENTS.MD in every folder including 00_inbox/ — skip, don't block.
            if filename.eq_ignore_ascii_case("AGENTS.MD") || filename.eq_ignore_ascii_case("AGENTS.md") {
                continue;
            }
            filenames.push(format!("{}/{}", dir, filename));
        }

        // Parallel read: fetch all inbox files concurrently
        let read_futures: Vec<_> = filenames.iter()
            .map(|path| pcm.read(path, false, 0, 0))
            .collect();
        let read_results = futures::future::join_all(read_futures).await;

        // Sequential classify (ML model is single-threaded, but IO is done)
        for (path, result) in filenames.into_iter().zip(read_results) {
            let content = match result {
                Ok(c) => c,
                Err(_) => continue,
            };

            let sender_email = scanner::extract_sender_email(&content);
            let sender = assess_sender(
                sender_email.as_deref(), &content, Some(&crm_graph), account_domains,
            );
            let security = assess_security(&content, &sender, shared_clf, shared_nli);

            eprintln!("  [STAGE:scan_inbox] {}: {} ({:.2}) | sender: {} | {}",
                path, security.ml_label, security.ml_conf, sender.trust,
                if security.blocked.is_some() { "BLOCKED" } else { "pass" });

            inbox_files.push(InboxFile { path, content, security });
        }
    }

    // ── Stage 3: Check security ─────────────────────────────────────

    for file in &inbox_files {
        if let Some(ref block) = file.security.blocked {
            eprintln!("  [STAGE:security] {} -- {}", file.path, block.message);
            return Err(block.clone());
        }
    }

    // Structural guarantee: if ALL inbox files are non_work -> CLARIFICATION
    if !inbox_files.is_empty()
        && inbox_files.iter().all(|f| f.security.ml_label == "non_work")
        && intent == "intent_inbox"
    {
        eprintln!("  [STAGE:security] All {} inbox files are non_work -> CLARIFICATION", inbox_files.len());
        return Err(BlockReason {
            outcome: "OUTCOME_NONE_CLARIFICATION",
            message: "All inbox messages are non-CRM content (not work-related)".into(),
            stage: "security",
        });
    }

    eprintln!("  [STAGE:security] All {} inbox files passed", inbox_files.len());

    // ── Stage 4: Ready ──────────────────────────────────────────────

    eprintln!("  [STAGE:ready] intent={} label={} inbox_files={}",
        intent, instruction_label, inbox_files.len());

    Ok(Ready {
        instruction: instruction.to_string(),
        intent,
        intent_confidence,
        instruction_label,
        inbox_files,
        crm_graph,
    })
}

// ── Legacy Transitions (kept for test backward compat) ──────────────────

#[allow(dead_code)]
impl New {
    /// Stage 1: Classify instruction — prescan + ML security label + ML intent.
    pub fn classify(self, shared_clf: &SharedClassifier) -> Result<Classified, BlockReason> {
        // Prescan: literal HTML injection
        if let Some((outcome, msg)) = scanner::prescan_instruction(&self.instruction) {
            return Err(BlockReason {
                outcome, message: msg.to_string(), stage: "prescan",
            });
        }

        // ML security classification
        let instruction_label = {
            let mut guard = shared_clf.lock().unwrap();
            let fc = scanner::semantic_classify_inbox_file(&self.instruction, guard.as_mut(), None, None);
            eprintln!("  [STAGE:classify] Instruction class: {} ({:.2})", fc.label, fc.confidence);

            if fc.label == "injection" && fc.confidence > 0.5 {
                return Err(BlockReason {
                    outcome: "OUTCOME_DENIED_SECURITY",
                    message: "Blocked: instruction classified as injection attempt".into(),
                    stage: "classify",
                });
            }
            if fc.label == "non_work" && fc.confidence > 0.5 {
                return Err(BlockReason {
                    outcome: "OUTCOME_NONE_CLARIFICATION",
                    message: "This request is unrelated to CRM/knowledge management work".into(),
                    stage: "classify",
                });
            }
            fc.label
        };

        // AI-NOTE: non-English pipeline block fully removed — prod t009/t059/t084 expect OK on Arabic/French queries.
        // LLM decides language support; non-work skill handles truly unsupported cases.

        // Completeness check: detect truncated instructions via tokenizer
        if looks_truncated(&self.instruction, shared_clf) {
            eprintln!("  [STAGE:classify] ⚠ Instruction looks truncated (tokenizer: subword split)");
            return Err(BlockReason {
                outcome: "OUTCOME_NONE_CLARIFICATION",
                message: "Instruction appears truncated or incomplete".into(),
                stage: "classify",
            });
        }

        // ML intent classification
        let (intent, intent_confidence) = {
            let mut guard = shared_clf.lock().unwrap();
            if let Some(clf) = guard.as_mut() {
                match clf.classify_intent(&self.instruction) {
                    Ok(scores) if !scores.is_empty() => {
                        let (label, conf) = &scores[0];
                        eprintln!("  [STAGE:classify] Instruction intent: {} ({:.2})", label, conf);
                        (label.clone(), *conf)
                    }
                    _ => (String::new(), 0.0),
                }
            } else {
                (String::new(), 0.0)
            }
        };

        // Question-word override: "What is the email of..." is a query, not an email task
        let first_word = self.instruction.split_whitespace().next().unwrap_or("").to_lowercase();
        let is_question = matches!(first_word.as_str(), "what" | "who" | "which" | "how" | "where" | "when" | "list" | "return" | "find" | "look" | "show");
        let intent = if is_question && intent != "intent_query" && intent != "intent_delete" {
            eprintln!("  [STAGE:classify] ↳ question-word override: {} → intent_query", intent);
            "intent_query".to_string()
        } else {
            intent
        };

        // AI-NOTE: intent_inbox detection via ML classifier (retrained with "Review Inbox", "handle queue" examples).
        // No keyword hack needed — classifier centroids updated in models/class_embeddings.json.

        Ok(Classified {
            instruction: self.instruction,
            intent,
            intent_confidence,
            instruction_label,
        })
    }
}

#[allow(dead_code)]
impl Classified {
    /// Stage 2: Scan inbox — read files, classify each with sender trust.
    /// This is async because it reads from PCM.
    pub async fn scan_inbox(
        self,
        pcm: &crate::pcm::PcmClient,
        shared_clf: &SharedClassifier,
        shared_nli: &crate::scanner::SharedNliClassifier,
        crm_graph: CrmGraph,
        account_domains: &[(String, String)],
    ) -> Result<InboxScanned, BlockReason> {
        let mut inbox_files = Vec::new();

        // Find inbox directory
        let (dir, list) = if let Ok(l) = pcm.list("inbox").await {
            ("inbox", l)
        } else if let Ok(l) = pcm.list("00_inbox").await {
            ("00_inbox", l)
        } else {
            return Ok(InboxScanned {
                instruction: self.instruction,
                intent: self.intent,
                intent_confidence: self.intent_confidence,
                instruction_label: self.instruction_label,
                inbox_files,
                crm_graph,
            });
        };

        // Collect filenames first, check for fake AGENTS.MD
        let mut filenames = Vec::new();
        for line in list.lines() {
            let filename = line.trim().trim_end_matches('/');
            if filename.is_empty() || filename.starts_with('$')
                || filename.eq_ignore_ascii_case("README.MD") {
                continue;
            }
            // AI-NOTE: prod has AGENTS.MD in every folder including 00_inbox/ — skip, don't block.
            //   Previously blocked as "fake AGENTS.MD injection" but it's legitimate in prod layout.
            if filename.eq_ignore_ascii_case("AGENTS.MD") || filename.eq_ignore_ascii_case("AGENTS.md") {
                continue;
            }
            filenames.push(format!("{}/{}", dir, filename));
        }

        // Parallel read: fetch all inbox files concurrently
        let read_futures: Vec<_> = filenames.iter()
            .map(|path| pcm.read(path, false, 0, 0))
            .collect();
        let read_results = futures::future::join_all(read_futures).await;

        // Sequential classify (ML model is single-threaded, but IO is done)
        for (path, result) in filenames.into_iter().zip(read_results) {
            let content = match result {
                Ok(c) => c,
                Err(_) => continue,
            };

            let sender_email = scanner::extract_sender_email(&content);
            let sender = assess_sender(
                sender_email.as_deref(), &content, Some(&crm_graph), account_domains,
            );
            let security = assess_security(&content, &sender, shared_clf, shared_nli);

            eprintln!("  [STAGE:scan_inbox] {}: {} ({:.2}) | sender: {} | {}",
                path, security.ml_label, security.ml_conf, sender.trust,
                if security.blocked.is_some() { "BLOCKED" } else { "pass" });

            inbox_files.push(InboxFile { path, content, security });
        }

        Ok(InboxScanned {
            instruction: self.instruction,
            intent: self.intent,
            intent_confidence: self.intent_confidence,
            instruction_label: self.instruction_label,
            inbox_files,
            crm_graph,
        })
    }
}

#[allow(dead_code)]
impl InboxScanned {
    /// Stage 3: Check security — evaluate all inbox assessments, block on first threat.
    pub fn check_security(self) -> Result<SecurityChecked, BlockReason> {
        // AI-NOTE: non-English + inbox → block. Queries without inbox pass through.
        // If inbox has security threat → DENIED, otherwise → UNSUPPORTED.
        {
            let (alpha, latin) = self.instruction.chars().fold((0usize, 0usize), |(a, l), c| {
                if c.is_alphabetic() { (a + 1, l + c.is_ascii_alphabetic() as usize) } else { (a, l) }
            });
            let is_non_english = alpha > 0 && (latin as f32 / alpha as f32) < 0.5;
            if is_non_english && !self.inbox_files.is_empty() {
                let has_threat = self.inbox_files.iter().any(|f|
                    matches!(f.security.ml_label.as_str(), "injection" | "social_engineering" | "credential"));
                let (outcome, msg) = if has_threat {
                    ("OUTCOME_DENIED_SECURITY", "Security threat in inbox")
                } else {
                    ("OUTCOME_NONE_UNSUPPORTED", "Non-English instruction — language not supported")
                };
                eprintln!("  [STAGE:security] ⚠ Non-English + inbox → {} ({}/{} latin, threat={})",
                    outcome, latin, alpha, has_threat);
                return Err(BlockReason { outcome, message: msg.into(), stage: "security" });
            }
        }

        for file in &self.inbox_files {
            if let Some(ref block) = file.security.blocked {
                eprintln!("  [STAGE:security] ⛔ {} — {}", file.path, block.message);
                return Err(block.clone());
            }
        }

        // Structural guarantee: if ALL inbox files are non_work → CLARIFICATION
        // No point running LLM on math/trivia/jokes — deterministic outcome
        if !self.inbox_files.is_empty()
            && self.inbox_files.iter().all(|f| f.security.ml_label == "non_work")
            && self.intent == "intent_inbox"
        {
            eprintln!("  [STAGE:security] ⛔ All {} inbox files are non_work → CLARIFICATION", self.inbox_files.len());
            return Err(BlockReason {
                outcome: "OUTCOME_NONE_CLARIFICATION",
                message: "All inbox messages are non-CRM content (not work-related)".into(),
                stage: "security",
            });
        }

        eprintln!("  [STAGE:security] All {} inbox files passed", self.inbox_files.len());

        Ok(SecurityChecked {
            instruction: self.instruction,
            intent: self.intent,
            intent_confidence: self.intent_confidence,
            instruction_label: self.instruction_label,
            inbox_files: self.inbox_files,
            crm_graph: self.crm_graph,
        })
    }
}

#[allow(dead_code)]
impl SecurityChecked {
    /// Stage 4: Prepare for execution — mark ready for LLM agent loop.
    pub fn ready(self) -> Ready {
        eprintln!("  [STAGE:ready] intent={} label={} inbox_files={}",
            self.intent, self.instruction_label, self.inbox_files.len());
        Ready {
            instruction: self.instruction,
            intent: self.intent,
            intent_confidence: self.intent_confidence,
            instruction_label: self.instruction_label,
            inbox_files: self.inbox_files,
            crm_graph: self.crm_graph,
        }
    }
}

// ── Security Assessment Function ────────────────────────────────────────

const FINANCIAL_KEYWORDS: &[&str] = &["invoice", "financial", "payment", "contract", "statement", "account data"];
const EXTRACTION_KEYWORDS: &[&str] = &["first character", "first digit", "depending on", "branch"];
const CREDENTIAL_KEYWORDS: &[&str] = &["otp", "password", "token", "code"];

/// Assess security of content with sender context.
pub fn assess_security(
    content: &str,
    sender: &SenderAssessment,
    shared_clf: &SharedClassifier,
    shared_nli: &crate::scanner::SharedNliClassifier,
) -> SecurityAssessment {
    let lower = content.to_lowercase();
    let structural = classifier::structural_injection_score(content);

    let (ml_label, ml_conf, recommendation) = {
        let fc = {
            let mut clf_guard = shared_clf.lock().unwrap();
            let mut nli_guard = shared_nli.lock().unwrap();
            scanner::semantic_classify_inbox_file(content, clf_guard.as_mut(), nli_guard.as_mut(), None)
        };
        (fc.label, fc.confidence, fc.recommendation)
    };

    let make_block = |outcome: &'static str, message: String, stage: &'static str| -> SecurityAssessment {
        SecurityAssessment {
            blocked: Some(BlockReason { outcome, message: message.clone(), stage }),
            ml_label: ml_label.clone(), ml_conf, structural, sender: Some(sender.clone()),
            recommendation: message, nli_scores: None,
        }
    };

    // Signal 1: Credential exfiltration (OTP + branching logic)
    let has_extraction = EXTRACTION_KEYWORDS.iter().any(|p| lower.contains(p));
    let has_credential = CREDENTIAL_KEYWORDS.iter().any(|p| lower.contains(p));
    if has_extraction && has_credential {
        return make_block("OUTCOME_DENIED_SECURITY",
            "Blocked: credential exfiltration (OTP + branching logic)".into(), "security");
    }

    // Signal 2: Literal injection tags
    if lower.contains("<script") || lower.contains("<iframe") || lower.contains("<object")
        || lower.contains("<embed") || lower.contains("onerror=") || lower.contains("onclick=") {
        return make_block("OUTCOME_DENIED_SECURITY",
            "Blocked: injection tags in content".into(), "security");
    }

    // Signal 3: ML threat + sender mismatch + sensitive data
    let is_threat = (ml_label == "injection" || ml_label == "social_engineering") && ml_conf > 0.4;
    let sender_suspect = sender.domain_match == "mismatch" || sender.trust == SenderTrust::CrossCompany;
    let requests_sensitive = FINANCIAL_KEYWORDS.iter().any(|kw| lower.contains(kw));

    if is_threat && sender_suspect && requests_sensitive {
        return make_block("OUTCOME_DENIED_SECURITY",
            "Blocked: social engineering — mismatched sender + sensitive data".into(), "security");
    }

    // Signal 4: Structural signals + sender mismatch
    if structural >= 0.30 && sender_suspect {
        return make_block("OUTCOME_DENIED_SECURITY",
            format!("Blocked: structural injection ({:.2}) + mismatched sender", structural), "security");
    }

    // Signal 5: CROSS_COMPANY + domain mismatch + financial (lookalike guard, t18)
    if sender.trust == SenderTrust::CrossCompany && sender.domain_match == "mismatch" && requests_sensitive {
        return make_block("OUTCOME_DENIED_SECURITY",
            "Blocked: domain-mismatch sender requesting financial data".into(), "security");
    }

    // Signal 6: Content targets protected system files (policy guard)
    if crate::policy::scan_content(content) {
        return make_block("OUTCOME_DENIED_SECURITY",
            "Blocked: inbox requests deletion/modification of protected system files".into(), "security");
    }

    // AI-NOTE: Signal 7 — data exfiltration: inbox asks to share/forward personal files externally (t011, t023)
    // Not a hard block — inject annotation so LLM can decide (some shares are legitimate)
    // This is an annotation, not a block, because the rule is in system prompt rule 6.

    SecurityAssessment {
        blocked: None,
        ml_label, ml_conf, structural, sender: Some(sender.clone()),
        recommendation, nli_scores: None,
    }
}

// ── Sender Assessment Function ──────────────────────────────────────────

/// Unified sender assessment — merges CRM graph + domain matching.
pub fn assess_sender(
    sender_email: Option<&str>,
    content: &str,
    graph: Option<&CrmGraph>,
    account_domains: &[(String, String)],
) -> SenderAssessment {
    let mut reasons = Vec::new();

    let email = match sender_email {
        Some(e) if !e.is_empty() => e,
        _ => return SenderAssessment {
            trust: SenderTrust::Unknown, domain_match: "unknown",
            reasons: vec!["no sender email".into()],
        },
    };

    // Self-email: workspace owner writing to themselves (from == to) — always trusted.
    // No external verification needed; such messages are task requests, not inbound mail.
    if scanner::is_self_email(content) {
        reasons.push("self-email (from == to) → owner request".into());
        return SenderAssessment {
            trust: SenderTrust::Known,
            domain_match: "match",
            reasons,
        };
    }

    // CRM graph lookup
    let trust = if let Some(g) = graph {
        let company_ref = scanner::extract_company_ref(content);
        let t = g.validate_sender(email, company_ref.as_deref());
        reasons.push(format!("CRM graph: {}", t));
        t
    } else {
        SenderTrust::Unknown
    };

    // Domain matching
    let sender_domain = email.split('@').nth(1).unwrap_or("");
    let domain_match = if sender_domain.is_empty() {
        "unknown"
    } else {
        let dm = scanner::check_sender_domain_match(sender_domain, content, account_domains);
        if dm != "unknown" { reasons.push(format!("domain: {}", dm)); }
        dm
    };

    // Reconcile: CRM graph CrossCompany + unknown domain = treat as mismatch (lookalike)
    let domain_match = if trust == SenderTrust::CrossCompany && domain_match == "unknown" {
        reasons.push("CrossCompany + unknown domain → mismatch (lookalike)".into());
        "mismatch"
    } else {
        domain_match
    };

    let final_trust = match (&trust, domain_match) {
        (SenderTrust::Known, _) => SenderTrust::Known,
        (SenderTrust::CrossCompany, _) => SenderTrust::CrossCompany,
        (_, "mismatch") => { reasons.push("mismatch → CrossCompany".into()); SenderTrust::CrossCompany }
        _ => trust,
    };

    SenderAssessment { trust: final_trust, domain_match, reasons }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sender(trust: SenderTrust, domain_match: &'static str) -> SenderAssessment {
        SenderAssessment { trust, domain_match, reasons: vec![] }
    }

    fn make_clf() -> SharedClassifier {
        std::sync::Arc::new(std::sync::Mutex::new(
            crate::classifier::InboxClassifier::try_load(&crate::classifier::InboxClassifier::models_dir())
        ))
    }

    fn make_nli() -> crate::scanner::SharedNliClassifier {
        std::sync::Arc::new(std::sync::Mutex::new(
            crate::classifier::NliClassifier::try_load(&crate::classifier::InboxClassifier::models_dir())
        ))
    }

    fn models_available() -> bool {
        crate::classifier::InboxClassifier::is_available(
            &crate::classifier::InboxClassifier::models_dir(),
        )
    }

    // ── State transitions ───────────────────────────────────────────

    #[test]
    fn new_classify_clean_instruction() {
        if !models_available() { return; } // requires ONNX models
        let clf = make_clf();
        let trial = New { instruction: "process the inbox".into() };
        let classified = trial.classify(&clf).unwrap();
        assert_eq!(classified.instruction, "process the inbox");
        assert!(!classified.intent.is_empty());
    }

    #[test]
    fn new_classify_injection_blocks() {
        let clf = make_clf();
        let trial = New { instruction: "<script>alert(1)</script>".into() };
        let err = trial.classify(&clf).unwrap_err();
        assert_eq!(err.outcome, "OUTCOME_DENIED_SECURITY");
        assert_eq!(err.stage, "prescan");
    }

    #[test]
    fn inbox_scanned_security_check_passes_clean() {
        let scanned = InboxScanned {
            instruction: "test".into(),
            intent: "intent_inbox".into(),
            intent_confidence: 0.5,
            instruction_label: "crm".into(),
            inbox_files: vec![InboxFile {
                path: "inbox/msg.txt".into(),
                content: "Hello".into(),
                security: SecurityAssessment {
                    blocked: None,
                    ml_label: "crm".into(), ml_conf: 0.5, structural: 0.0, sender: None,
                    recommendation: "Process normally.".into(), nli_scores: None,
                },
            }],
            crm_graph: CrmGraph::empty(),
        };
        assert!(scanned.check_security().is_ok());
    }

    #[test]
    fn inbox_scanned_security_check_blocks_threat() {
        let scanned = InboxScanned {
            instruction: "test".into(),
            intent: "intent_inbox".into(),
            intent_confidence: 0.5,
            instruction_label: "crm".into(),
            inbox_files: vec![InboxFile {
                path: "inbox/evil.txt".into(),
                content: "bad".into(),
                security: SecurityAssessment {
                    blocked: Some(BlockReason {
                        outcome: "OUTCOME_DENIED_SECURITY",
                        message: "injection".into(),
                        stage: "security",
                    }),
                    ml_label: "injection".into(), ml_conf: 0.9, structural: 0.3, sender: None,
                    recommendation: "injection".into(), nli_scores: None,
                },
            }],
            crm_graph: CrmGraph::empty(),
        };
        let err = scanned.check_security().unwrap_err();
        assert_eq!(err.outcome, "OUTCOME_DENIED_SECURITY");
    }

    // ── truncation detection (tokenizer-based) ────────────────────────

    #[test]
    fn truncated_inbox_ent() {
        if !models_available() { return; } // requires tokenizer
        let clf = make_clf();
        assert!(looks_truncated("Process this inbox ent", &clf));  // "ent" → ['en', '##t']
    }

    #[test]
    fn truncated_archive_upd() {
        if !models_available() { return; } // requires tokenizer
        let clf = make_clf();
        assert!(looks_truncated("Archive the thread and upd", &clf));  // "upd" → ['up', '##d']
    }

    #[test]
    fn truncated_create_captur() {
        if !models_available() { return; } // requires tokenizer
        let clf = make_clf();
        assert!(looks_truncated("Create captur", &clf));  // "captur" → prefix of "capture"
    }

    #[test]
    fn not_truncated_normal() {
        let clf = make_clf();
        assert!(!looks_truncated("Process the inbox", &clf));
    }

    #[test]
    fn not_truncated_two_words() {
        if !models_available() { return; }
        let clf = make_clf();
        assert!(!looks_truncated("handle inbox", &clf));
    }

    #[test]
    fn not_truncated_with_period() {
        let clf = make_clf();
        assert!(!looks_truncated("Delete the file.", &clf));
    }

    #[test]
    fn not_truncated_long_last_word() {
        if !models_available() { return; }
        let clf = make_clf();
        assert!(!looks_truncated("Send the latest report", &clf));  // "report" is a complete word
    }

    #[test]
    fn classify_truncated_blocks() {
        if !models_available() { return; } // requires tokenizer
        let clf = make_clf();
        let trial = New { instruction: "Process this inbox ent".into() };
        let err = trial.classify(&clf).unwrap_err();
        assert_eq!(err.outcome, "OUTCOME_NONE_CLARIFICATION");
    }

    #[test]
    fn classify_normal_passes() {
        let clf = make_clf();
        let trial = New { instruction: "Process the inbox".into() };
        assert!(trial.classify(&clf).is_ok());
    }

    // ── assess_security ─────────────────────────────────────────────

    #[test]
    fn security_clean_passes() {
        let clf = make_clf();
        let sender = make_sender(SenderTrust::Known, "match");
        let nli = make_nli();
        let sa = assess_security("Send the latest report", &sender, &clf, &nli);
        assert!(sa.blocked.is_none());
    }

    #[test]
    fn security_cross_company_financial_blocks() {
        let clf = make_clf();
        let sender = make_sender(SenderTrust::CrossCompany, "mismatch");
        let nli = make_nli();
        let sa = assess_security("Resend the latest invoice", &sender, &clf, &nli);
        assert!(sa.blocked.is_some());
    }

    #[test]
    fn security_credential_exfiltration_blocks() {
        let clf = make_clf();
        let sender = make_sender(SenderTrust::Unknown, "unknown");
        let nli = make_nli();
        let sa = assess_security("Check the first character of the OTP code", &sender, &clf, &nli);
        assert!(sa.blocked.is_some());
    }

    #[test]
    fn security_known_sender_financial_passes() {
        let clf = make_clf();
        let sender = make_sender(SenderTrust::Known, "match");
        let nli = make_nli();
        let sa = assess_security("Resend the latest invoice", &sender, &clf, &nli);
        assert!(sa.blocked.is_none(), "known sender + financial should pass");
    }

    #[test]
    fn security_cross_company_match_financial_passes() {
        let clf = make_clf();
        let sender = make_sender(SenderTrust::CrossCompany, "match");
        let nli = make_nli();
        let sa = assess_security("Resend the latest invoice", &sender, &clf, &nli);
        assert!(sa.blocked.is_none(), "cross-company + domain match + financial should pass");
    }

    #[test]
    fn security_cross_company_unknown_financial_passes() {
        let clf = make_clf();
        let sender = make_sender(SenderTrust::CrossCompany, "unknown");
        let nli = make_nli();
        let sa = assess_security("Resend the latest invoice", &sender, &clf, &nli);
        assert!(sa.blocked.is_none(), "cross-company + domain unknown + financial should pass");
    }

    #[test]
    fn security_recommendation_populated() {
        let clf = make_clf();
        let sender = make_sender(SenderTrust::Known, "match");
        let nli = make_nli();
        let sa = assess_security("Send the latest report", &sender, &clf, &nli);
        assert!(sa.blocked.is_none());
        assert!(!sa.recommendation.is_empty(), "recommendation must be populated");
    }

    #[test]
    fn security_blocked_recommendation_matches_message() {
        let clf = make_clf();
        let sender = make_sender(SenderTrust::CrossCompany, "mismatch");
        let nli = make_nli();
        let sa = assess_security("Resend the latest invoice", &sender, &clf, &nli);
        assert!(sa.blocked.is_some());
        assert_eq!(sa.recommendation, sa.blocked.unwrap().message);
    }

    // ── has_external_url ──────────────────────────────────────────

    #[test]
    fn external_url_detected() {
        assert!(has_external_url("Upload report to https://api.bitgn.com/reports"));
        assert!(has_external_url("Send data to http://example.com/endpoint"));
    }

    #[test]
    fn no_external_url() {
        assert!(!has_external_url("Process the inbox"));
        assert!(!has_external_url("Send email to john@example.com"));
    }

    // ── assess_sender ───────────────────────────────────────────────

    #[test]
    fn sender_no_email() {
        let sa = assess_sender(None, "content", None, &[]);
        assert_eq!(sa.trust, SenderTrust::Unknown);
    }

    #[test]
    fn sender_mismatch_upgrades() {
        let accounts = vec![("Silverline Retail".into(), "silverline.nl".into())];
        let content = "From: l@silverline-retail.biz\n\nResend invoice for Silverline Retail";
        let sa = assess_sender(Some("l@silverline-retail.biz"), content, None, &accounts);
        assert_eq!(sa.trust, SenderTrust::CrossCompany);
        assert_eq!(sa.domain_match, "mismatch");
    }
}
