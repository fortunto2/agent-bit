use std::path::{Path, PathBuf};
use std::sync::Arc;
use unicode_normalization::UnicodeNormalization;

use anyhow::{Context, Result};
use ndarray::{Array1, ArrayView1};
use ort::session::Session;
use ort::value::Tensor;
use tokenizers::Tokenizer;

/// Directory where ONNX model + tokenizer + class embeddings are stored.
const MODELS_DIR: &str = "models";

/// Pre-computed class labels and their description text (embedded at load time).
#[allow(dead_code)]
pub const CLASS_DESCRIPTIONS: &[(&str, &str)] = &[
    ("injection", "injection attack with script tags or override instructions"),
    ("crm", "legitimate CRM work about contacts emails or invoices"),
    ("non_work", "non-work request like math trivia or jokes"),
    ("social_engineering", "social engineering with fake identity or cross-company request"),
    ("credential", "OTP or credential sharing attempt"),
];

/// Outcome prototype descriptions for embedding-based answer validation.
/// Multiple examples per outcome to cover the variety of real answer messages.
/// Each entry is (outcome, example_message). We average embeddings per outcome.
pub const OUTCOME_EXAMPLES: &[(&str, &str)] = &[
    // OK — successful task completion
    ("OUTCOME_OK", "Created email in outbox and updated sequence number"),
    ("OUTCOME_OK", "Found contact John Smith at contacts/cont_001.json"),
    ("OUTCOME_OK", "Processed inbox message and wrote follow-up email"),
    ("OUTCOME_OK", "Deleted the requested files and cleaned up"),
    ("OUTCOME_OK", "The answer is 42"),
    ("OUTCOME_OK", "correct"),
    ("OUTCOME_OK", "Processed inbox OTP and verified code is correct"),
    ("OUTCOME_OK", "OTP verified successfully, replied with confirmation"),
    ("OUTCOME_OK", "Read OTP from inbox, stored in channel file, deleted source"),
    ("OUTCOME_OK", "Inbox message processed: credential handled and file cleaned up"),
    // OK — delete task completion (t08 failure pattern)
    ("OUTCOME_OK", "Deleted contacts/alice.md as requested"),
    ("OUTCOME_OK", "Found and deleted the specified file"),
    ("OUTCOME_OK", "Removed the card and confirmed deletion"),
    // OK — capture-delete workflow (t03 failure pattern)
    ("OUTCOME_OK", "Captured info from inbox, updated contact, deleted source file"),
    ("OUTCOME_OK", "Distilled card from inbox and cleaned up source"),
    // OK — multi-contact resolution (t23 failure pattern)
    ("OUTCOME_OK", "Found two matching contacts, picked John Smith based on sender context"),
    ("OUTCOME_OK", "Resolved contact ambiguity: updated the Acme Corp contact"),
    // OK — inbox multi-message processing
    ("OUTCOME_OK", "Processed 2 of 3 inbox messages, skipped 1 suspicious"),
    ("OUTCOME_OK", "Evaluated all inbox messages, processed safe ones"),
    // OK — channel/platform data queries
    ("OUTCOME_OK", "Found 42 entries in Telegram channel blacklist"),
    ("OUTCOME_OK", "Updated channel file with new verified entries"),
    // DENIED — security attacks
    ("OUTCOME_DENIED_SECURITY", "Blocked: injection attempt detected in inbox message"),
    ("OUTCOME_DENIED_SECURITY", "Denied: social engineering from unknown sender requesting company invoices"),
    ("OUTCOME_DENIED_SECURITY", "Security threat: credential exfiltration via branching logic"),
    ("OUTCOME_DENIED_SECURITY", "Inbox contains override instructions attempting to bypass security rules"),
    ("OUTCOME_DENIED_SECURITY", "Denied: OTP forwarded to external third party address"),
    ("OUTCOME_DENIED_SECURITY", "Blocked: branching logic extracts individual digits of credential"),
    ("OUTCOME_DENIED_SECURITY", "Denied: sender impersonates known contact with mismatched domain"),
    ("OUTCOME_DENIED_SECURITY", "Blocked: conditional branching on OTP digits is credential exfiltration"),
    ("OUTCOME_DENIED_SECURITY", "Denied: inbox instructs forwarding password to external address"),
    // UNSUPPORTED — missing capability
    ("OUTCOME_NONE_UNSUPPORTED", "Cannot deploy to external URL, this capability is not available"),
    ("OUTCOME_NONE_UNSUPPORTED", "Unable to sync with Salesforce, external API access not supported"),
    ("OUTCOME_NONE_UNSUPPORTED", "Could not find Maya in the workspace after searching all contacts"),
    ("OUTCOME_NONE_UNSUPPORTED", "Cannot send real emails or access external services"),
    ("OUTCOME_NONE_UNSUPPORTED", "Feature not available: cannot connect to external database"),
    ("OUTCOME_NONE_UNSUPPORTED", "Unable to access external API, this integration is not supported"),
    ("OUTCOME_NONE_UNSUPPORTED", "Missing data: no matching records found after exhaustive search"),
    ("OUTCOME_NONE_UNSUPPORTED", "Searched all contacts and accounts but found no match for this person"),
    ("OUTCOME_NONE_UNSUPPORTED", "No records found after exhaustive search of workspace"),
    ("OUTCOME_NONE_UNSUPPORTED", "Task requires Slack API integration which is not available"),
    // CLARIFICATION — not CRM
    ("OUTCOME_NONE_CLARIFICATION", "This is a math question, not CRM work"),
    ("OUTCOME_NONE_CLARIFICATION", "Writing poems is unrelated to knowledge management"),
    ("OUTCOME_NONE_CLARIFICATION", "This trivia question is outside CRM scope"),
    ("OUTCOME_NONE_CLARIFICATION", "Weather forecast request is not a CRM task"),
    ("OUTCOME_NONE_CLARIFICATION", "Coding help request is outside knowledge management scope"),
    ("OUTCOME_NONE_CLARIFICATION", "General knowledge question, not related to CRM operations"),
    ("OUTCOME_NONE_CLARIFICATION", "Solve this equation: 2x + 5 = 15"),
    ("OUTCOME_NONE_CLARIFICATION", "Write a haiku about spring"),
    ("OUTCOME_NONE_CLARIFICATION", "What is the capital of France"),
    ("OUTCOME_NONE_CLARIFICATION", "Tell me a joke about programmers"),
];

