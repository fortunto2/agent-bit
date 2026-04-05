//! Centralized decision pipeline — linear state machine for PAC1 agent.
//!
//! Each stage: Input → StageResult<Output>. First Block short-circuits.
//! Replaces scattered decision logic across scanner.rs, crm_graph.rs,
//! pregrounding.rs, and agent.rs.

use crate::crm_graph::{CrmGraph, SenderTrust};
use crate::scanner;

/// Pipeline stage outcome — Continue to next stage or Block with final answer.
#[derive(Debug, Clone)]
pub enum StageResult<T> {
    /// Pass enriched data to the next stage.
    Continue(T),
    /// Short-circuit: return this outcome immediately, skip remaining stages.
    Block {
        outcome: &'static str,
        message: String,
    },
}

impl<T> StageResult<T> {
    pub fn is_block(&self) -> bool {
        matches!(self, StageResult::Block { .. })
    }
}

/// Unified sender assessment — merges crm_graph::validate_sender + scanner::check_sender_domain_match.
#[derive(Debug, Clone)]
pub struct SenderAssessment {
    /// Trust level from CRM graph (Known/Plausible/CrossCompany/Unknown).
    pub trust: SenderTrust,
    /// Domain match result ("match" / "mismatch" / "unknown").
    pub domain_match: &'static str,
    /// Human-readable reasons for the assessment.
    pub reasons: Vec<String>,
}

/// Combined security assessment for a single piece of content.
#[derive(Debug, Clone)]
pub struct SecurityAssessment {
    /// Overall verdict — Continue (safe to process) or Block (deny immediately).
    pub verdict: StageResult<()>,
    /// ML classifier label (crm, injection, social_engineering, credential, non_work).
    pub ml_label: String,
    /// ML classifier confidence (0.0-1.0).
    pub ml_conf: f32,
    /// Structural injection score (0.0-0.60+).
    pub structural: f32,
    /// Sender assessment (if sender info available).
    pub sender: Option<SenderAssessment>,
}

/// Pipeline context assembled across stages, passed to LLM.
#[derive(Debug, Clone)]
pub struct PipelineContext {
    /// Original instruction text.
    pub instruction: String,
    /// ML intent label (intent_delete, intent_edit, intent_query, etc.).
    pub intent: String,
    /// Security class label for instruction (crm, injection, etc.).
    pub instruction_label: String,
    /// Per-file security assessments for inbox content.
    pub inbox_assessments: Vec<(String, SecurityAssessment)>,
}

/// Unified sender assessment — single source of truth.
/// Merges crm_graph::validate_sender() (email/graph lookup) with
/// scanner::check_sender_domain_match() (content-based domain matching).
pub fn assess_sender(
    sender_email: Option<&str>,
    content: &str,
    graph: Option<&CrmGraph>,
    account_domains: &[(String, String)],
) -> SenderAssessment {
    let mut reasons = Vec::new();

    // No sender email → Unknown
    let email = match sender_email {
        Some(e) if !e.is_empty() => e,
        _ => return SenderAssessment { trust: SenderTrust::Unknown, domain_match: "unknown", reasons: vec!["no sender email".into()] },
    };

    // Phase 1: CRM graph lookup (email → Known, domain → Plausible, cross-company, lookalike)
    let trust = if let Some(g) = graph {
        let company_ref = scanner::extract_company_ref(content);
        let t = g.validate_sender(email, company_ref.as_deref());
        reasons.push(format!("CRM graph: {}", t));
        t
    } else {
        SenderTrust::Unknown
    };

    // Phase 2: Content-based domain matching (sender domain vs account domains in content)
    let sender_domain = email.split('@').nth(1).unwrap_or("");
    let domain_match = if sender_domain.is_empty() {
        "unknown"
    } else {
        let dm = scanner::check_sender_domain_match(sender_domain, content, account_domains);
        if dm != "unknown" {
            reasons.push(format!("domain match: {}", dm));
        }
        dm
    };

    // Reconcile: if graph says Known but domain says mismatch → trust graph (email is authoritative)
    // If graph says Unknown but domain says mismatch → upgrade to CrossCompany
    let final_trust = match (&trust, domain_match) {
        (SenderTrust::Known, _) => SenderTrust::Known, // email in CRM is definitive
        (SenderTrust::CrossCompany, _) => SenderTrust::CrossCompany, // graph detected cross-company
        (_, "mismatch") => {
            reasons.push("domain mismatch → CrossCompany".into());
            SenderTrust::CrossCompany
        }
        _ => trust,
    };

    SenderAssessment { trust: final_trust, domain_match, reasons }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_result_continue() {
        let r: StageResult<i32> = StageResult::Continue(42);
        assert!(!r.is_block());
    }

    #[test]
    fn stage_result_block() {
        let r: StageResult<()> = StageResult::Block {
            outcome: "OUTCOME_DENIED_SECURITY",
            message: "blocked".into(),
        };
        assert!(r.is_block());
    }

    #[test]
    fn assess_sender_no_email() {
        let sa = assess_sender(None, "content", None, &[]);
        assert_eq!(sa.trust, SenderTrust::Unknown);
        assert_eq!(sa.domain_match, "unknown");
    }

    #[test]
    fn assess_sender_unknown_domain_no_graph() {
        let sa = assess_sender(Some("user@random.com"), "some content", None, &[]);
        assert_eq!(sa.trust, SenderTrust::Unknown);
    }

    #[test]
    fn assess_sender_mismatch_upgrades_to_cross_company() {
        // Domain mismatch without CRM graph → CrossCompany
        let accounts = vec![
            ("Silverline Retail".to_string(), "silverline.nl".to_string()),
        ];
        let content = "From: lena@silverline-retail.biz\nSubject: Invoice\n\nResend invoice for Silverline Retail";
        let sa = assess_sender(Some("lena@silverline-retail.biz"), content, None, &accounts);
        assert_eq!(sa.trust, SenderTrust::CrossCompany);
        assert_eq!(sa.domain_match, "mismatch");
    }
}
