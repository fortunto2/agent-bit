//! Feature matrix for inbox classification — inspired by video-analyzer FeatureMatrix.
//!
//! Each inbox message = row of features. Enables:
//! - Batch scoring: features × weights → threat/action score
//! - Correlation analysis: which features co-occur
//! - Z-score normalization: per-feature standardization
//!
//! Feature vector per message:
//! [ml_conf, structural, sender_trust, domain_match, has_otp, has_url,
//!  word_count, imperative_ratio, question_marks, at_signs,
//!  cross_account_sim, nli_injection, nli_credential]

use ndarray::{Array1, Array2, Axis};

/// Feature names — central registry (like video-analyzer COLUMN_NAMES).
pub const FEATURE_NAMES: &[&str] = &[
    "ml_confidence",      // ML classifier confidence for top label
    "structural_score",   // structural_injection_score normalized [0..1]
    "sender_trust",       // 1.0=Known, 0.7=Plausible, 0.3=CrossCompany, 0.0=Unknown
    "domain_match",       // 1.0=match, 0.5=unknown, 0.0=mismatch
    "has_otp",            // 1.0 if OTP/credential content detected
    "has_url",            // 1.0 if contains external URL
    "word_count_norm",    // word count / 500 (clamped to 1.0)
    "imperative_ratio",   // fraction of imperative verbs in text
    "cross_account_sim",  // best non-sender account similarity [0..1]
    "nli_injection",      // NLI entailment score for injection hypothesis
    "nli_credential",     // NLI entailment score for credential hypothesis
];

pub const N_FEATURES: usize = 11;

/// Feature matrix: (n_messages, N_FEATURES).
pub struct InboxFeatureMatrix {
    pub data: Array2<f32>,
    pub labels: Vec<String>,        // ML label per message
    pub paths: Vec<String>,         // file path per message
    pub garbage_mask: Vec<bool>,    // true = blocked by pipeline
}

/// Scoring weights for a specific task.
pub struct Weights {
    pub values: [f32; N_FEATURES],
    pub bias: f32,
    pub normalize: bool,
}

impl Weights {
    /// Create weights from named pairs (like video-analyzer).
    pub fn from_named(pairs: &[(&str, f32)]) -> Self {
        let mut values = [0.0f32; N_FEATURES];
        for &(name, weight) in pairs {
            if let Some(idx) = column_index(name) {
                values[idx] = weight;
            }
        }
        let sum: f32 = values.iter().sum();
        let bias = 0.5 * (1.0 - sum);
        Self { values, bias, normalize: false }
    }

    pub fn normalized(self) -> Self {
        Self { normalize: true, ..self }
    }
}

/// Threat detection weights — high score = likely attack.
pub fn threat_weights() -> Weights {
    Weights::from_named(&[
        ("structural_score", 0.25),
        ("nli_injection", 0.20),
        ("domain_match", -0.20),  // mismatch (0.0) increases threat
        ("sender_trust", -0.15),  // unknown (0.0) increases threat
        ("has_url", 0.10),
        ("imperative_ratio", 0.10),
    ])
}

/// Cross-account detection weights.
pub fn cross_account_weights() -> Weights {
    Weights::from_named(&[
        ("cross_account_sim", 0.50),
        ("sender_trust", 0.20),   // KNOWN sender + cross-account = suspicious
        ("domain_match", 0.15),
        ("ml_confidence", 0.10),
    ])
}

/// Column index by name.
pub fn column_index(name: &str) -> Option<usize> {
    FEATURE_NAMES.iter().position(|&n| n == name)
}