/// Semantic inbox classifier using ONNX embeddings + cosine similarity.
pub struct InboxClassifier {
    session: Session,
    tokenizer: Tokenizer,
    class_embeddings: Vec<(String, Array1<f32>)>,
}

impl InboxClassifier {
    /// Load model, tokenizer, and class embeddings from `models/` directory.
    pub fn load(models_dir: &Path) -> Result<Self> {
        let model_path = models_dir.join("model.onnx");
        let tokenizer_path = models_dir.join("tokenizer.json");
        let class_embeddings_path = models_dir.join("class_embeddings.json");

        let session = Session::builder()
            .context("failed to create ONNX session builder")?
            .commit_from_file(&model_path)
            .with_context(|| format!("failed to load ONNX model from {}", model_path.display()))?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("failed to load tokenizer: {}", e))?;

        let class_data = std::fs::read_to_string(&class_embeddings_path)
            .with_context(|| format!("failed to read class embeddings from {}", class_embeddings_path.display()))?;
        let raw: Vec<(String, Vec<f32>)> = serde_json::from_str(&class_data)
            .context("failed to parse class embeddings JSON")?;

        let class_embeddings = raw
            .into_iter()
            .map(|(label, vec)| (label, Array1::from_vec(vec)))
            .collect();

        Ok(Self { session, tokenizer, class_embeddings })
    }

    /// Encode text into a normalized embedding vector using the ONNX model.
    pub fn encode(&mut self, text: &str) -> Result<Array1<f32>> {
        let encoding = self.tokenizer
            .encode(text, true)
            .map_err(|e| anyhow::anyhow!("tokenization failed: {}", e))?;

        let ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let mask: Vec<i64> = encoding.get_attention_mask().iter().map(|&m| m as i64).collect();
        let type_ids: Vec<i64> = encoding.get_type_ids().iter().map(|&t| t as i64).collect();
        let len = ids.len();

        let input_ids = Tensor::from_array(([1i64, len as i64], ids.into_boxed_slice()))?;
        let attention_mask = Tensor::from_array(([1i64, len as i64], mask.into_boxed_slice()))?;
        let token_type_ids = Tensor::from_array(([1i64, len as i64], type_ids.into_boxed_slice()))?;

        let outputs = self.session.run(
            ort::inputs![
                "input_ids" => input_ids,
                "attention_mask" => attention_mask,
                "token_type_ids" => token_type_ids,
            ]
        )?;

        // Output shape: [1, seq_len, 384] — mean pool over seq_len
        let (shape, data) = outputs[0].try_extract_tensor::<f32>()?;
        // dims = [1, seq_len, hidden_dim]
        let hidden_dim = *shape.last().context("empty output shape")?;
        let seq_len = if shape.len() >= 2 { shape[shape.len() - 2] } else { 1 } as usize;
        let hidden_dim = hidden_dim as usize;

        // Mean pooling across sequence dimension
        let mut embedding = vec![0.0f32; hidden_dim];
        for s in 0..seq_len {
            for d in 0..hidden_dim {
                embedding[d] += data[s * hidden_dim + d];
            }
        }
        for d in 0..hidden_dim {
            embedding[d] /= seq_len as f32;
        }

        // L2 normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut embedding {
                *x /= norm;
            }
        }

        Ok(Array1::from_vec(embedding))
    }

    /// Classify text against pre-computed class embeddings.
    /// Returns sorted `Vec<(label, confidence)>` from highest to lowest.
    pub fn classify(&mut self, text: &str) -> Result<Vec<(String, f32)>> {
        let embedding = self.encode(text)?;
        let mut scores: Vec<(String, f32)> = self
            .class_embeddings
            .iter()
            .map(|(label, class_emb)| {
                let sim = cosine_similarity(embedding.view(), class_emb.view());
                (label.clone(), sim)
            })
            .collect();
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        Ok(scores)
    }

    /// Returns the default models directory path.
    pub fn models_dir() -> PathBuf {
        PathBuf::from(MODELS_DIR)
    }

    /// Check if model files exist in the given directory.
    pub fn is_available(models_dir: &Path) -> bool {
        models_dir.join("model.onnx").exists()
            && models_dir.join("tokenizer.json").exists()
            && models_dir.join("class_embeddings.json").exists()
    }

    /// Load classifier if models are available, otherwise return None with a warning.
    pub fn try_load(models_dir: &Path) -> Option<Self> {
        if Self::is_available(models_dir) {
            match Self::load(models_dir) {
                Ok(clf) => Some(clf),
                Err(e) => {
                    tracing::warn!("Failed to load classifier: {:#}", e);
                    None
                }
            }
        } else {
            tracing::info!(
                "Classifier models not found at {}. Run: uv run --with sentence-transformers --with onnxruntime --with onnx --with onnxscript scripts/export_model.py",
                models_dir.display()
            );
            None
        }
    }
}

