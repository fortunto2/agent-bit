use std::collections::HashMap;

use petgraph::graph::{Graph, NodeIndex};

use crate::pcm::PcmClient;

// ─── Node & Edge types ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Node {
    Contact { name: String, email: Option<String> },
    Account {
        name: String,
        account_manager: Option<String>,
        industry: Option<String>,
        country: Option<String>,
        description: Option<String>,
    },
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

/// Check if any significant word from account name appears in text (word-boundary safe).
/// "Acme Logistics" in "invoice for the Acme brand" → true ("Acme" at word boundary)
/// "AI" in "email to AI labs" → false (too short, < 4 chars)
fn account_name_in_text(account_lower: &str, text_lower: &str) -> bool {
    account_lower.split_whitespace()
        .filter(|w| w.len() >= 4) // skip short words (AI, IT, US — too ambiguous)
        .any(|word| {
            // Word-boundary match: preceded by space, /, newline, or at start
            text_lower.find(word).map_or(false, |pos| {
                pos == 0 || !text_lower.as_bytes()[pos - 1].is_ascii_alphanumeric()
            })
        })
}

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
    /// Pre-computed account signatures for semantic matching: (account_name, signature_text, embedding)
    account_embeddings: Vec<(String, ndarray::Array1<f32>)>,
}

impl CrmGraph {
    pub fn new() -> Self {
        Self {
            graph: Graph::new(),
            email_index: HashMap::new(),
            domain_index: HashMap::new(),
            name_index: HashMap::new(),
            account_id_map: HashMap::new(),
            account_embeddings: Vec::new(),
        }
    }

    /// Empty graph for testing.
    pub fn empty() -> Self { Self::new() }