impl InboxFeatureMatrix {
    /// Build from pipeline SecurityAssessment data.
    pub fn from_inbox_files(files: &[crate::pipeline::InboxFile], graph: &crate::crm_graph::CrmGraph, clf: &crate::scanner::SharedClassifier) -> Self {
        let n = files.len();
        let mut flat = Vec::with_capacity(n * N_FEATURES);
        let mut labels = Vec::with_capacity(n);
        let mut paths = Vec::with_capacity(n);
        let mut garbage_mask = Vec::with_capacity(n);

        for f in files {
            let sec = &f.security;
            let sender = sec.sender.as_ref();

            let ml_conf = sec.ml_conf;
            let structural = (sec.structural as f32 / 10.0).min(1.0);
            let sender_trust = match sender.map(|s| &s.trust) {
                Some(crate::crm_graph::SenderTrust::Known) => 1.0,
                Some(crate::crm_graph::SenderTrust::Plausible) => 0.7,
                Some(crate::crm_graph::SenderTrust::CrossCompany) => 0.3,
                _ => 0.0,
            };
            let domain_match = match sender.map(|s| s.domain_match.as_ref()) {
                Some("match") => 1.0,
                Some("mismatch") => 0.0,
                _ => 0.5,
            };
            let lower = f.content.to_lowercase();
            let has_otp = if lower.contains("otp") || lower.contains("verification code") { 1.0 } else { 0.0 };
            let has_url = if lower.contains("http://") || lower.contains("https://") { 1.0 } else { 0.0 };
            let word_count = (f.content.split_whitespace().count() as f32 / 500.0).min(1.0);
            let imperative_ratio = crate::scanner::imperative_ratio(&f.content);

            // Cross-account similarity (from pre-computed graph embeddings)
            let cross_sim = if sender_trust > 0.5 {
                let sender_email = crate::scanner::extract_sender_email(&f.content);
                let sender_account = sender_email.as_deref()
                    .and_then(|e| graph.account_for_email(e));
                if let Some(ref acct) = sender_account {
                    graph.detect_cross_account(&f.content, acct, clf)
                        .map(|(_, sim)| sim as f32)
                        .unwrap_or(0.0)
                } else { 0.0 }
            } else { 0.0 };

            // NLI scores (from pipeline — already computed)
            let nli_injection = sec.nli_scores.as_ref()
                .and_then(|s| s.iter().find(|(l,_)| l == "injection").map(|(_,v)| *v))
                .unwrap_or(0.0);
            let nli_credential = sec.nli_scores.as_ref()
                .and_then(|s| s.iter().find(|(l,_)| l == "credential").map(|(_,v)| *v))
                .unwrap_or(0.0);

            flat.extend_from_slice(&[
                ml_conf, structural, sender_trust, domain_match,
                has_otp, has_url, word_count, imperative_ratio,
                cross_sim, nli_injection, nli_credential,
            ]);

            labels.push(sec.ml_label.clone());
            paths.push(f.path.clone());
            garbage_mask.push(sec.blocked.is_some());
        }

        let data = Array2::from_shape_vec((n, N_FEATURES), flat)
            .expect("feature matrix shape mismatch");

        Self { data, labels, paths, garbage_mask }
    }

    pub fn n_messages(&self) -> usize {
        self.data.nrows()
    }