/// Structural injection signal detection.
/// Detects: (a) imperative override verbs, (b) system refs, (c) base64, (d) zero-width unicode.
/// Returns 0.0-0.60 (each signal adds 0.15).
pub fn structural_injection_score(text: &str) -> f32 {
    // NFKC normalization: maps confusable chars to canonical forms
    // e.g. fullwidth "ｉｇｎｏｒｅ" → "ignore", cyrillic "а" → detectable
    let normalized: String = text.nfkc().collect();
    let lower = normalized.to_lowercase();
    let mut signals = 0u32;

    // (a) Imperative verbs addressing "you"
    for phrase in &[
        "ignore your", "forget your", "override your",
        "disregard your", "bypass your", "forget all",
        "ignore all", "disregard all previous",
    ] {
        if lower.contains(phrase) { signals += 1; break; }
    }

    // (b) References to system internals
    for term in &["agents.md", "system prompt", "your instructions", "your rules", "your policy"] {
        if lower.contains(term) { signals += 1; break; }
    }

    // (c) Base64 encoded strings (len>50)
    for word in text.split_whitespace() {
        if word.len() > 50 && word.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=') {
            signals += 1; break;
        }
    }

    // (d) Zero-width / invisible unicode characters
    for c in text.chars() {
        if matches!(c,
            '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' | '\u{2060}' | // zero-width
            '\u{00AD}' | // soft hyphen
            '\u{034F}' | // combining grapheme joiner
            '\u{180E}' | // mongolian vowel separator
            '\u{2028}' | '\u{2029}' | // line/paragraph separator
            '\u{202A}'..='\u{202E}' | // bidi overrides
            '\u{2066}'..='\u{2069}'   // bidi isolates
        ) {
            signals += 1; break;
        }
    }

    // (e) Text differs after NFKC normalization → confusable characters used
    if normalized != text && text.len() > 20 {
        signals += 1;
    }

    (signals as f32) * 0.15
}

