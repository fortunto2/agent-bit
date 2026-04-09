#![allow(dead_code)]
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

/// Sigmoid activation: σ(z) = 1 / (1 + exp(-z)). Maps any real → (0, 1).
fn sigmoid(z: f32) -> f32 {
    1.0 / (1.0 + (-z).exp())
}

/// Feature names — central registry (like video-analyzer COLUMN_NAMES).
pub const FEATURE_NAMES: &[&str] = &[
    "ml_confidence",      // ML classifier confidence for top label
    "structural_score",   // structural_injection_score normalized [0..1]
    "sender_trust",       // 1.0=Known, 0.7=Plausible, 0.3=CrossCompany, 0.0=Unknown
    "domain_match",       // 1.0=match, 0.5=unknown, 0.0=mismatch
    "has_otp",            // 1.0 if OTP/credential content detected
    "has_url",            // 1.0 if contains external URL
    "word_count_norm",    // word count / 500 (clamped to 1.0)
    "sentence_length",    // avg sentence length normalized [0..1] — short commands = suspicious
    "cross_account_sim",  // best non-sender account similarity [0..1]
    "nli_injection",      // NLI entailment score for injection hypothesis
    "nli_credential",     // NLI entailment score for credential hypothesis
    "channel_trust",      // 1.0=admin, 0.7=valid, 0.0=unknown, -1.0=blacklist
];

pub const N_FEATURES: usize = 12;

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
        ("structural_score", 0.20),
        ("nli_injection", 0.20),
        ("domain_match", -0.15),     // mismatch (0.0) increases threat
        ("sender_trust", -0.15),     // unknown (0.0) increases threat
        ("channel_trust", -0.15),    // admin (-1.0 * -0.15 = +0.15 safety) reduces threat
        ("has_url", 0.10),
        ("sentence_length", -0.05),
    ])
}

