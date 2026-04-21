//! Typed instruction intent — replaces bare `String` labels ("intent_inbox" etc).
//!
//! Single source of truth for intent-driven behavioral decisions:
//! - Task-type forcing (delete intent → restrict tool set)
//! - Outbox limit selection
//! - Workflow guard branches
//! - Reasoning tool description selection
//!
//! Wire format (classifier output, JSON schemas, skill triggers, dumps) remains
//! the string form `intent_X`; `Intent::parse` normalizes into the enum.
//!
//! AI-NOTE: `intent_capture` has been fully normalized to `Intent::Inbox` — it was
//! previously a dead string that had to be normalized in two separate fallback sites
//! (pregrounding OpenAI + LLM classifier). `parse` now handles that silently.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Intent {
    Inbox,
    Delete,
    Query,
    Edit,
    Email,
    #[default]
    Unclear,
}

impl Intent {
    /// Parse a classifier label. Unknown → `Unclear`, `intent_capture` → `Inbox`.
    pub fn parse(s: &str) -> Self {
        match s {
            "intent_inbox" => Intent::Inbox,
            "intent_delete" => Intent::Delete,
            "intent_query" => Intent::Query,
            "intent_edit" => Intent::Edit,
            "intent_email" => Intent::Email,
            "intent_capture" => Intent::Inbox,
            _ => Intent::Unclear,
        }
    }

    /// Wire format — used in logs, dumps, evolution.jsonl, think-tool description keys.
    pub fn as_str(&self) -> &'static str {
        match self {
            Intent::Inbox => "intent_inbox",
            Intent::Delete => "intent_delete",
            Intent::Query => "intent_query",
            Intent::Edit => "intent_edit",
            Intent::Email => "intent_email",
            Intent::Unclear => "intent_unclear",
        }
    }

    // ── Behavioral predicates (replace scattered literal compares) ──────

    /// Structural task-type override from ML intent. Only Delete forces a type;
    /// other intents let cached_task_type / LLM reasoning decide.
    /// Replaces agent.rs::detect_forced_task_type.
    pub fn forces_task_type(&self) -> Option<&'static str> {
        match self {
            Intent::Delete => Some("delete"),
            _ => None,
        }
    }

    /// Max outbox emails allowed per inbox task (0 = unlimited). Capture tasks
    /// bypass the limit (capture writes are not outbox). Replaces workflow.rs:90.
    pub fn outbox_limit(&self, is_capture: bool) -> usize {
        if matches!(self, Intent::Inbox) && !is_capture { 2 } else { 0 }
    }

    /// Whether this intent is eligible for the "read-without-write" analysis-paralysis
    /// nudge. Replaces the `matches!(..., "intent_edit" | "intent_inbox")` at workflow.rs:164.
    pub fn allows_multi_write(&self) -> bool {
        matches!(self, Intent::Edit | Intent::Inbox)
    }

    /// Whether the write-nudge during Reading phase applies. Excludes Delete/Query
    /// (they legitimately stay read-only). Replaces workflow.rs:150-151.
    pub fn expects_write_during_reading(&self) -> bool {
        !matches!(self, Intent::Delete | Intent::Query)
    }

    /// Answer-OK guard: whether to block OK with 0 writes when inbox files exist.
    /// Skip for Query / Delete / Unclear — they don't require writes. Replaces workflow.rs:252.
    pub fn answer_ok_requires_write(&self) -> bool {
        !matches!(self, Intent::Query | Intent::Delete | Intent::Unclear)
    }

    /// Query-style intent — triggers the "OK with no content reads" clarification guard.
    /// Replaces workflow.rs:297.
    pub fn is_query(&self) -> bool {
        matches!(self, Intent::Query)
    }

    /// Whether delete transitions straight from Reading → Cleanup (skipping Acting).
    /// Replaces workflow.rs:353.
    pub fn delete_from_reading(&self) -> bool {
        matches!(self, Intent::Delete)
    }

    /// Whether to skip the multi-inbox step-budget scaling. Delete tasks ignore inbox.
    /// Replaces pregrounding.rs:497.
    pub fn skips_inbox_scaling(&self) -> bool {
        matches!(self, Intent::Delete)
    }

    /// Whether low-confidence fallback should run structural reclassification.
    /// Delete-intent bypasses because detect_forced_task_type already handles it.
    /// Replaces pregrounding.rs:115.
    pub fn skips_low_conf_fallback(&self) -> bool {
        matches!(self, Intent::Delete)
    }

    /// Wire labels exposed to LLM classifiers and skill frontmatter.
    /// `intent_capture` is included as a legacy alias that `parse` folds into `Inbox`;
    /// `intent_unclear` is internal-only and excluded from external contracts.
    /// Use this to build the `enum` array in LLM JSON-schemas — adding a new
    /// `Intent` variant then becomes a single-file change.
    pub fn wire_values() -> &'static [&'static str] {
        &[
            "intent_inbox",
            "intent_email",
            "intent_delete",
            "intent_query",
            "intent_edit",
            "intent_capture",
        ]
    }
}