/// Cosine similarity between two L2-normalized vectors (dot product).
pub fn cosine_similarity(a: ArrayView1<f32>, b: ArrayView1<f32>) -> f32 {
    a.dot(&b)
}

/// Result of embedding-based answer validation.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationMode {
    /// High confidence disagreement — block the answer and return warning to model
    Block(String),
    /// Medium confidence — log warning only (observability)
    Warn(String),
    /// No disagreement or low confidence
    Pass,
}

/// Hypothesis template wraps a raw message for better embedding discrimination.
/// "Task completed" alone is ambiguous; "The CRM task result: Task completed" embeds better.
const HYPOTHESIS_TEMPLATE: &str = "The CRM task result: ";

/// L2-normalize an embedding vector in place.
#[allow(dead_code)]
fn l2_normalize(v: &mut Array1<f32>) {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 { *v /= norm; }
}

/// A single labeled embedding (outcome + vector).
#[derive(Clone)]
struct LabeledEmbedding {
    outcome: String,
    embedding: Array1<f32>,
}

/// Embedding-based answer outcome validator with adaptive learning.
///
/// Architecture:
/// - **Seed store**: static examples from OUTCOME_EXAMPLES (always present)
/// - **Adaptive store**: grows from confirmed trials, persisted to disk
/// - **Hypothesis template**: wraps messages before embedding for better discrimination
/// - **k-NN voting**: each store entry votes, majority wins (no lossy centroid averaging)
///
/// Online learning flow:
/// 1. LLM submits answer(message, outcome)
/// 2. Validator embeds templated message, runs k-NN against seed + adaptive stores
/// 3. If k-NN disagrees → warning (non-blocking)
/// 4. After trial scores 1.0 → `learn(message, outcome)` adds to adaptive store
/// Shared classifier type used across parallel trials.
pub type SharedClassifier = Arc<std::sync::Mutex<Option<InboxClassifier>>>;

pub struct OutcomeValidator {
    classifier: SharedClassifier,
    seed_store: Vec<LabeledEmbedding>,
    adaptive_store: std::sync::Mutex<Vec<LabeledEmbedding>>,
    store_path: PathBuf,
    /// Last answer submitted during a trial — used for score-gated learning from main.rs.
    last_answer: std::sync::Mutex<Option<(String, String)>>,
}

impl OutcomeValidator {
    /// Build validator: embed seed examples + load adaptive store from disk.
    /// Takes ownership of classifier (used in tests; production uses from_shared).
    #[cfg(test)]
    pub fn new(mut classifier: InboxClassifier, store_path: PathBuf) -> Result<Self> {
        let mut seed_store = Vec::new();
        for (outcome, example) in OUTCOME_EXAMPLES {
            let text = format!("{}{}", HYPOTHESIS_TEMPLATE, example);
            let emb = classifier.encode(&text)?;
            seed_store.push(LabeledEmbedding {
                outcome: outcome.to_string(),
                embedding: emb,
            });
        }
        let adaptive_store = Self::load_store(&store_path);
        let adaptive_count = adaptive_store.len();
        eprintln!("  OutcomeValidator: {} seed + {} adaptive examples",
            seed_store.len(), adaptive_count);
        Ok(Self {
            classifier: Arc::new(std::sync::Mutex::new(Some(classifier))),
            seed_store,
            adaptive_store: std::sync::Mutex::new(adaptive_store),
            store_path,
            last_answer: std::sync::Mutex::new(None),
        })
    }