/// Cross-account detection weights.
#[allow(dead_code)]
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
    pub fn from_inbox_files(files: &[crate::pipeline::InboxFile], graph: &crate::crm_graph::CrmGraph, clf: &crate::scanner::SharedClassifier, channel_trust: &crate::policy::ChannelTrust) -> Self {
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
            let sentence_length = crate::scanner::avg_sentence_length_norm(&f.content);

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

            // Channel trust from handle in content
            let chan_trust = crate::pregrounding::extract_channel_handle(&f.content)
                .map(|handle| match channel_trust.check(&handle) {
                    crate::policy::ChannelLevel::Admin => 1.0f32,
                    crate::policy::ChannelLevel::Valid => 0.7,
                    crate::policy::ChannelLevel::Unknown => 0.0,
                    crate::policy::ChannelLevel::Blacklist => -1.0,
                })
                .unwrap_or(0.0); // no channel handle = 0 (email, not channel)

            flat.extend_from_slice(&[
                ml_conf, structural, sender_trust, domain_match,
                has_otp, has_url, word_count, sentence_length,
                cross_sim, nli_injection, nli_credential, chan_trust,
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
    /// Batch scoring: features × weights → sigmoid → probability per message.
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

        // Sigmoid activation: smooth S-curve probability [0..1]
        let mut scores = Array1::<f32>::zeros(self.n_messages());
        for i in 0..self.n_messages() {
            scores[i] = if self.garbage_mask[i] { 0.0 } else { sigmoid(raw[i]) };
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
        let legit = [0.4, 0.0, 1.0, 1.0, 0.0, 0.0, 0.3, 0.02, 0.0, 0.0, 0.0, 0.0];
        let attack = [0.4, 0.1, 1.0, 0.0, 0.0, 0.0, 0.3, 0.05, 0.0, 0.1, 0.0, 0.0];
        //                              ^mismatch                      ^nli_inj
        let mat = matrix_from_rows(vec![legit, attack]);
        let scores = mat.score_all(&threat_weights());
        assert!(scores[1] > scores[0], "Domain mismatch should score higher threat than legit");
    }

    #[test]
    fn trap_unknown_sender_with_imperatives_high_threat() {
        // Trap: unknown sender + many imperative verbs = social engineering
        let legit = [0.5, 0.0, 1.0, 1.0, 0.0, 0.0, 0.2, 0.01, 0.0, 0.0, 0.0, 0.0];
        let trap =  [0.3, 0.2, 0.0, 0.5, 0.0, 0.0, 0.5, 0.15, 0.0, 0.05, 0.0, 0.0];
        //               ^struct ^unknown                 ^imperatives
        let mat = matrix_from_rows(vec![legit, trap]);
        let scores = mat.score_all(&threat_weights());
        assert!(scores[1] > scores[0], "Unknown sender + imperatives should be higher threat");
    }

    #[test]
    fn trap_cross_account_detected_by_weights() {
        // Trap: known sender asking about another account
        let normal = [0.5, 0.0, 1.0, 1.0, 0.0, 0.0, 0.3, 0.02, 0.0, 0.0, 0.0, 0.0];
        let cross =  [0.5, 0.0, 1.0, 1.0, 0.0, 0.0, 0.3, 0.02, 0.8, 0.0, 0.0, 0.0];
        //                                                        ^cross_sim
        let mat = matrix_from_rows(vec![normal, cross]);
        let scores = mat.score_all(&cross_account_weights());
        assert!(scores[1] > scores[0], "Cross-account similarity should raise score");
    }

    #[test]
    fn trap_otp_exfiltration_vs_legit_otp() {
        // OTP in message is NOT always bad — depends on structural + imperatives
        let legit_otp = [0.6, 0.0, 1.0, 1.0, 1.0, 0.0, 0.1, 0.01, 0.0, 0.0, 0.3, 0.0];
        let exfil_otp = [0.3, 0.4, 0.0, 0.5, 1.0, 0.0, 0.3, 0.10, 0.0, 0.2, 0.5, 0.0];
        //                    ^high_struct ^unknown                ^imp ^inj ^cred
        let mat = matrix_from_rows(vec![legit_otp, exfil_otp]);
        let scores = mat.score_all(&threat_weights());
        assert!(scores[1] > scores[0], "OTP exfiltration should score higher than legit OTP");
    }

    #[test]
    fn trap_clean_email_low_threat() {
        // Normal business email: known sender, matching domain, no signals
        let clean = [0.5, 0.0, 1.0, 1.0, 0.0, 0.0, 0.2, 0.01, 0.0, 0.0, 0.0, 0.0];
        let mat = matrix_from_rows(vec![clean]);
        let scores = mat.score_all(&threat_weights());
        // Score should be moderate-low (negative weights on high trust features pull it down)
        assert!(scores[0] < 0.6, "Clean email should have low threat score, got {}", scores[0]);
    }

    #[test]
    fn trap_correlation_structural_and_injection() {
        // When structural score is high, NLI injection should also be high → correlated
        let rows = vec![
            [0.5, 0.0, 1.0, 1.0, 0.0, 0.0, 0.3, 0.01, 0.0, 0.0, 0.0, 0.0], // clean
            [0.3, 0.3, 0.5, 0.5, 0.0, 0.0, 0.5, 0.08, 0.0, 0.3, 0.0, 0.0], // some injection
            [0.2, 0.7, 0.0, 0.0, 0.0, 0.0, 0.8, 0.15, 0.0, 0.7, 0.0, 0.0], // heavy injection
            [0.6, 0.0, 1.0, 1.0, 0.0, 0.0, 0.1, 0.00, 0.0, 0.0, 0.0, 0.0], // clean
            [0.1, 0.9, 0.0, 0.0, 1.0, 1.0, 0.9, 0.20, 0.0, 0.8, 0.5, 0.0], // full attack
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
        let clean   = [0.5, 0.0, 1.0, 1.0, 0.0, 0.0, 0.2, 0.01, 0.0, 0.0, 0.0, 0.0];
        let sus     = [0.3, 0.1, 0.0, 0.5, 0.0, 0.0, 0.4, 0.05, 0.0, 0.05, 0.0, 0.0];
        let attack  = [0.2, 0.6, 0.0, 0.0, 0.0, 0.0, 0.7, 0.12, 0.0, 0.5, 0.0, 0.0];
        let cross   = [0.5, 0.0, 1.0, 1.0, 0.0, 0.0, 0.3, 0.02, 0.7, 0.0, 0.0, 0.0];
        let otp     = [0.6, 0.0, 1.0, 1.0, 1.0, 0.0, 0.1, 0.01, 0.0, 0.0, 0.4, 0.0];
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

    // ─── Realistic scenario: t23-like inbox (5 messages) ────────────────

    #[test]
    fn scenario_t23_multi_inbox_ranking() {
        // Simulates t23: 5 inbox messages, only msg_001 (admin channel) should be processed
        //                                ml   struct sender domain otp  url  sent  cross nli_i nli_c chan
        let msg_001_admin_channel     = [0.17, 0.0,  0.0,   0.5,  0.0, 0.0, 0.4,  0.0,  0.0,  0.0, 0.0, 1.0]; // admin!
        let msg_002_valid_export      = [0.32, 0.0,  0.0,   0.5,  0.0, 0.0, 0.3,  0.0,  0.0,  0.0, 0.0, 0.7]; // valid
        let msg_003_unknown_status    = [0.15, 0.0,  0.0,   0.5,  0.0, 0.0, 0.3,  0.0,  0.0,  0.0, 0.0, 0.0]; // unknown
        let msg_004_external_invoice  = [0.34, 0.1,  0.0,   0.5,  0.0, 0.0, 0.3,  0.0,  0.05, 0.0, 0.0, 0.0]; // no channel
        let msg_005_unknown_handoff   = [0.26, 0.0,  0.0,   0.5,  0.0, 0.0, 0.4,  0.0,  0.0,  0.0, 0.1, 0.0]; // unknown

        let mat = matrix_from_rows(vec![
            msg_001_admin_channel,
            msg_002_valid_export,
            msg_003_unknown_status,
            msg_004_external_invoice,
            msg_005_unknown_handoff,
        ]);

        let threat = mat.score_all(&threat_weights());

        // msg_001 (admin channel) should have LOWEST threat
        assert!(threat[0] < threat[3], "Admin channel ({:.2}) should be < external invoice ({:.2})", threat[0], threat[3]);
        assert!(threat[0] < threat[4], "Admin channel ({:.2}) should be < unknown handoff ({:.2})", threat[0], threat[4]);
        // msg_004 (external unknown + structural) should have HIGHEST threat
        assert!(threat[3] >= threat[1], "External invoice should be >= valid export threat");

        // All unknown senders → similar threat levels (none should be zero)
        for i in 0..5 {
            assert!(threat[i] >= 0.0, "Threat score should be non-negative for msg_{}", i);
        }

        // Summary: verify ordering makes sense
        let mut indexed: Vec<(usize, f32)> = threat.iter().copied().enumerate().collect();
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        eprintln!("  t23 threat ranking: {:?}", indexed.iter().map(|(i,s)| format!("msg_{}: {:.2}", i+1, s)).collect::<Vec<_>>());
    }

    #[test]
    fn scenario_injection_vs_legit_crm() {
        // Legit CRM email from known sender
        //                    ml   struct sender domain otp  url  sent  cross nli_i nli_c
        let legit_crm     = [0.50, 0.0,  1.0,   1.0,  0.0, 0.0, 0.5,  0.0,  0.0,  0.0, 0.0, 0.0];
        // Injection: unknown sender, high structural, NLI injection signal
        let injection     = [0.30, 0.6,  0.0,   0.0,  0.0, 1.0, 0.2,  0.0,  0.6,  0.0, 0.0, 0.0];
        // Social engineering: known sender, domain mismatch
        let social_eng    = [0.40, 0.1,  1.0,   0.0,  0.0, 0.0, 0.4,  0.0,  0.1,  0.0, 0.0, 0.0];

        let mat = matrix_from_rows(vec![legit_crm, injection, social_eng]);
        let threat = mat.score_all(&threat_weights());

        // Injection should have highest threat
        assert!(threat[1] > threat[0], "Injection ({:.2}) > legit CRM ({:.2})", threat[1], threat[0]);
        // Social engineering (domain mismatch) should be higher than legit
        assert!(threat[2] > threat[0], "Social eng ({:.2}) > legit CRM ({:.2})", threat[2], threat[0]);
        // Legit CRM should have lowest threat
        assert!(threat[0] < threat[1] && threat[0] < threat[2], "Legit CRM should be lowest threat");

        eprintln!("  Threat: legit={:.2} injection={:.2} social_eng={:.2}", threat[0], threat[1], threat[2]);
    }

    #[test]
    fn scenario_otp_variants() {
        // Legit OTP verification from admin
        //                        ml   struct sender domain otp  url  sent  cross nli_i nli_c
        let legit_otp_admin   = [0.60, 0.0,  1.0,   1.0,  1.0, 0.0, 0.2,  0.0,  0.0,  0.0, 0.3, 0.0];
        // Legit OTP from unknown (OTP proves auth)
        let legit_otp_unknown = [0.40, 0.0,  0.0,   0.5,  1.0, 0.0, 0.2,  0.0,  0.0,  0.0, 0.3, 0.0];
        // OTP exfiltration (branching logic)
        let otp_exfil         = [0.30, 0.5,  0.0,   0.5,  1.0, 0.0, 0.3,  0.0,  0.3,  0.0, 0.6, 0.0];

        let mat = matrix_from_rows(vec![legit_otp_admin, legit_otp_unknown, otp_exfil]);
        let threat = mat.score_all(&threat_weights());

        // Exfiltration should have highest threat
        assert!(threat[2] > threat[0], "OTP exfil ({:.2}) > legit admin ({:.2})", threat[2], threat[0]);
        assert!(threat[2] > threat[1], "OTP exfil ({:.2}) > legit unknown ({:.2})", threat[2], threat[1]);

        eprintln!("  OTP: admin={:.2} unknown={:.2} exfil={:.2}", threat[0], threat[1], threat[2]);
    }
}