impl fmt::Display for Intent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl serde::Serialize for Intent {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for Intent {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = <&str as serde::Deserialize>::deserialize(d)?;
        Ok(Intent::parse(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_known_labels() {
        assert_eq!(Intent::parse("intent_inbox"), Intent::Inbox);
        assert_eq!(Intent::parse("intent_delete"), Intent::Delete);
        assert_eq!(Intent::parse("intent_query"), Intent::Query);
        assert_eq!(Intent::parse("intent_edit"), Intent::Edit);
        assert_eq!(Intent::parse("intent_email"), Intent::Email);
    }

    #[test]
    fn parse_capture_normalizes_to_inbox() {
        assert_eq!(Intent::parse("intent_capture"), Intent::Inbox);
    }

    #[test]
    fn parse_unknown_becomes_unclear() {
        assert_eq!(Intent::parse(""), Intent::Unclear);
        assert_eq!(Intent::parse("random_label"), Intent::Unclear);
        assert_eq!(Intent::parse("intent_typo"), Intent::Unclear);
    }

    #[test]
    fn as_str_roundtrip() {
        for i in [Intent::Inbox, Intent::Delete, Intent::Query, Intent::Edit, Intent::Email, Intent::Unclear] {
            assert_eq!(Intent::parse(i.as_str()), i, "roundtrip failed for {i}");
        }
    }

    #[test]
    fn forces_task_type_only_delete() {
        assert_eq!(Intent::Delete.forces_task_type(), Some("delete"));
        assert_eq!(Intent::Inbox.forces_task_type(), None);
        assert_eq!(Intent::Query.forces_task_type(), None);
        assert_eq!(Intent::Edit.forces_task_type(), None);
        assert_eq!(Intent::Email.forces_task_type(), None);
        assert_eq!(Intent::Unclear.forces_task_type(), None);
    }

    #[test]
    fn outbox_limit_inbox_non_capture() {
        assert_eq!(Intent::Inbox.outbox_limit(false), 2);
        assert_eq!(Intent::Inbox.outbox_limit(true), 0);
        assert_eq!(Intent::Email.outbox_limit(false), 0);
        assert_eq!(Intent::Delete.outbox_limit(false), 0);
    }

    #[test]
    fn allows_multi_write() {
        assert!(Intent::Edit.allows_multi_write());
        assert!(Intent::Inbox.allows_multi_write());
        assert!(!Intent::Query.allows_multi_write());
        assert!(!Intent::Delete.allows_multi_write());
        assert!(!Intent::Email.allows_multi_write());
    }

    #[test]
    fn answer_ok_requires_write() {
        assert!(Intent::Inbox.answer_ok_requires_write());
        assert!(Intent::Edit.answer_ok_requires_write());
        assert!(Intent::Email.answer_ok_requires_write());
        assert!(!Intent::Query.answer_ok_requires_write());
        assert!(!Intent::Delete.answer_ok_requires_write());
        assert!(!Intent::Unclear.answer_ok_requires_write());
    }

    #[test]
    fn delete_from_reading() {
        assert!(Intent::Delete.delete_from_reading());
        assert!(!Intent::Inbox.delete_from_reading());
    }

    #[test]
    fn skips_inbox_scaling() {
        assert!(Intent::Delete.skips_inbox_scaling());
        assert!(!Intent::Inbox.skips_inbox_scaling());
    }

    #[test]
    fn display_matches_as_str() {
        assert_eq!(Intent::Inbox.to_string(), "intent_inbox");
        assert_eq!(Intent::Delete.to_string(), "intent_delete");
        assert_eq!(Intent::Unclear.to_string(), "intent_unclear");
    }

    #[test]
    fn default_is_unclear() {
        assert_eq!(Intent::default(), Intent::Unclear);
    }

    #[test]
    fn serde_roundtrip() {
        let i = Intent::Inbox;
        let json = serde_json::to_string(&i).unwrap();
        assert_eq!(json, "\"intent_inbox\"");
        let back: Intent = serde_json::from_str(&json).unwrap();
        assert_eq!(back, i);
    }

    #[test]
    fn wire_values_all_parse_to_known_intent() {
        for w in Intent::wire_values() {
            assert_ne!(Intent::parse(w), Intent::Unclear, "wire value {:?} must parse", w);
        }
    }

    #[test]
    fn wire_values_exclude_unclear() {
        assert!(!Intent::wire_values().contains(&"intent_unclear"));
    }
}
