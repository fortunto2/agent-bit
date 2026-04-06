use std::collections::HashMap;

use petgraph::graph::{Graph, NodeIndex};

use crate::pcm::PcmClient;

// ─── Node & Edge types ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Node {
    Contact { name: String, email: Option<String> },
    Account { name: String },
    Domain { name: String },
}

#[derive(Debug, Clone)]
pub enum Edge {
    WorksAt,
    HasDomain,
    KnownEmail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SenderTrust {
    Known,
    Plausible,
    CrossCompany,
    Unknown,
}

impl std::fmt::Display for SenderTrust {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SenderTrust::Known => write!(f, "KNOWN"),
            SenderTrust::Plausible => write!(f, "PLAUSIBLE"),
            SenderTrust::CrossCompany => write!(f, "CROSS_COMPANY"),
            SenderTrust::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

// ─── CRM Knowledge Graph ────────────────────────────────────────────────────

pub struct CrmGraph {
    graph: Graph<Node, Edge>,
    /// email (lowercase) → NodeIndex of Contact
    email_index: HashMap<String, NodeIndex>,
    /// domain (lowercase) → NodeIndex of Domain
    domain_index: HashMap<String, NodeIndex>,
    /// name (lowercase) → NodeIndex of Contact or Account
    name_index: HashMap<String, NodeIndex>,
    /// account ID → account name (for resolving contact.account_id)
    account_id_map: HashMap<String, String>,
}

impl CrmGraph {
    pub fn new() -> Self {
        Self {
            graph: Graph::new(),
            email_index: HashMap::new(),
            domain_index: HashMap::new(),
            name_index: HashMap::new(),
            account_id_map: HashMap::new(),
        }
    }

    /// Empty graph for testing.
    pub fn empty() -> Self { Self::new() }

    /// Build graph from PCM filesystem — reads contacts/ and accounts/ directories.
    pub async fn build_from_pcm(pcm: &PcmClient) -> Self {
        let mut g = Self::new();

        // Read accounts FIRST (builds account_id_map for contact.account_id resolution)
        match pcm.list("accounts").await {
            Ok(listing) => {
                for line in listing.lines() {
                    let name = line.trim().trim_end_matches('/');
                    if name.is_empty() || name.starts_with('$') || name.eq_ignore_ascii_case("README.MD") {
                        continue;
                    }
                    let path = format!("accounts/{}", name);
                    if let Ok(content) = pcm.read(&path, false, 0, 0).await {
                        g.ingest_account(&content);
                    }
                }
            }
            Err(e) => eprintln!("  CRM graph: accounts/ list failed: {}", e),
        }

        // Read contacts (account_id_map available for resolving account_id → name)
        match pcm.list("contacts").await {
            Ok(listing) => {
                for line in listing.lines() {
                    let name = line.trim().trim_end_matches('/');
                    if name.is_empty() || name.starts_with('$') || name.eq_ignore_ascii_case("README.MD") {
                        continue;
                    }
                    let path = format!("contacts/{}", name);
                    if let Ok(content) = pcm.read(&path, false, 0, 0).await {
                        g.ingest_contact(&content);
                    }
                }
            }
            Err(e) => eprintln!("  CRM graph: contacts/ list failed: {}", e),
        }

        g
    }

    /// Strip PCM shell header ("$ cat ...\n") from read output.
    fn strip_pcm_header(content: &str) -> &str {
        if content.starts_with("$ ") {
            content.find('\n').map(|i| &content[i + 1..]).unwrap_or(content)
        } else {
            content
        }
    }