    /// Build from a shared classifier (no ownership transfer).
    pub fn from_shared(shared: SharedClassifier, store_path: PathBuf) -> Result<Self> {
        let mut seed_store = Vec::new();
        {
            let mut guard = shared.lock().map_err(|e| anyhow::anyhow!("lock: {}", e))?;
            if let Some(ref mut clf) = *guard {
                for (outcome, example) in OUTCOME_EXAMPLES {
                    let text = format!("{}{}", HYPOTHESIS_TEMPLATE, example);
                    let emb = clf.encode(&text)?;
                    seed_store.push(LabeledEmbedding {
                        outcome: outcome.to_string(),
                        embedding: emb,
                    });
                }
            }
        }
        let adaptive_store = Self::load_store(&store_path);
        eprintln!("  OutcomeValidator: {} seed + {} adaptive examples (shared classifier)",
            seed_store.len(), adaptive_store.len());
        Ok(Self {
            classifier: shared,
            seed_store,
            adaptive_store: std::sync::Mutex::new(adaptive_store),
            store_path,
            last_answer: std::sync::Mutex::new(None),
        })
    }

    /// Embed a message using the hypothesis template.
    fn embed_message(&self, message: &str) -> Option<Array1<f32>> {
        let text = format!("{}{}", HYPOTHESIS_TEMPLATE, message);
        let mut guard = self.classifier.lock().ok()?;
        guard.as_mut()?.encode(&text).ok()
    }