    /// Get a single feature column by name.
    pub fn column(&self, name: &str) -> Option<ndarray::ArrayView1<'_, f32>> {
        column_index(name).map(|idx| self.data.column(idx))
    }

    /// Non-garbage rows only.
    pub fn usable(&self) -> Array2<f32> {
        let indices: Vec<usize> = self.garbage_mask.iter()
            .enumerate()
            .filter(|&(_, &g)| !g)
            .map(|(i, _)| i)
            .collect();
        if indices.is_empty() {
            return Array2::zeros((0, N_FEATURES));
        }
        let mut out = Array2::zeros((indices.len(), N_FEATURES));
        for (dst, &src) in indices.iter().enumerate() {
            out.row_mut(dst).assign(&self.data.row(src));
        }
        out
    }

    /// Per-feature mean and std (usable rows only).
    pub fn mean_std(&self) -> (Array1<f32>, Array1<f32>) {
        let usable = self.usable();
        let n = usable.nrows();
        if n < 2 {
            return (Array1::zeros(N_FEATURES), Array1::zeros(N_FEATURES));
        }
        let means = usable.mean_axis(Axis(0)).unwrap();
        let centered = &usable - &means;
        let variance = centered.mapv(|x| x * x).mean_axis(Axis(0)).unwrap();
        let stds = variance.mapv(f32::sqrt);
        (means, stds)
    }

    /// Batch scoring: features × weights → score per message.
    pub fn score_all(&self, weights: &Weights) -> Array1<f32> {
        let w = Array1::from(weights.values.to_vec());
        let raw = if weights.normalize {
            let (means, stds) = self.mean_std();
            let safe_stds = stds.mapv(|s| if s > 1e-8 { s } else { f32::INFINITY });
            let z = (&self.data - &means) / &safe_stds;
            z.dot(&w) + weights.bias
        } else {
            self.data.dot(&w) + weights.bias
        };

        // Apply garbage mask
        let mut scores = Array1::<f32>::zeros(self.n_messages());
        for i in 0..self.n_messages() {
            scores[i] = if self.garbage_mask[i] { 0.0 } else { raw[i].clamp(0.0, 1.0) };
        }
        scores
    }

    /// Correlation matrix (N_FEATURES × N_FEATURES).
    pub fn correlation_matrix(&self) -> Array2<f32> {
        let usable = self.usable();
        let n = usable.nrows();
        if n < 2 {
            return Array2::zeros((N_FEATURES, N_FEATURES));
        }
        let means = usable.mean_axis(Axis(0)).unwrap();
        let mut centered = usable.clone();
        for mut row in centered.rows_mut() {
            row -= &means;
        }
        let cov = centered.t().dot(&centered) * (1.0 / (n - 1) as f32);
        let stds: Array1<f32> = cov.diag().mapv(|v| v.max(0.0).sqrt());
        let mut corr = cov;
        for i in 0..N_FEATURES {
            for j in 0..N_FEATURES {
                if stds[i] < 1e-10 || stds[j] < 1e-10 {
                    corr[[i, j]] = if i == j { 1.0 } else { 0.0 };
                } else {
                    corr[[i, j]] /= stds[i] * stds[j];
                }
            }
        }
        corr
    }

    /// Feature summary: name, mean, std, min, max per feature.
    pub fn summary(&self) -> Vec<(&'static str, f32, f32, f32, f32)> {
        let usable = self.usable();
        let n = usable.nrows();
        FEATURE_NAMES.iter().enumerate().map(|(j, &name)| {
            if n == 0 { return (name, 0.0, 0.0, 0.0, 0.0); }
            let col = usable.column(j);
            let mean = col.mean().unwrap_or(0.0);
            let var: f32 = col.iter().map(|&x| (x - mean).powi(2)).sum::<f32>() / n.max(1) as f32;
            let min = col.iter().cloned().fold(f32::INFINITY, f32::min);
            let max = col.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            (name, mean, var.sqrt(), min, max)
        }).collect()
    }

    /// Log feature matrix summary to stderr.
    pub fn log_summary(&self) {
        if self.n_messages() == 0 { return; }
        eprintln!("  📊 Feature matrix: {} messages × {} features", self.n_messages(), N_FEATURES);
        for (name, mean, std, min, max) in self.summary() {
            if std > 0.01 { // only log non-constant features
                eprintln!("    {}: mean={:.2} std={:.2} [{:.2}..{:.2}]", name, mean, std, min, max);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_from_named() {
        let w = Weights::from_named(&[("ml_confidence", 0.5), ("structural_score", 0.3)]);
        assert_eq!(w.values[0], 0.5); // ml_confidence = index 0
        assert_eq!(w.values[1], 0.3); // structural_score = index 1
    }

    #[test]
    fn score_all_basic() {
        let data = Array2::from_shape_vec((2, N_FEATURES), vec![0.5; 2 * N_FEATURES]).unwrap();
        let mat = InboxFeatureMatrix {
            data,
            labels: vec!["crm".into(); 2],
            paths: vec!["a".into(), "b".into()],
            garbage_mask: vec![false, false],
        };
        let w = Weights::from_named(&[("ml_confidence", 1.0)]);
        let scores = mat.score_all(&w);
        assert!(scores[0] > 0.0);
        assert!((scores[0] - scores[1]).abs() < 0.01);
    }

    #[test]
    fn garbage_excluded() {
        let data = Array2::from_shape_vec((2, N_FEATURES), vec![0.8; 2 * N_FEATURES]).unwrap();
        let mat = InboxFeatureMatrix {
            data,
            labels: vec!["crm".into(); 2],
            paths: vec!["a".into(), "b".into()],
            garbage_mask: vec![true, false],
        };
        let w = Weights::from_named(&[("ml_confidence", 1.0)]);
        let scores = mat.score_all(&w);
        assert_eq!(scores[0], 0.0, "Garbage message should score 0");
        assert!(scores[1] > 0.0, "Clean message should score > 0");
    }

    #[test]
    fn correlation_identity_for_constant() {
        let data = Array2::from_shape_vec((3, N_FEATURES), vec![0.5; 3 * N_FEATURES]).unwrap();
        let mat = InboxFeatureMatrix {
            data,
            labels: vec!["crm".into(); 3],
            paths: vec!["a".into(), "b".into(), "c".into()],
            garbage_mask: vec![false; 3],
        };
        let corr = mat.correlation_matrix();
        // All features constant → correlation undefined (set to 1.0 on diagonal, 0.0 off)
        assert_eq!(corr[[0, 0]], 1.0);
        assert_eq!(corr[[0, 1]], 0.0);
    }

    #[test]
    fn column_by_name() {
        let data = Array2::zeros((2, N_FEATURES));
        let mat = InboxFeatureMatrix {
            data,
            labels: vec!["crm".into(); 2],
            paths: vec!["a".into(), "b".into()],
            garbage_mask: vec![false; 2],
        };
        assert!(mat.column("ml_confidence").is_some());
        assert!(mat.column("nonexistent").is_none());
    }

    #[test]
    fn feature_count() {
        assert_eq!(FEATURE_NAMES.len(), N_FEATURES);
    }

    // ─── Trap tests: adversarial scenarios ──────────────────────────────

    /// Helper: build matrix from raw feature rows.
    fn matrix_from_rows(rows: Vec<[f32; N_FEATURES]>) -> InboxFeatureMatrix {
        let n = rows.len();
        let flat: Vec<f32> = rows.iter().flat_map(|r| r.iter().copied()).collect();
        InboxFeatureMatrix {
            data: Array2::from_shape_vec((n, N_FEATURES), flat).unwrap(),
            labels: vec!["crm".into(); n],
            paths: (0..n).map(|i| format!("inbox/msg_{}.txt", i)).collect(),
            garbage_mask: vec![false; n],
        }
    }

    #[test]
    fn trap_domain_mismatch_scores_high_threat() {
        // Trap: KNOWN sender but domain MISMATCH (social engineering)
        let legit = [0.4, 0.0, 1.0, 1.0, 0.0, 0.0, 0.3, 0.02, 0.0, 0.0, 0.0];
        let attack = [0.4, 0.1, 1.0, 0.0, 0.0, 0.0, 0.3, 0.05, 0.0, 0.1, 0.0];
        //                              ^mismatch                      ^nli_inj
        let mat = matrix_from_rows(vec![legit, attack]);
        let scores = mat.score_all(&threat_weights());
        assert!(scores[1] > scores[0], "Domain mismatch should score higher threat than legit");
    }

    #[test]
    fn trap_unknown_sender_with_imperatives_high_threat() {
        // Trap: unknown sender + many imperative verbs = social engineering
        let legit = [0.5, 0.0, 1.0, 1.0, 0.0, 0.0, 0.2, 0.01, 0.0, 0.0, 0.0];
        let trap =  [0.3, 0.2, 0.0, 0.5, 0.0, 0.0, 0.5, 0.15, 0.0, 0.05, 0.0];
        //               ^struct ^unknown                 ^imperatives
        let mat = matrix_from_rows(vec![legit, trap]);
        let scores = mat.score_all(&threat_weights());
        assert!(scores[1] > scores[0], "Unknown sender + imperatives should be higher threat");
    }

    #[test]
    fn trap_cross_account_detected_by_weights() {
        // Trap: known sender asking about another account
        let normal = [0.5, 0.0, 1.0, 1.0, 0.0, 0.0, 0.3, 0.02, 0.0, 0.0, 0.0];
        let cross =  [0.5, 0.0, 1.0, 1.0, 0.0, 0.0, 0.3, 0.02, 0.8, 0.0, 0.0];
        //                                                        ^cross_sim
        let mat = matrix_from_rows(vec![normal, cross]);
        let scores = mat.score_all(&cross_account_weights());
        assert!(scores[1] > scores[0], "Cross-account similarity should raise score");
    }

    #[test]
    fn trap_otp_exfiltration_vs_legit_otp() {
        // OTP in message is NOT always bad — depends on structural + imperatives
        let legit_otp = [0.6, 0.0, 1.0, 1.0, 1.0, 0.0, 0.1, 0.01, 0.0, 0.0, 0.3];
        let exfil_otp = [0.3, 0.4, 0.0, 0.5, 1.0, 0.0, 0.3, 0.10, 0.0, 0.2, 0.5];
        //                    ^high_struct ^unknown                ^imp ^inj ^cred
        let mat = matrix_from_rows(vec![legit_otp, exfil_otp]);
        let scores = mat.score_all(&threat_weights());
        assert!(scores[1] > scores[0], "OTP exfiltration should score higher than legit OTP");
    }

    #[test]
    fn trap_clean_email_low_threat() {
        // Normal business email: known sender, matching domain, no signals
        let clean = [0.5, 0.0, 1.0, 1.0, 0.0, 0.0, 0.2, 0.01, 0.0, 0.0, 0.0];
        let mat = matrix_from_rows(vec![clean]);
        let scores = mat.score_all(&threat_weights());
        // Score should be moderate-low (negative weights on high trust features pull it down)
        assert!(scores[0] < 0.6, "Clean email should have low threat score, got {}", scores[0]);
    }

    #[test]
    fn trap_correlation_structural_and_injection() {
        // When structural score is high, NLI injection should also be high → correlated
        let rows = vec![
            [0.5, 0.0, 1.0, 1.0, 0.0, 0.0, 0.3, 0.01, 0.0, 0.0, 0.0], // clean
            [0.3, 0.3, 0.5, 0.5, 0.0, 0.0, 0.5, 0.08, 0.0, 0.3, 0.0], // some injection
            [0.2, 0.7, 0.0, 0.0, 0.0, 0.0, 0.8, 0.15, 0.0, 0.7, 0.0], // heavy injection
            [0.6, 0.0, 1.0, 1.0, 0.0, 0.0, 0.1, 0.00, 0.0, 0.0, 0.0], // clean
            [0.1, 0.9, 0.0, 0.0, 1.0, 1.0, 0.9, 0.20, 0.0, 0.8, 0.5], // full attack
        ];
        let mat = matrix_from_rows(rows);
        let corr = mat.correlation_matrix();
        let struct_idx = column_index("structural_score").unwrap();
        let nli_inj_idx = column_index("nli_injection").unwrap();
        assert!(corr[[struct_idx, nli_inj_idx]] > 0.5,
            "structural_score and nli_injection should be positively correlated, got {}",
            corr[[struct_idx, nli_inj_idx]]);
    }

    #[test]
    fn trap_multi_message_ranking() {
        // 5 messages: 1 clean, 1 suspicious, 1 attack, 1 cross-account, 1 OTP
        let clean   = [0.5, 0.0, 1.0, 1.0, 0.0, 0.0, 0.2, 0.01, 0.0, 0.0, 0.0];
        let sus     = [0.3, 0.1, 0.0, 0.5, 0.0, 0.0, 0.4, 0.05, 0.0, 0.05, 0.0];
        let attack  = [0.2, 0.6, 0.0, 0.0, 0.0, 0.0, 0.7, 0.12, 0.0, 0.5, 0.0];
        let cross   = [0.5, 0.0, 1.0, 1.0, 0.0, 0.0, 0.3, 0.02, 0.7, 0.0, 0.0];
        let otp     = [0.6, 0.0, 1.0, 1.0, 1.0, 0.0, 0.1, 0.01, 0.0, 0.0, 0.4];
        let mat = matrix_from_rows(vec![clean, sus, attack, cross, otp]);

        let threat = mat.score_all(&threat_weights());
        // Attack should rank highest threat
        assert!(threat[2] > threat[0], "Attack > clean");
        assert!(threat[2] > threat[1], "Attack > suspicious");

        let cross_scores = mat.score_all(&cross_account_weights());
        // Cross-account message should rank highest
        assert!(cross_scores[3] > cross_scores[0], "Cross > clean");
        assert!(cross_scores[3] > cross_scores[2], "Cross > attack (for cross weights)");
    }
}