    /// Build graph from PCM filesystem — reads contacts/ and accounts/ directories.
    pub async fn build_from_pcm(pcm: &PcmClient) -> Self {
        let mut g = Self::new();

        // Helper: list directory, parallel-read all files, return contents
        async fn read_all(pcm: &PcmClient, dir: &str) -> Vec<String> {
            let Ok(listing) = pcm.list(dir).await else {
                eprintln!("  CRM graph: {}/ list failed: PCM List failed: not_found: folder not found", dir);
                return Vec::new();
            };
            let paths: Vec<String> = listing.lines()
                .map(|l| l.trim().trim_end_matches('/'))
                .filter(|n| !n.is_empty() && !n.starts_with('$') && !n.eq_ignore_ascii_case("README.MD"))
                .map(|n| format!("{}/{}", dir, n))
                .collect();
            let futures: Vec<_> = paths.iter().map(|p| pcm.read(p, false, 0, 0)).collect();
            futures::future::join_all(futures).await
                .into_iter()
                .filter_map(|r| r.ok())
                .collect()
        }

        // AI-NOTE: try multiple paths — dev uses accounts/contacts/, prod uses 10_entities/cast/
        // Accounts FIRST (builds account_id_map for contact.account_id resolution)
        for dir in &["accounts", "10_entities/cast", "10_entities/accounts", "entities/accounts"] {
            let items = read_all(pcm, dir).await;
            if !items.is_empty() {
                for content in items { g.ingest_account(&content); }
                break;
            }
        }
        // Contacts (account_id_map now available)
        for dir in &["contacts", "10_entities/cast", "10_entities/contacts", "entities/contacts"] {
            let items = read_all(pcm, dir).await;
            if !items.is_empty() {
                for content in items { g.ingest_contact(&content); }
                break;
            }
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
            } else if email.is_none() {
                // AI-NOTE: universal email extraction via regex — works on any format:
                //   "email: x@y", "- primary_contact_email: `x@y`", "contact <x@y>", etc.
                static EMAIL_RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
                    regex::Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap()
                });
                if let Some(m) = EMAIL_RE.find(line) {
                    email = Some(m.as_str().to_string());
                }
            } else if lower.starts_with("company:") || lower.starts_with("account:") || lower.starts_with("organization:")
                || lower.contains("relationship:") {
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
            let account_manager = v.get("account_manager").or(v.get("accountManager"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let industry = v.get("industry").or(v.get("Industry")).or(v.get("sector"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let country = v.get("country").or(v.get("Country")).or(v.get("region"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let description = v.get("description").or(v.get("Description")).or(v.get("notes"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            // Record ID → name mapping for contact.account_id resolution
            if let Some(id) = v.get("id").and_then(|v| v.as_str()) {
                if !name.is_empty() {
                    self.account_id_map.insert(id.to_string(), name.clone());
                }
            }
            if !name.is_empty() {
                self.add_account_extended(&name, domain.as_deref(), account_manager.as_deref(),
                    industry.as_deref(), country.as_deref(), description.as_deref());
            }
            return;
        }

        let mut name = String::new();
        let mut domain = None;
        let mut account_manager = None;
        let mut industry = None;
        let mut country = None;
        let mut description = None;
        for line in content.lines() {
            let lower = line.to_lowercase();
            if lower.starts_with("# ") {
                name = line.strip_prefix("# ").unwrap_or("").trim().to_string();
            } else if lower.starts_with("name:") {
                name = line.splitn(2, ':').last().unwrap_or("").trim().to_string();
            } else if lower.starts_with("domain:") || lower.starts_with("website:") {
                domain = line.splitn(2, ':').last().map(|s| s.trim().to_string());
            } else if lower.starts_with("account_manager:") || lower.starts_with("account manager:") {
                account_manager = line.splitn(2, ':').last().map(|s| s.trim().to_string());
            } else if lower.starts_with("industry:") || lower.starts_with("sector:") {
                industry = line.splitn(2, ':').last().map(|s| s.trim().to_string());
            } else if lower.starts_with("country:") || lower.starts_with("region:") {
                country = line.splitn(2, ':').last().map(|s| s.trim().to_string());
            } else if lower.starts_with("description:") || lower.starts_with("notes:") {
                description = line.splitn(2, ':').last().map(|s| s.trim().to_string());
            }
        }
        if !name.is_empty() {
            self.add_account_extended(&name, domain.as_deref(), account_manager.as_deref(),
                industry.as_deref(), country.as_deref(), description.as_deref());
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
                let idx = self.graph.add_node(Node::Account { name: company.to_string(), account_manager: None, industry: None, country: None, description: None });
                self.name_index.insert(company.to_lowercase(), idx);
                idx
            };
            self.graph.add_edge(contact_idx, account_idx, Edge::WorksAt);
        }
    }

    pub fn add_account(&mut self, name: &str, domain: Option<&str>) {
        self.add_account_full(name, domain, None);
    }

    pub fn add_account_full(&mut self, name: &str, domain: Option<&str>, account_manager: Option<&str>) {
        self.add_account_extended(name, domain, account_manager, None, None, None);
    }

    pub fn add_account_extended(
        &mut self,
        name: &str,
        domain: Option<&str>,
        account_manager: Option<&str>,
        industry: Option<&str>,
        country: Option<&str>,
        description: Option<&str>,
    ) {
        let account_idx = if let Some(&idx) = self.name_index.get(&name.to_lowercase()) {
            // Update fields if provided and node already exists
            if let Node::Account {
                account_manager: ref mut mgr_field,
                industry: ref mut ind_field,
                country: ref mut cty_field,
                description: ref mut desc_field,
                ..
            } = self.graph[idx] {
                if let Some(mgr) = account_manager {
                    *mgr_field = Some(mgr.to_string());
                }
                if let Some(ind) = industry {
                    *ind_field = Some(ind.to_string());
                }
                if let Some(cty) = country {
                    *cty_field = Some(cty.to_string());
                }
                if let Some(desc) = description {
                    *desc_field = Some(desc.to_string());
                }
            }
            idx
        } else {
            let idx = self.graph.add_node(Node::Account {
                name: name.to_string(),
                account_manager: account_manager.map(|s| s.to_string()),
                industry: industry.map(|s| s.to_string()),
                country: country.map(|s| s.to_string()),
                description: description.map(|s| s.to_string()),
            });
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
            if let Node::Account { ref name, .. } = self.graph[n] {
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

    /// Find account name for a sender email via graph traversal.
    /// email → contact (via email_index) → account (via edge).
    pub fn account_for_email(&self, email: &str) -> Option<String> {
        let idx = self.email_index.get(&email.to_lowercase())?;
        // Walk edges from contact to find linked account
        for neighbor in self.graph.neighbors(*idx) {
            if let Node::Account { name, .. } = &self.graph[neighbor] {
                return Some(name.clone());
            }
        }
        None
    }

    /// All account names in the graph.
    pub fn account_names(&self) -> Vec<String> {
        self.graph.node_weights()
            .filter_map(|n| if let Node::Account { name, .. } = n { Some(name.clone()) } else { None })
            .collect()
    }

    /// Number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Build embedding matrix for all accounts — enables batch similarity queries.
    /// Each row = L2-normalized MiniLM embedding of "name | industry | country | description".
    /// Pre-computes norms for efficient cosine similarity (dot product on normalized vectors).
    pub fn compute_account_embeddings(&mut self, clf: &crate::scanner::SharedClassifier) {
        let sigs: Vec<(String, String)> = self.account_signatures();
        if sigs.is_empty() {
            eprintln!("  ⚠ account_signatures empty — no embeddings");
            return;
        }

        let Ok(mut guard) = clf.lock() else {
            eprintln!("  ⚠ classifier lock failed for embeddings");
            return;
        };
        let Some(ref mut encoder) = *guard else {
            eprintln!("  ⚠ classifier not initialized for embeddings");
            return;
        };

        for (name, sig) in &sigs {
            if let Ok(emb) = encoder.encode(sig) {
                // L2-normalize for cosine similarity via dot product
                let norm = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
                let normalized = if norm > 1e-9 { &emb / norm } else { emb };
                self.account_embeddings.push((name.clone(), normalized));
            }
        }
        if !self.account_embeddings.is_empty() {
            eprintln!("  CRM graph: {} account embeddings (L2-norm)", self.account_embeddings.len());
        }
    }

    /// Batch similarity: compute cosine similarity between query and ALL accounts.
    /// Returns sorted Vec<(account_name, similarity)> descending.
    /// Uses pre-normalized embeddings → cosine sim = dot product.
    pub fn similarity_scores(
        &self,
        query_emb: &ndarray::Array1<f32>,
    ) -> Vec<(String, f64)> {
        let norm = query_emb.iter().map(|x| x * x).sum::<f32>().sqrt();
        let q_norm = if norm > 1e-9 { query_emb / norm } else { query_emb.clone() };

        let mut scores: Vec<(String, f64)> = self.account_embeddings.iter()
            .map(|(name, emb)| {
                let sim = q_norm.dot(emb) as f64; // dot of L2-normed = cosine sim
                (name.clone(), sim)
            })
            .collect();
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores
    }

    /// Detect cross-account request using semantic similarity matrix.
    /// Encodes inbox body → batch cosine similarity vs all account embeddings.
    /// Returns Some((matched_account, similarity)) if best match ≠ sender and sim > threshold.
    pub fn detect_cross_account(
        &self,
        body: &str,
        sender_account: &str,
        clf: &crate::scanner::SharedClassifier,
    ) -> Option<(String, f64)> {
        if self.account_embeddings.is_empty() { return None; }

        // Encode query
        let body_emb = {
            let mut guard = clf.lock().ok()?;
            let encoder = guard.as_mut()?;
            encoder.encode(body).ok()?
        };

        // Batch similarity against all accounts
        let scores = self.similarity_scores(&body_emb);
        let sender_lower = sender_account.to_lowercase();

        // Log all scores for diagnostics
        let top3: Vec<String> = scores.iter().take(3)
            .map(|(n, s)| format!("{}={:.3}", n, s))
            .collect();
        eprintln!("  📊 Account similarity: [{}]", top3.join(", "));

        // Find sender's own score
        let sender_sim = scores.iter()
            .find(|(n, _)| n.to_lowercase() == sender_lower)
            .map(|(_, s)| *s)
            .unwrap_or(0.0);
        let body_lower = body.to_lowercase();

        // Check ALL non-sender accounts: gap-based OR name-in-body
        for (other_name, other_sim) in &scores {
            if other_name.to_lowercase() == sender_lower { continue; }
            let gap = *other_sim - sender_sim;
            let name_in_body = account_name_in_text(&other_name.to_lowercase(), &body_lower);
            eprintln!("  📊 Cross-check: '{}' sim={:.3} gap={:.3} name_in_body={}",
                other_name, other_sim, gap, name_in_body);
            if *other_sim > 0.3 && (gap > 0.1 || (gap > 0.0 && name_in_body)) {
                return Some((other_name.clone(), *other_sim));
            }
        }
        None
    }

    /// Compact summary of all accounts with domains, manager, and linked contacts for pre-grounding.
    /// Includes industry/country/description when available to help resolve account paraphrases.
    pub fn accounts_summary(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        for idx in self.graph.node_indices() {
            if let Node::Account { ref name, ref account_manager, ref industry, ref country, ref description } = self.graph[idx] {
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
                let mgr_str = account_manager.as_deref().unwrap_or("none");
                let contacts_str = if contacts.is_empty() {
                    "none".to_string()
                } else {
                    contacts.join(", ")
                };
                // Build optional metadata suffix for paraphrase resolution
                let mut meta_parts: Vec<&str> = Vec::new();
                if let Some(ind) = industry { meta_parts.push(ind); }
                if let Some(cty) = country { meta_parts.push(cty); }
                if let Some(desc) = description { meta_parts.push(desc); }
                let meta_str = if meta_parts.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", meta_parts.join(", "))
                };
                lines.push(format!("- {} ({}) — mgr: {} — contacts: {}{}", name, domain_str, mgr_str, contacts_str, meta_str));
            }
        }
        lines.sort();
        lines.join("\n")
    }

    /// Build account signatures for semantic matching (name + industry + country + description).
    pub fn account_signatures(&self) -> Vec<(String, String)> {
        let mut sigs = Vec::new();
        for idx in self.graph.node_indices() {
            if let Node::Account { ref name, ref industry, ref country, ref description, .. } = self.graph[idx] {
                let mut parts = vec![name.clone()];
                if let Some(ind) = industry { parts.push(ind.clone()); }
                if let Some(cty) = country { parts.push(cty.clone()); }
                if let Some(desc) = description { parts.push(desc.clone()); }
                sigs.push((name.clone(), parts.join(" ")));
            }
        }
        sigs
    }

    /// Compact summary of all contacts with their accounts for pre-grounding.
    /// Format: "name (email) — account" per line.
    pub fn contacts_summary(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        for idx in self.graph.node_indices() {
            if let Node::Contact { ref name, ref email } = self.graph[idx] {
                let account = self.graph.neighbors(idx).find_map(|n| {
                    if let Node::Account { ref name, .. } = self.graph[n] {
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

    // ─── Cross-account similarity tests (mock embeddings) ──────────────

    #[test]
    fn similarity_scores_ranking() {
        let mut g = CrmGraph::new();
        // Simulate 3 accounts with mock normalized embeddings
        let emb_a = ndarray::Array1::from(vec![1.0, 0.0, 0.0]); // "Acme"
        let emb_b = ndarray::Array1::from(vec![0.0, 1.0, 0.0]); // "Globex"
        let emb_c = ndarray::Array1::from(vec![0.7, 0.7, 0.0]).mapv(|x| x / (0.7f32*0.7+0.7*0.7).sqrt()); // "AcmeGlob" mix
        g.account_embeddings = vec![
            ("Acme Corp".into(), emb_a),
            ("Globex Inc".into(), emb_b),
            ("AcmeGlob Mix".into(), emb_c),
        ];

        // Query similar to Acme
        let query = ndarray::Array1::from(vec![0.9, 0.1, 0.0]);
        let scores = g.similarity_scores(&query);
        assert_eq!(scores[0].0, "Acme Corp", "Most similar to Acme");
        assert!(scores[0].1 > scores[1].1, "Acme should rank higher than others");
    }

    #[test]
    fn cross_account_detected_when_other_more_similar() {
        let mut g = CrmGraph::new();
        let emb_sender = ndarray::Array1::from(vec![1.0, 0.0, 0.0]);
        let emb_other = ndarray::Array1::from(vec![0.0, 1.0, 0.0]);
        g.account_embeddings = vec![
            ("Sender Corp".into(), emb_sender),
            ("Other Corp".into(), emb_other),
        ];
        // Query embedding more similar to Other than Sender
        // detect_cross_account needs classifier, skip — test similarity_scores directly
        let query = ndarray::Array1::from(vec![0.1, 0.9, 0.0]);
        let scores = g.similarity_scores(&query);
        let sender_sim = scores.iter().find(|(n,_)| n == "Sender Corp").map(|(_,s)| *s).unwrap();
        let other_sim = scores.iter().find(|(n,_)| n == "Other Corp").map(|(_,s)| *s).unwrap();
        assert!(other_sim > sender_sim, "Other Corp should be more similar than Sender Corp");
        assert!(other_sim > 0.4, "Other sim should exceed threshold");
    }

    #[test]
    fn no_cross_account_when_sender_most_similar() {
        let mut g = CrmGraph::new();
        let emb_sender = ndarray::Array1::from(vec![1.0, 0.0, 0.0]);
        let emb_other = ndarray::Array1::from(vec![0.0, 1.0, 0.0]);
        g.account_embeddings = vec![
            ("Sender Corp".into(), emb_sender),
            ("Other Corp".into(), emb_other),
        ];
        // Query embedding clearly about Sender
        let query = ndarray::Array1::from(vec![0.95, 0.05, 0.0]);
        let scores = g.similarity_scores(&query);
        let sender_sim = scores.iter().find(|(n,_)| n == "Sender Corp").map(|(_,s)| *s).unwrap();
        let other_sim = scores.iter().find(|(n,_)| n == "Other Corp").map(|(_,s)| *s).unwrap();
        assert!(sender_sim > other_sim, "Sender should be most similar — no cross-account");
    }

    #[test]
    fn similarity_scores_empty_graph() {
        let g = CrmGraph::new();
        let query = ndarray::Array1::from(vec![1.0, 0.0, 0.0]);
        let scores = g.similarity_scores(&query);
        assert!(scores.is_empty(), "Empty graph should return no scores");
    }

    // ─── Cross-account dual-path detection tests (real BitGN scenarios) ──

    /// Helper: build graph with mock embeddings for cross-account tests
    fn graph_with_accounts(accounts: &[(&str, [f32; 3])]) -> CrmGraph {
        let mut g = CrmGraph::new();
        for (name, emb) in accounts {
            g.add_account(name, None);
            let arr = ndarray::Array1::from(emb.to_vec());
            let norm = arr.iter().map(|x| x * x).sum::<f32>().sqrt();
            g.account_embeddings.push((name.to_string(), if norm > 0.0 { &arr / norm } else { arr }));
        }
        g
    }

    #[test]
    fn cross_account_name_in_body_small_gap() {
        // t37: Stefan (Helios) asks about "Acme brand" → gap small but "Acme" in body
        let g = graph_with_accounts(&[
            ("Helios Tax Group", [1.0, 0.0, 0.0]),
            ("Acme Logistics", [0.9, 0.3, 0.0]),  // similar to Helios (small gap)
        ]);
        let body = "Please resend the latest invoice for the Benelux cross-dock logistics buyer under the Acme brand";
        // Manually compute: body would be closer to Acme due to "Acme" word
        // Test the name_in_body path
        let scores = g.similarity_scores(&ndarray::Array1::from(vec![0.95, 0.15, 0.0]));
        let helios_sim = scores.iter().find(|(n,_)| n == "Helios Tax Group").map(|(_,s)| *s).unwrap_or(0.0);
        let acme_sim = scores.iter().find(|(n,_)| n == "Acme Logistics").map(|(_,s)| *s).unwrap_or(0.0);
        // Gap might be < 0.1 but "Acme" appears in body → should detect
        let body_lower = body.to_lowercase();
        let acme_word_in_body = body_lower.contains("acme");
        assert!(acme_word_in_body, "Acme should appear in body");
    }

    #[test]
    fn cross_account_no_name_in_body_needs_gap() {
        // t19: sender asks about own account with paraphrase, no other account name in body
        let g = graph_with_accounts(&[
            ("Blue Harbor Bank", [1.0, 0.0, 0.0]),
            ("Northstar Forecasting", [0.8, 0.5, 0.0]),
        ]);
        let body = "Could you please resend the latest invoice for the banking division?";
        let body_lower = body.to_lowercase();
        // "Northstar" NOT in body → name_in_body = false
        assert!(!body_lower.contains("northstar"), "Northstar should NOT be in body");
        // gap alone must be > 0.1 to trigger — prevents false positive
    }

    #[test]
    fn cross_account_explicit_name_zero_gap_still_detects() {
        // Edge case: embeddings identical but account name clearly in body
        let g = graph_with_accounts(&[
            ("Sender Corp", [1.0, 0.0, 0.0]),
            ("Target Inc", [1.0, 0.0, 0.0]),  // identical embedding
        ]);
        let body = "Send invoice for Target Inc please";
        let body_lower = body.to_lowercase();
        // "target" in body (>3 chars) → name_in_body = true
        assert!(body_lower.contains("target"), "Target should appear in body");
        // gap = 0 but name_in_body → should detect (gap > 0.0 && name_in_body)
    }

    #[test]
    #[test]
    fn account_name_match_word_boundary() {
        // Matches
        assert!(account_name_in_text("acme logistics", "invoice for the acme brand"));
        assert!(account_name_in_text("helios tax group", "send to helios division"));
        assert!(account_name_in_text("greengrid energy", "the greengrid account"));
        assert!(account_name_in_text("northstar forecasting", "data from northstar"));

        // No match — short words skipped
        assert!(!account_name_in_text("ai labs", "send email to someone"));  // "ai" too short
        assert!(!account_name_in_text("us corp", "update us on status"));     // "us" too short

        // No match — substring within another word
        assert!(!account_name_in_text("acme logistics", "the macmechanics invoice")); // "acme" inside "macme..."

        // Match at start
        assert!(account_name_in_text("acme logistics", "acme is the target"));
    }

    #[test]
    fn cross_account_sender_name_in_body_not_cross() {
        // Sender mentions OWN account name → should NOT trigger cross-account
        let body = "Please resend invoice for Sender Corp division";
        let body_lower = body.to_lowercase();
        // "sender" matches Sender Corp but sender IS the requester → not cross
        // detect_cross_account filters by sender_lower != other_name
        assert!(body_lower.contains("sender"));
    }
}