    /// Parse a contact file (JSON or markdown) and add to graph.
    fn ingest_contact(&mut self, raw: &str) {
        let content = Self::strip_pcm_header(raw);
        // Try JSON first
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(content) {
            let name = v.get("name").or(v.get("Name")).or(v.get("full_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let email = v.get("email").or(v.get("Email"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let company_raw = v.get("company").or(v.get("Company")).or(v.get("account")).or(v.get("account_id"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            // Resolve account_id to account name if possible
            let company = company_raw.map(|c| {
                self.account_id_map.get(&c).cloned().unwrap_or(c)
            });
            self.add_contact(&name, email.as_deref(), company.as_deref());
            return;
        }

        // Fallback: parse markdown/text key-value pairs
        let mut name = String::new();
        let mut email = None;
        let mut company = None;
        for line in content.lines() {
            let lower = line.to_lowercase();
            if lower.starts_with("# ") {
                name = line.strip_prefix("# ").unwrap_or("").trim().to_string();
            } else if lower.starts_with("name:") {
                name = line.splitn(2, ':').last().unwrap_or("").trim().to_string();
            } else if lower.starts_with("email:") || lower.starts_with("e-mail:") {
                email = line.splitn(2, ':').last().map(|s| s.trim().to_string());
            } else if lower.starts_with("company:") || lower.starts_with("account:") || lower.starts_with("organization:") {
                company = line.splitn(2, ':').last().map(|s| s.trim().to_string());
            }
        }
        if !name.is_empty() {
            self.add_contact(&name, email.as_deref(), company.as_deref());
        }
    }

    /// Parse an account file and add to graph.
    fn ingest_account(&mut self, raw: &str) {
        let content = Self::strip_pcm_header(raw);
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(content) {
            let name = v.get("name").or(v.get("Name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let domain = v.get("domain").or(v.get("Domain")).or(v.get("website"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            // Record ID → name mapping for contact.account_id resolution
            if let Some(id) = v.get("id").and_then(|v| v.as_str()) {
                if !name.is_empty() {
                    self.account_id_map.insert(id.to_string(), name.clone());
                }
            }
            if !name.is_empty() {
                self.add_account(&name, domain.as_deref());
            }
            return;
        }

        let mut name = String::new();
        let mut domain = None;
        for line in content.lines() {
            let lower = line.to_lowercase();
            if lower.starts_with("# ") {
                name = line.strip_prefix("# ").unwrap_or("").trim().to_string();
            } else if lower.starts_with("name:") {
                name = line.splitn(2, ':').last().unwrap_or("").trim().to_string();
            } else if lower.starts_with("domain:") || lower.starts_with("website:") {
                domain = line.splitn(2, ':').last().map(|s| s.trim().to_string());
            }
        }
        if !name.is_empty() {
            self.add_account(&name, domain.as_deref());
        }
    }

    pub fn add_contact(&mut self, name: &str, email: Option<&str>, company: Option<&str>) {
        let contact_idx = self.graph.add_node(Node::Contact {
            name: name.to_string(),
            email: email.map(|s| s.to_string()),
        });
        self.name_index.insert(name.to_lowercase(), contact_idx);

        if let Some(email) = email {
            let email_lower = email.to_lowercase();
            self.email_index.insert(email_lower.clone(), contact_idx);

            // Extract domain and link
            if let Some(at) = email_lower.find('@') {
                let domain = &email_lower[at + 1..];
                let domain_idx = self.get_or_create_domain(domain);
                self.graph.add_edge(contact_idx, domain_idx, Edge::KnownEmail);
            }
        }

        if let Some(company) = company {
            // Link to existing account or create placeholder
            let account_idx = if let Some(&idx) = self.name_index.get(&company.to_lowercase()) {
                idx
            } else {
                let idx = self.graph.add_node(Node::Account { name: company.to_string() });
                self.name_index.insert(company.to_lowercase(), idx);
                idx
            };
            self.graph.add_edge(contact_idx, account_idx, Edge::WorksAt);
        }
    }

    pub fn add_account(&mut self, name: &str, domain: Option<&str>) {
        let account_idx = if let Some(&idx) = self.name_index.get(&name.to_lowercase()) {
            idx
        } else {
            let idx = self.graph.add_node(Node::Account { name: name.to_string() });
            self.name_index.insert(name.to_lowercase(), idx);
            idx
        };

        if let Some(domain) = domain {
            let clean = domain.trim_start_matches("http://")
                .trim_start_matches("https://")
                .trim_start_matches("www.")
                .trim_end_matches('/');
            let domain_idx = self.get_or_create_domain(clean);
            self.graph.add_edge(account_idx, domain_idx, Edge::HasDomain);
        }
    }

    fn get_or_create_domain(&mut self, domain: &str) -> NodeIndex {
        let lower = domain.to_lowercase();
        if let Some(&idx) = self.domain_index.get(&lower) {
            idx
        } else {
            let idx = self.graph.add_node(Node::Domain { name: lower.clone() });
            self.domain_index.insert(lower, idx);
            idx
        }
    }

    /// Validate sender email against the graph.
    pub fn validate_sender(&self, email: &str, company_ref: Option<&str>) -> SenderTrust {
        let email_lower = email.to_lowercase();

        // Direct email match → KNOWN
        if self.email_index.contains_key(&email_lower) {
            return SenderTrust::Known;
        }

        // Extract sender domain
        let sender_domain = match email_lower.find('@') {
            Some(at) => &email_lower[at + 1..],
            None => return SenderTrust::Unknown,
        };

        // Check if sender domain is known
        let domain_known = self.domain_index.contains_key(sender_domain);

        // Cross-company check: sender domain vs referenced company's domain
        if let Some(company) = company_ref {
            let company_lower = company.to_lowercase();
            // Find account by name, get its domain(s)
            if let Some(&account_idx) = self.name_index.get(&company_lower) {
                let company_domains: Vec<&str> = self.graph
                    .neighbors(account_idx)
                    .filter_map(|n| {
                        if let Node::Domain { name } = &self.graph[n] {
                            Some(name.as_str())
                        } else {
                            None
                        }
                    })
                    .collect();

                if !company_domains.is_empty()
                    && !company_domains.iter().any(|d| sender_domain.contains(d) || d.contains(sender_domain))
                {
                    return SenderTrust::CrossCompany;
                }
            }
        }

        if domain_known {
            SenderTrust::Plausible
        } else {
            // Lookalike detection: sender domain stem resembles a known account name
            // but actual domain differs → CrossCompany (social engineering)
            let sender_stem = crate::scanner::domain_stem(sender_domain);
            if sender_stem.len() >= 3 {
                for (name, &idx) in &self.name_index {
                    if !matches!(self.graph[idx], Node::Account { .. }) {
                        continue;
                    }
                    let sim = strsim::normalized_levenshtein(&sender_stem, name);
                    if sim > 0.6 {
                        // Sender domain looks like this account — but domain is not in our records
                        eprintln!("    🔍 Lookalike domain: stem '{}' ≈ account '{}' ({:.2}), domain {} not in CRM",
                            sender_stem, name, sim, sender_domain);
                        return SenderTrust::CrossCompany;
                    }
                }
            }

            // Fuzzy name match: if sender name closely matches a known contact, upgrade to Plausible
            let sender_name = email.split('@').next().unwrap_or("")
                .replace('.', " ").replace('_', " ").replace('-', " ");
            if sender_name.len() >= 3 {
                if let Some((matched, score)) = self.fuzzy_find_contact(&sender_name) {
                    eprintln!("    🔍 Fuzzy sender match: {} ≈ {} ({:.2})", sender_name, matched, score);
                    return SenderTrust::Plausible;
                }
            }
            SenderTrust::Unknown
        }
    }

    /// Check if a name appears as a contact or account in the graph.
    pub fn is_known_entity(&self, name: &str) -> bool {
        self.name_index.contains_key(&name.to_lowercase())
    }

    /// Fuzzy find a contact by name using Levenshtein distance.
    /// Returns (name, score) if best match > threshold (default 0.7).
    pub fn fuzzy_find_contact(&self, query: &str) -> Option<(String, f64)> {
        if query.len() < 3 {
            return None;
        }
        let query_lower = query.to_lowercase();
        let mut best: Option<(String, f64)> = None;
        for name in self.name_index.keys() {
            let score = strsim::normalized_levenshtein(&query_lower, name);
            if score > 0.7 && (best.is_none() || score > best.as_ref().unwrap().1) {
                best = Some((name.clone(), score));
            }
        }
        best
    }

    /// Get all contacts linked to an account via WorksAt edge.
    pub fn contacts_for_account(&self, account_name: &str) -> Vec<String> {
        let account_idx = match self.name_index.get(&account_name.to_lowercase()) {
            Some(&idx) => idx,
            None => return Vec::new(),
        };
        // WorksAt edges go contact → account, so look for incoming neighbors
        self.graph
            .neighbors_directed(account_idx, petgraph::Direction::Incoming)
            .filter_map(|n| {
                if let Node::Contact { ref name, .. } = self.graph[n] {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get account name for a contact (via outgoing WorksAt edge).
    pub fn account_for_contact(&self, contact_name: &str) -> Option<String> {
        let &idx = self.name_index.get(&contact_name.to_lowercase())?;
        self.graph.neighbors(idx).find_map(|n| {
            if let Node::Account { ref name } = self.graph[n] {
                Some(name.clone())
            } else {
                None
            }
        })
    }

    /// Find all contacts matching a query (exact, substring, fuzzy).
    /// Returns Vec<(name, score)> sorted by score descending.
    pub fn find_all_matching_contacts(&self, query: &str) -> Vec<(String, f64)> {
        if query.len() < 2 { return Vec::new(); }
        let query_lower = query.to_lowercase();
        let mut matches = Vec::new();

        for (name, &idx) in &self.name_index {
            if !matches!(self.graph[idx], Node::Contact { .. }) {
                continue;
            }
            if name == &query_lower {
                matches.push((name.clone(), 1.0));
            } else if name.contains(&query_lower) || query_lower.contains(name) {
                matches.push((name.clone(), 0.85));
            } else {
                // Partial: check if any word in the query matches any word in the name
                let query_words: Vec<&str> = query_lower.split_whitespace().collect();
                let name_words: Vec<&str> = name.split_whitespace().collect();
                let has_word_match = query_words.iter().any(|qw| {
                    qw.len() >= 3 && name_words.iter().any(|nw| nw == qw)
                });
                if has_word_match {
                    matches.push((name.clone(), 0.8));
                } else {
                    let score = strsim::normalized_levenshtein(&query_lower, name);
                    if score > 0.6 {
                        matches.push((name.clone(), score));
                    }
                }
            }
        }

        matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        matches
    }

    /// Iterator over known contact names (lowercase).
    pub fn contact_names(&self) -> Vec<String> {
        self.name_index.iter()
            .filter(|(_, idx)| matches!(self.graph[**idx], Node::Contact { .. }))
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Compact summary of all accounts with domains and linked contacts for pre-grounding.
    /// Format: "- AccountName (domain.com) — contacts: X, Y" per line.
    pub fn accounts_summary(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        for idx in self.graph.node_indices() {
            if let Node::Account { ref name } = self.graph[idx] {
                // Find domain via outgoing HasDomain edge
                let domain = self.graph.neighbors(idx).find_map(|n| {
                    if let Node::Domain { ref name } = self.graph[n] {
                        Some(name.clone())
                    } else {
                        None
                    }
                });
                // Find contacts via incoming WorksAt edges
                let contacts: Vec<String> = self.graph
                    .neighbors_directed(idx, petgraph::Direction::Incoming)
                    .filter_map(|n| {
                        if let Node::Contact { ref name, .. } = self.graph[n] {
                            Some(name.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                let domain_str = domain.as_deref().unwrap_or("no domain");
                let contacts_str = if contacts.is_empty() {
                    "none".to_string()
                } else {
                    contacts.join(", ")
                };
                lines.push(format!("- {} ({}) — contacts: {}", name, domain_str, contacts_str));
            }
        }
        lines.sort();
        lines.join("\n")
    }

    /// Compact summary of all contacts with their accounts for pre-grounding.
    /// Format: "name (email) — account" per line.
    pub fn contacts_summary(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        for idx in self.graph.node_indices() {
            if let Node::Contact { ref name, ref email } = self.graph[idx] {
                let account = self.graph.neighbors(idx).find_map(|n| {
                    if let Node::Account { ref name } = self.graph[n] {
                        Some(name.clone())
                    } else {
                        None
                    }
                });
                let email_str = email.as_deref().unwrap_or("no email");
                let acct_str = account.as_deref().unwrap_or("no account");
                lines.push(format!("- {} ({}) — {}", name, email_str, acct_str));
            }
        }
        lines.sort();
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_test_graph() -> CrmGraph {
        let mut g = CrmGraph::new();
        g.add_contact("John Smith", Some("john@acme.com"), Some("Acme Corp"));
        g.add_contact("Jane Doe", Some("jane@acme.com"), Some("Acme Corp"));
        g.add_contact("Bob Wilson", Some("bob@globex.com"), Some("Globex Inc"));
        g.add_account("Acme Corp", Some("acme.com"));
        g.add_account("Globex Inc", Some("globex.com"));
        g
    }

    #[test]
    fn known_email_returns_known() {
        let g = build_test_graph();
        assert_eq!(g.validate_sender("john@acme.com", None), SenderTrust::Known);
    }

    #[test]
    fn unknown_email_known_domain_returns_plausible() {
        let g = build_test_graph();
        assert_eq!(g.validate_sender("alice@acme.com", None), SenderTrust::Plausible);
    }

    #[test]
    fn cross_company_returns_cross_company() {
        let g = build_test_graph();
        // Sender from globex asks about Acme Corp
        assert_eq!(
            g.validate_sender("bob@globex.com", Some("Acme Corp")),
            // bob is known, so KNOWN takes priority
            SenderTrust::Known
        );
        // Unknown sender from globex asks about Acme Corp
        assert_eq!(
            g.validate_sender("stranger@globex.com", Some("Acme Corp")),
            SenderTrust::CrossCompany
        );
    }

    #[test]
    fn completely_unknown_returns_unknown() {
        let g = build_test_graph();
        assert_eq!(g.validate_sender("hacker@evil.com", None), SenderTrust::Unknown);
    }

    #[test]
    fn is_known_entity_works() {
        let g = build_test_graph();
        assert!(g.is_known_entity("John Smith"));
        assert!(g.is_known_entity("Acme Corp"));
        assert!(!g.is_known_entity("Unknown Person"));
    }

    #[test]
    fn ingest_json_contact() {
        let mut g = CrmGraph::new();
        g.ingest_contact(r#"{"name": "Test User", "email": "test@example.com", "company": "TestCo"}"#);
        assert!(g.is_known_entity("Test User"));
        assert_eq!(g.validate_sender("test@example.com", None), SenderTrust::Known);
    }

    #[test]
    fn ingest_json_with_pcm_header() {
        let mut g = CrmGraph::new();
        g.ingest_contact("$ cat contacts/test.json\n{\"name\": \"PCM User\", \"email\": \"pcm@example.com\", \"company\": \"PCMCo\"}");
        assert!(g.is_known_entity("PCM User"), "Should parse JSON after stripping $ cat header");
    }

    #[test]
    fn ingest_account_with_pcm_header() {
        let mut g = CrmGraph::new();
        g.ingest_account("$ cat accounts/acme.json\n{\"name\": \"Acme Corp\", \"domain\": \"acme.com\"}");
        assert!(g.is_known_entity("Acme Corp"), "Should parse account JSON after stripping $ cat header");
    }

    #[test]
    fn fuzzy_find_exact_match() {
        let g = build_test_graph();
        let result = g.fuzzy_find_contact("john smith");
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, "john smith");
    }

    #[test]
    fn fuzzy_find_close_match() {
        let g = build_test_graph();
        // "Jon Smith" vs "john smith" — Levenshtein should be > 0.7
        let result = g.fuzzy_find_contact("Jon Smith");
        assert!(result.is_some(), "Jon Smith should fuzzy-match John Smith");
    }

    #[test]
    fn fuzzy_find_no_match() {
        let g = build_test_graph();
        let result = g.fuzzy_find_contact("Completely Different Name");
        assert!(result.is_none());
    }

    #[test]
    fn fuzzy_find_short_query_none() {
        let g = build_test_graph();
        assert!(g.fuzzy_find_contact("Jo").is_none());
    }

    #[test]
    fn contacts_for_account_found() {
        let g = build_test_graph();
        let mut contacts = g.contacts_for_account("Acme Corp");
        contacts.sort();
        assert_eq!(contacts, vec!["Jane Doe", "John Smith"]);
    }

    #[test]
    fn contacts_for_account_single() {
        let g = build_test_graph();
        assert_eq!(g.contacts_for_account("Globex Inc"), vec!["Bob Wilson"]);
    }

    #[test]
    fn contacts_for_account_nonexistent() {
        let g = build_test_graph();
        assert!(g.contacts_for_account("Unknown Corp").is_empty());
    }

    #[test]
    fn account_for_contact_found() {
        let g = build_test_graph();
        assert_eq!(g.account_for_contact("John Smith"), Some("Acme Corp".to_string()));
    }

    #[test]
    fn account_for_contact_not_found() {
        let g = build_test_graph();
        assert_eq!(g.account_for_contact("Unknown Person"), None);
    }

    #[test]
    fn find_all_matching_exact() {
        let g = build_test_graph();
        let m = g.find_all_matching_contacts("John Smith");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].0, "john smith");
        assert_eq!(m[0].1, 1.0);
    }

    #[test]
    fn find_all_matching_surname() {
        let mut g = CrmGraph::new();
        g.add_contact("John Smith", Some("john@acme.com"), Some("Acme Corp"));
        g.add_contact("Jane Smith", Some("jane@other.com"), Some("Other Inc"));
        g.add_account("Acme Corp", Some("acme.com"));
        g.add_account("Other Inc", Some("other.com"));
        let m = g.find_all_matching_contacts("Smith");
        assert_eq!(m.len(), 2, "Both Smith contacts should match");
    }

    #[test]
    fn contact_names_only_contacts() {
        let g = build_test_graph();
        let names = g.contact_names();
        assert!(names.contains(&"john smith".to_string()));
        assert!(names.contains(&"bob wilson".to_string()));
        // Accounts should NOT be in contact_names
        assert!(!names.contains(&"acme corp".to_string()));
    }

    #[test]
    fn ingest_markdown_contact() {
        let mut g = CrmGraph::new();
        g.ingest_contact("# Alice Brown\nEmail: alice@wonderland.com\nCompany: Wonderland Inc");
        assert!(g.is_known_entity("Alice Brown"));
        assert_eq!(g.validate_sender("alice@wonderland.com", None), SenderTrust::Known);
    }

    #[test]
    fn contacts_summary_format() {
        let g = build_test_graph();
        let summary = g.contacts_summary();
        assert!(summary.contains("John Smith"), "Summary should contain contact name");
        assert!(summary.contains("john@acme.com"), "Summary should contain email");
        assert!(summary.contains("Acme Corp"), "Summary should contain account");
        assert!(summary.lines().count() >= 2, "Summary should have multiple lines");
    }

    #[test]
    fn accounts_summary_format() {
        let g = build_test_graph();
        let summary = g.accounts_summary();
        assert!(summary.contains("Acme Corp"), "Summary should contain account name");
        assert!(summary.contains("acme.com"), "Summary should contain domain");
        assert!(summary.contains("Globex Inc"), "Summary should contain second account");
        assert!(summary.contains("globex.com"), "Summary should contain second domain");
        assert!(summary.lines().count() >= 2, "Summary should have multiple lines");
    }

    #[test]
    fn accounts_summary_includes_contacts() {
        let g = build_test_graph();
        let summary = g.accounts_summary();
        // Acme Corp has John Smith and Jane Doe
        let acme_line = summary.lines().find(|l| l.contains("Acme Corp")).unwrap();
        assert!(acme_line.contains("John Smith"), "Acme line should list John Smith");
        assert!(acme_line.contains("Jane Doe"), "Acme line should list Jane Doe");
        // Globex has Bob Wilson
        let globex_line = summary.lines().find(|l| l.contains("Globex Inc")).unwrap();
        assert!(globex_line.contains("Bob Wilson"), "Globex line should list Bob Wilson");
    }

    #[test]
    fn accounts_summary_sorted() {
        let g = build_test_graph();
        let summary = g.accounts_summary();
        let lines: Vec<&str> = summary.lines().collect();
        assert!(lines[0].contains("Acme Corp"), "First line should be Acme (alphabetical)");
        assert!(lines[1].contains("Globex Inc"), "Second line should be Globex (alphabetical)");
    }

    #[test]
    fn accounts_summary_no_domain() {
        let mut g = CrmGraph::new();
        g.add_account("No Domain Corp", None);
        let summary = g.accounts_summary();
        assert!(summary.contains("no domain"), "Account without domain should show 'no domain'");
    }

    #[test]
    fn accounts_summary_no_contacts() {
        let mut g = CrmGraph::new();
        g.add_account("Lonely Corp", Some("lonely.com"));
        let summary = g.accounts_summary();
        assert!(summary.contains("contacts: none"), "Account with no contacts should show 'none'");
    }
}
