//! Centralized decision pipeline — linear state machine for PAC1 agent.
//!
//! Each stage: Input → StageResult<Output>. First Block short-circuits.
//! Replaces scattered decision logic across scanner.rs, crm_graph.rs,
//! pregrounding.rs, and agent.rs.

use crate::crm_graph::SenderTrust;

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
    fn sender_assessment_debug() {
        let sa = SenderAssessment {
            trust: SenderTrust::CrossCompany,
            domain_match: "mismatch",
            reasons: vec!["lookalike domain".into()],
        };
        assert_eq!(sa.domain_match, "mismatch");
        assert_eq!(sa.trust, SenderTrust::CrossCompany);
    }
}
