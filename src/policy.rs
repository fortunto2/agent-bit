//! File access policy — structural guards for protected paths.
//!
//! Checked in PcmClient before any write/delete RPC.
//! Tools don't need to know about protection — PcmClient enforces.

/// Check if a path is protected from modification (write/delete).
/// Returns Some(reason) if blocked, None if allowed.
pub fn check_write(path: &str) -> Option<&'static str> {
    let norm = path.trim_start_matches('/');
    let basename = norm.rsplit('/').next().unwrap_or(norm).to_lowercase();

    // System files: agent instructions, README schemas
    if basename == "agents.md" || basename == "readme.md" {
        return Some("system file (agent instructions / schema)");
    }

    // Channel policy files (security rules, trust lists) — except otp.txt (ephemeral)
    if norm.to_lowercase().starts_with("docs/channels/")
        && basename.ends_with(".txt")
        && basename != "otp.txt"
    {
        return Some("channel security policy");
    }

    None
}

/// Scan inbox content for references to protected paths.
/// Returns true if content asks to modify/delete protected files.
pub fn content_targets_protected(content: &str) -> bool {
    let lower = content.to_lowercase();
    // Must have destructive intent
    let has_destructive = lower.contains("delete") || lower.contains("remove")
        || lower.contains("overwrite") || lower.contains("replace");
    if !has_destructive {
        return false;
    }
    // Check for protected file references
    lower.contains("agents.md") || lower.contains("readme.md")
        || (lower.contains("docs/channels/") && !lower.contains("otp.txt"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protects_agents_md() {
        assert!(check_write("AGENTS.md").is_some());
        assert!(check_write("/AGENTS.MD").is_some());
        assert!(check_write("docs/AGENTS.md").is_some());
    }

    #[test]
    fn protects_readme() {
        assert!(check_write("README.md").is_some());
        assert!(check_write("accounts/README.md").is_some());
    }

    #[test]
    fn protects_channel_policy() {
        assert!(check_write("docs/channels/Discord.txt").is_some());
        assert!(check_write("docs/channels/Telegram.txt").is_some());
    }

    #[test]
    fn allows_otp_delete() {
        assert!(check_write("docs/channels/otp.txt").is_none());
    }

    #[test]
    fn allows_normal_files() {
        assert!(check_write("contacts/cont_001.json").is_none());
        assert!(check_write("inbox/msg_001.txt").is_none());
        assert!(check_write("outbox/100.json").is_none());
        assert!(check_write("01_notes/acme.md").is_none());
    }
}
