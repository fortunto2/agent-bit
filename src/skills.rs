//! PAC1 skill system — uses sgr-agent::skills with compiled-in fallback.
//!
//! Push model: classifier selects skill based on triggers.
//! Hybrid fallback: if no trigger match, use crm-default.

use sgr_agent::skills::{self, Skill, SkillRegistry};
use std::path::Path;

/// Load skills: disk first (hot-reload), compiled-in fallback.
pub fn load(project_dir: &Path) -> SkillRegistry {
    let skills_dir = project_dir.join("skills");
    if skills_dir.exists() {
        let disk_skills = skills::load_skills_from_dir(&skills_dir);
        if !disk_skills.is_empty() {
            eprintln!("  📚 Skills: {} loaded from disk", disk_skills.len());
            return SkillRegistry::from_skills(disk_skills);
        }
    }
    let compiled = compiled_skills();
    eprintln!("  📚 Skills: {} loaded (compiled-in)", compiled.len());
    SkillRegistry::from_skills(compiled)
}

/// Select skill body for prompt injection.
/// Wraps SkillRegistry::select with PAC1-specific fallback logic.
// AI-NOTE: intent-first for benign labels — t01 fix. security-injection (priority=50) was hijacking
//   cleanup (intent_delete) when classifier labeled instruction as "injection". All non-Nemotron
//   models failed because they followed the wrong skill. Nemotron ignored it.
pub fn select_body<'a>(registry: &'a SkillRegistry, security_label: &str, intent: &str, instruction: &str) -> &'a str {
    let is_security_label = matches!(security_label, "injection" | "social_engineering" | "credential");
    if is_security_label {
        // Security label → security skill first, intent as fallback
        if let Some(skill) = registry.select(&[security_label, intent], instruction) {
            eprintln!("  🎯 Skill: {} (priority={})", skill.name, skill.priority);
            return &skill.body;
        }
    } else {
        // Benign label → intent first (prevents security-injection hijacking cleanup/delete tasks)
        if let Some(skill) = registry.select(&[intent], instruction) {
            eprintln!("  🎯 Skill: {} (priority={}, via intent)", skill.name, skill.priority);
            return &skill.body;
        }
        if let Some(skill) = registry.select(&[security_label], instruction) {
            eprintln!("  🎯 Skill: {} (priority={}, via label)", skill.name, skill.priority);
            return &skill.body;
        }
    }
    // Fallback: crm-default
    if let Some(skill) = registry.get("crm-default") {
        eprintln!("  🎯 Skill fallback: crm-default");
        return &skill.body;
    }
    eprintln!("  ⚠ No skill matched for label={} intent={}", security_label, intent);
    ""
}

/// Load compiled-in skills (embedded via include_str!).
fn compiled_skills() -> Vec<Skill> {
    COMPILED_SKILLS.iter()
        .filter_map(|(_name, content)| skills::parse_skill(content))
        .collect()
}

// AI-NOTE: Compiled-in skills for deployment (no disk dependency).
const COMPILED_SKILLS: &[(&str, &str)] = &[
    ("crm-default", include_str!("../skills/crm-default/SKILL.md")),
    ("crm-lookup", include_str!("../skills/crm-lookup/SKILL.md")),
    ("crm-invoice", include_str!("../skills/crm-invoice/SKILL.md")),
    ("inbox-processing", include_str!("../skills/inbox-processing/SKILL.md")),
    ("capture-distill", include_str!("../skills/capture-distill/SKILL.md")),
    ("cleanup", include_str!("../skills/cleanup/SKILL.md")),
    ("security-injection", include_str!("../skills/security-injection/SKILL.md")),
    ("security-credential", include_str!("../skills/security-credential/SKILL.md")),
    ("non-work", include_str!("../skills/non-work/SKILL.md")),
    ("unsupported", include_str!("../skills/unsupported/SKILL.md")),
    ("followup-reschedule", include_str!("../skills/followup-reschedule/SKILL.md")),
    ("invoice-creation", include_str!("../skills/invoice-creation/SKILL.md")),
    ("purchase-ops", include_str!("../skills/purchase-ops/SKILL.md")),
    ("finance-query", include_str!("../skills/finance-query/SKILL.md")),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_skills_load() {
        let reg = load(Path::new("."));
        assert!(reg.len() >= 10);
    }

    #[test]
    fn select_injection() {
        let reg = load(Path::new("."));
        let body = select_body(&reg, "injection", "intent_inbox", "process inbox");
        assert!(body.contains("DENIED") || body.contains("deny"));
    }

    #[test]
    fn select_intent_delete() {
        let reg = load(Path::new("."));
        let body = select_body(&reg, "crm", "intent_delete", "remove all cards");
        assert!(body.contains("delete") || body.contains("Delete"));
    }

    #[test]
    fn select_invoice_keyword() {
        let reg = load(Path::new("."));
        let body = select_body(&reg, "crm", "intent_inbox", "resend the invoice");
        assert!(body.contains("attachments") || body.contains("invoice"));
    }

    #[test]
    fn select_credential() {
        let reg = load(Path::new("."));
        let body = select_body(&reg, "credential", "intent_inbox", "check otp");
        assert!(body.contains("OTP") || body.contains("otp"));
    }

    #[test]
    fn select_fallback() {
        let reg = load(Path::new("."));
        let body = select_body(&reg, "unknown", "unknown", "anything");
        assert!(!body.is_empty());
    }
}