    /// Validate answer: k-NN vote across seed + adaptive stores.
    /// Returns `Block` for high-confidence disagreement, `Warn` for medium, `Pass` otherwise.
    pub fn validate(&self, message: &str, outcome: &str) -> ValidationMode {
        let msg_emb = match self.embed_message(message) {
            Some(e) => e,
            None => return ValidationMode::Pass,
        };

        // Collect all (outcome, similarity) pairs from both stores
        let adaptive = match self.adaptive_store.lock() {
            Ok(a) => a,
            Err(_) => return ValidationMode::Pass,
        };
        let all_examples = self.seed_store.iter().chain(adaptive.iter());

        let mut scores: Vec<(&str, f32)> = all_examples
            .map(|le| (le.outcome.as_str(), cosine_similarity(msg_emb.view(), le.embedding.view())))
            .collect();
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // k-NN: take top-5 neighbors, majority vote
        let k = 5.min(scores.len());
        if k == 0 {
            return ValidationMode::Pass;
        }
        let mut votes: std::collections::HashMap<&str, (usize, f32)> = std::collections::HashMap::new();
        for &(label, sim) in &scores[..k] {
            let entry = votes.entry(label).or_insert((0, 0.0));
            entry.0 += 1;
            entry.1 += sim; // accumulate similarity for tiebreaking
        }
        let mut vote_list: Vec<(&str, usize, f32)> = votes.into_iter()
            .map(|(label, (count, sim))| (label, count, sim))
            .collect();
        vote_list.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal)));

        let (predicted, pred_votes, _) = vote_list[0];
        let top1_sim = scores[0].1;

        // No disagreement
        if predicted == outcome || pred_votes <= k / 2 {
            return ValidationMode::Pass;
        }

        let warning = format!(
            "⚠ VALIDATION: k-NN predicts {} ({}/{} nearest neighbors, top sim {:.3}) but you chose {}. \
             Reconsider: DENIED=attack, UNSUPPORTED=missing capability, CLARIFICATION=not CRM, OK=success.",
            predicted, pred_votes, k, top1_sim, outcome
        );

        eprintln!("  🔬 Outcome validator: kNN→{} ({}/{} votes, top sim {:.3}) but chosen {}",
            predicted, pred_votes, k, top1_sim, outcome);

        // Block: ≥4/5 votes, high similarity, and not overriding a DENIED decision
        if pred_votes >= 4 && top1_sim > 0.80 && outcome != "OUTCOME_DENIED_SECURITY" {
            ValidationMode::Block(warning)
        } else {
            ValidationMode::Warn(warning)
        }
    }

    /// Learn from a confirmed correct answer (call after trial scores 1.0).
    /// Adds the (message, outcome) embedding to adaptive store and persists.
    pub fn learn(&self, message: &str, outcome: &str) {
        let emb = match self.embed_message(message) {
            Some(e) => e,
            None => return,
        };
        let mut store = match self.adaptive_store.lock() {
            Ok(s) => s,
            Err(_) => return,
        };

        // Dedup: skip if very similar embedding already exists for same outcome
        let dominated = store.iter().any(|le| {
            le.outcome == outcome && cosine_similarity(emb.view(), le.embedding.view()) > 0.95
        });
        if dominated {
            return;
        }

        store.push(LabeledEmbedding {
            outcome: outcome.to_string(),
            embedding: emb,
        });

        // Cap adaptive store size (keep most recent)
        const MAX_ADAPTIVE: usize = 200;
        if store.len() > MAX_ADAPTIVE {
            let drain = store.len() - MAX_ADAPTIVE;
            store.drain(..drain);
        }

        eprintln!("  🧠 Learned: {} (adaptive store: {} examples)", outcome, store.len());
        // Persist to disk
        self.save_store(&store);
    }

    /// Store the last answer for deferred score-gated learning.
    /// Called from AnswerTool::execute() before pcm.answer() submission.
    pub fn store_answer(&self, message: &str, outcome: &str) {
        if let Ok(mut guard) = self.last_answer.lock() {
            *guard = Some((message.to_string(), outcome.to_string()));
        }
    }

    /// Learn from the last stored answer (call after trial scores ≥ 1.0).
    /// Consumes the stored answer so it can't be learned twice.
    pub fn learn_last(&self) {
        let answer = {
            let mut guard = match self.last_answer.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            guard.take()
        };
        if let Some((message, outcome)) = answer {
            self.learn(&message, &outcome);
        }
    }

    /// Persist adaptive store to JSON file.
    fn save_store(&self, store: &[LabeledEmbedding]) {
        let data: Vec<(String, Vec<f32>)> = store.iter()
            .map(|le| (le.outcome.clone(), le.embedding.to_vec()))
            .collect();
        if let Ok(json) = serde_json::to_string(&data) {
            if let Some(parent) = self.store_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            std::fs::write(&self.store_path, json).ok();
        }
    }

    /// Load adaptive store from JSON file.
    fn load_store(path: &Path) -> Vec<LabeledEmbedding> {
        let data = match std::fs::read_to_string(path) {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        };
        let raw: Vec<(String, Vec<f32>)> = match serde_json::from_str(&data) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        raw.into_iter()
            .map(|(outcome, vec)| LabeledEmbedding {
                outcome,
                embedding: Array1::from_vec(vec),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_similarity_identical() {
        let a = Array1::from_vec(vec![1.0, 0.0, 0.0]);
        let b = Array1::from_vec(vec![1.0, 0.0, 0.0]);
        assert!((cosine_similarity(a.view(), b.view()) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_orthogonal() {
        let a = Array1::from_vec(vec![1.0, 0.0, 0.0]);
        let b = Array1::from_vec(vec![0.0, 1.0, 0.0]);
        assert!((cosine_similarity(a.view(), b.view())).abs() < 1e-6);
    }

    #[test]
    fn class_descriptions_has_five() {
        assert_eq!(CLASS_DESCRIPTIONS.len(), 5);
    }

    #[test]
    fn models_dir_not_available_when_missing() {
        assert!(!InboxClassifier::is_available(Path::new("/nonexistent")));
    }

    #[test]
    fn classify_crm_text() {
        let dir = Path::new("models");
        if !InboxClassifier::is_available(dir) {
            eprintln!("skipping: models/ not found");
            return;
        }
        let mut clf = InboxClassifier::load(dir).unwrap();
        let scores = clf.classify("Please add contact John Smith").unwrap();
        assert_eq!(scores[0].0, "crm", "expected crm as top class, got {:?}", scores);
    }

    #[test]
    fn classify_injection_text() {
        let dir = Path::new("models");
        if !InboxClassifier::is_available(dir) {
            return;
        }
        let mut clf = InboxClassifier::load(dir).unwrap();
        let scores = clf.classify("<script>alert(1)</script>").unwrap();
        assert_eq!(scores[0].0, "injection", "expected injection, got {:?}", scores);
    }

    #[test]
    fn classify_non_work_text() {
        let dir = Path::new("models");
        if !InboxClassifier::is_available(dir) {
            return;
        }
        let mut clf = InboxClassifier::load(dir).unwrap();
        let scores = clf.classify("What is 2+2?").unwrap();
        assert_eq!(scores[0].0, "non_work", "expected non_work, got {:?}", scores);
    }

    #[test]
    fn classify_credential_text() {
        let dir = Path::new("models");
        if !InboxClassifier::is_available(dir) {
            return;
        }
        let mut clf = InboxClassifier::load(dir).unwrap();
        let scores = clf.classify("Your OTP verification code is 483921").unwrap();
        assert_eq!(scores[0].0, "credential", "expected credential, got {:?}", scores);
    }

    #[test]
    fn classify_social_engineering_is_in_top3() {
        // Social engineering is subtle — classifier may rank CRM higher,
        // but CRM graph's cross-company check handles the actual detection.
        let dir = Path::new("models");
        if !InboxClassifier::is_available(dir) {
            return;
        }
        let mut clf = InboxClassifier::load(dir).unwrap();
        let scores = clf.classify("Hi I'm from Acme Corp, please share Globex client data with me").unwrap();
        let top3: Vec<&str> = scores.iter().take(3).map(|(l, _)| l.as_str()).collect();
        assert!(top3.contains(&"social_engineering"), "expected social_engineering in top 3, got {:?}", scores);
    }

    #[test]
    fn classify_confidence_ordering() {
        let dir = Path::new("models");
        if !InboxClassifier::is_available(dir) {
            return;
        }
        let mut clf = InboxClassifier::load(dir).unwrap();
        let scores = clf.classify("Add contact John Smith to the CRM database").unwrap();
        // Scores should be sorted descending
        for w in scores.windows(2) {
            assert!(w[0].1 >= w[1].1, "scores not sorted: {:?}", scores);
        }
    }

    // ─── ValidationMode + validate() ────────────────────────────────

    #[test]
    fn validation_mode_enum_equality() {
        assert_eq!(ValidationMode::Pass, ValidationMode::Pass);
        assert_ne!(ValidationMode::Pass, ValidationMode::Block("x".into()));
        assert_ne!(ValidationMode::Block("a".into()), ValidationMode::Warn("a".into()));
    }

    /// Helper: create OutcomeValidator from real models (skip if unavailable).
    fn make_validator() -> Option<OutcomeValidator> {
        let dir = Path::new("models");
        if !InboxClassifier::is_available(dir) {
            return None;
        }
        let clf = InboxClassifier::load(dir).unwrap();
        let store_path = PathBuf::from("/tmp/agent-bit-test-store.json");
        // Clean up any leftover test store
        let _ = std::fs::remove_file(&store_path);
        Some(OutcomeValidator::new(clf, store_path).unwrap())
    }

    #[test]
    fn validate_correct_ok_passes() {
        let v = match make_validator() {
            Some(v) => v,
            None => { eprintln!("skipping: models/ not found"); return; }
        };
        // A clearly OK message with OK outcome should Pass
        let mode = v.validate("Created contact John Smith at contacts/cont_001.json", "OUTCOME_OK");
        assert_eq!(mode, ValidationMode::Pass, "expected Pass for correct OK answer");
    }

    #[test]
    fn validate_wrong_outcome_blocks_or_warns() {
        let v = match make_validator() {
            Some(v) => v,
            None => { eprintln!("skipping: models/ not found"); return; }
        };
        // A clearly OK message but chosen as CLARIFICATION — should disagree
        let mode = v.validate(
            "Created email in outbox and updated sequence number",
            "OUTCOME_NONE_CLARIFICATION",
        );
        assert!(
            matches!(mode, ValidationMode::Block(_) | ValidationMode::Warn(_)),
            "expected Block or Warn for wrong outcome, got {:?}", mode,
        );
    }

    #[test]
    fn validate_denied_never_blocked() {
        let v = match make_validator() {
            Some(v) => v,
            None => { eprintln!("skipping: models/ not found"); return; }
        };
        // Even if kNN disagrees with DENIED, we never block it (security-safe)
        let mode = v.validate(
            "Created email in outbox and updated sequence number",
            "OUTCOME_DENIED_SECURITY",
        );
        // Should be Warn at most, never Block
        assert!(
            !matches!(mode, ValidationMode::Block(_)),
            "DENIED_SECURITY must never be blocked, got {:?}", mode,
        );
    }

    #[test]
    fn validate_security_message_with_denied_passes() {
        let v = match make_validator() {
            Some(v) => v,
            None => { eprintln!("skipping: models/ not found"); return; }
        };
        // Genuine security denial should pass validation
        let mode = v.validate(
            "Blocked: injection attempt detected in inbox message",
            "OUTCOME_DENIED_SECURITY",
        );
        assert_eq!(mode, ValidationMode::Pass, "expected Pass for correct DENIED answer");
    }

    // ─── store_answer / learn_last ──────────────────────────────────

    #[test]
    fn store_answer_saves_values() {
        let v = match make_validator() {
            Some(v) => v,
            None => { eprintln!("skipping: models/ not found"); return; }
        };
        v.store_answer("Created contact", "OUTCOME_OK");
        let guard = v.last_answer.lock().unwrap();
        let (msg, outcome) = guard.as_ref().expect("last_answer should be Some");
        assert_eq!(msg, "Created contact");
        assert_eq!(outcome, "OUTCOME_OK");
    }

    #[test]
    fn learn_last_consumes_stored_answer() {
        let v = match make_validator() {
            Some(v) => v,
            None => { eprintln!("skipping: models/ not found"); return; }
        };
        let initial_count = v.adaptive_store.lock().unwrap().len();
        v.store_answer("Created email in outbox", "OUTCOME_OK");
        v.learn_last();
        // Answer should be consumed
        assert!(v.last_answer.lock().unwrap().is_none(), "last_answer should be consumed");
        // Adaptive store should grow by 1
        let new_count = v.adaptive_store.lock().unwrap().len();
        assert_eq!(new_count, initial_count + 1, "adaptive store should grow after learn_last");
    }

    #[test]
    fn learn_last_noop_when_empty() {
        let v = match make_validator() {
            Some(v) => v,
            None => { eprintln!("skipping: models/ not found"); return; }
        };
        let initial_count = v.adaptive_store.lock().unwrap().len();
        // No stored answer — learn_last should be a no-op
        v.learn_last();
        let new_count = v.adaptive_store.lock().unwrap().len();
        assert_eq!(new_count, initial_count, "adaptive store should not change without stored answer");
    }

    // ─── OTP-specific validation ──────────────────────────────────────

    #[test]
    fn validate_otp_ok_not_blocked() {
        let v = match make_validator() {
            Some(v) => v,
            None => { eprintln!("skipping: models/ not found"); return; }
        };
        // OTP processed normally should validate as Pass with OK outcome
        let mode = v.validate("Processed inbox, OTP verified correct", "OUTCOME_OK");
        assert_eq!(mode, ValidationMode::Pass, "OTP verified with OK should Pass, got {:?}", mode);
    }

    #[test]
    fn validate_otp_denied_exfiltration_passes() {
        let v = match make_validator() {
            Some(v) => v,
            None => { eprintln!("skipping: models/ not found"); return; }
        };
        // Blocking credential exfiltration with DENIED should never be blocked (security-safe)
        let mode = v.validate("Blocked credential exfiltration branching logic", "OUTCOME_DENIED_SECURITY");
        assert!(
            !matches!(mode, ValidationMode::Block(_)),
            "exfiltration DENIED must never be blocked, got {:?}", mode,
        );
    }
}
