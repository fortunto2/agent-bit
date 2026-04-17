//! BitGN PcmRuntime client — typed Connect-RPC via bitgn-sdk.

use anyhow::Result;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use bitgn_sdk::vm::pcm::{self as proto, PcmRuntimeClient};
use connectrpc::client::HttpClient;

/// Deferred answer: stored by AnswerTool, submitted after verifier review.
#[derive(Debug, Clone)]
pub struct ProposedAnswer {
    pub message: String,
    pub outcome: String,
    pub refs: Vec<String>,
}

pub struct PcmClient {
    inner: PcmRuntimeClient<HttpClient>,
    pub answer_submitted: AtomicBool,
    proposed_answer: Mutex<Option<ProposedAnswer>>,
    /// Paths of files read during this trial — for auto-refs in AnswerTool.
    recent_reads: Mutex<Vec<String>>,
    /// Read cache — keyed by path. Invalidated by write/delete to same path.
    /// Single owner: PcmClient. All tools share via Arc<PcmClient>.
    read_cache: Mutex<std::collections::HashMap<String, String>>,
    /// Nested AGENTS.md content cache — dir → Some(content) once fetched.
    /// Missing entries mean not fetched yet (`nested_agents_paths` knows they exist).
    nested_agents: Mutex<std::collections::HashMap<String, String>>,
    /// Known nested AGENTS.md locations — dir → actual filename (case: "AGENTS.md" vs "AGENTS.MD").
    /// Populated by `preload_nested_agents()` (2 finds, no reads); content fetched on demand.
    nested_agents_paths: Mutex<std::collections::HashMap<String, String>>,
    /// Subtrees where nested AGENTS.md was already injected to LLM context — dedup by dir.
    /// Avoids repeated injection on every read within same subtree.
    injected_subtrees: Mutex<std::collections::HashSet<String>>,
    /// Total RPC calls to harness (= BitGN "steps" metric). Cache hits NOT counted.
    rpc_count: AtomicU32,
}

impl PcmClient {
    pub fn new(harness_url: &str) -> Self {
        let http = bitgn_sdk::make_http_client(harness_url);
        let config = bitgn_sdk::make_client_config(harness_url, None);
        Self {
            inner: PcmRuntimeClient::new(http, config),
            answer_submitted: AtomicBool::new(false),
            proposed_answer: Mutex::new(None),
            recent_reads: Mutex::new(Vec::new()),
            read_cache: Mutex::new(std::collections::HashMap::new()),
            nested_agents: Mutex::new(std::collections::HashMap::new()),
            nested_agents_paths: Mutex::new(std::collections::HashMap::new()),
            injected_subtrees: Mutex::new(std::collections::HashSet::new()),
            rpc_count: AtomicU32::new(0),
        }
    }

    fn err(method: &str, e: connectrpc::ConnectError) -> anyhow::Error {
        anyhow::anyhow!("PCM {} failed: {}", method, e)
    }

    /// Total harness RPC calls (= BitGN "steps" metric). Cache hits excluded.
    pub fn rpc_count(&self) -> u32 {
        self.rpc_count.load(Ordering::Relaxed)
    }

    /// Increment RPC counter (called on every actual harness call, not cache hits).
    fn count_rpc(&self) {
        self.rpc_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Cache-through helper: return cached value or fetch, cache, and return.
    async fn cached<F, Fut>(&self, key: &str, fetch: F) -> Result<String>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<String>>,
    {
        if let Ok(cache) = self.read_cache.lock() {
            if let Some(hit) = cache.get(key) {
                return Ok(hit.clone());
            }
        }
        self.count_rpc();
        let result = fetch().await?;
        if let Ok(mut cache) = self.read_cache.lock() {
            cache.insert(key.to_string(), result.clone());
        }
        Ok(result)
    }

    pub async fn tree(&self, root: &str, level: i32) -> Result<String> {
        let inner = &self.inner;
        let root_norm = root.trim_start_matches('/').trim_end_matches('/');
        self.cached(&format!("__tree__{root_norm}_{level}"), || async move {
            let resp = inner.tree(proto::TreeRequest {
                root: root.into(), level, ..Default::default()
            }).await.map_err(|e| Self::err("Tree", e))?;
            let v = resp.view();
            Ok(format!("$ tree -L {} {}\n{}", level, root, format_tree_entry(&v.root, "", true)))
        }).await
    }

    pub async fn list(&self, path: &str) -> Result<String> {
        let inner = &self.inner;
        // Normalize: strip leading/trailing slash so `cast` / `/cast` / `/cast/` share cache
        let norm = path.trim_start_matches('/').trim_end_matches('/');
        self.cached(&format!("__list__{norm}"), || async move {
            let resp = inner.list(proto::ListRequest {
                name: path.into(), ..Default::default()
            }).await.map_err(|e| Self::err("List", e))?;
            let v = resp.view();
            let mut out = format!("$ ls {}\n", path);
            for e in &v.entries {
                if e.is_dir { out.push_str(&format!("{}/\n", e.name)); }
                else { out.push_str(&format!("{}\n", e.name)); }
            }
            Ok(out)
        }).await
    }

    /// Silent read — like `read()` but does NOT track the path in `recent_reads`.
    /// Used by internal preloaders (nested AGENTS.md) so their paths don't pollute
    /// `AnswerTool` auto-refs. Still caches content normally.
    pub async fn read_silent(&self, path: &str) -> Result<String> {
        let norm = path.trim_start_matches('/');
        if let Ok(cache) = self.read_cache.lock()
            && let Some(cached) = cache.get(norm)
        {
            return Ok(cached.clone());
        }
        self.count_rpc();
        let resp = self.inner.read(proto::ReadRequest {
            path: path.into(), number: false, start_line: 0, end_line: 0, ..Default::default()
        }).await.map_err(|e| Self::err("Read", e))?;
        let v = resp.view();
        let result = format!("$ cat {}\n{}", path, v.content);
        if let Ok(mut c) = self.read_cache.lock() {
            c.insert(norm.to_string(), result.clone());
        }
        Ok(result)
    }

    pub async fn read(&self, path: &str, number: bool, start_line: i32, end_line: i32) -> Result<String> {
        // Cache: full-file reads (no line range, no numbering)
        let is_cacheable = start_line == 0 && end_line == 0 && !number;
        let norm = path.trim_start_matches('/');
        if is_cacheable {
            if let Ok(cache) = self.read_cache.lock() {
                if let Some(cached) = cache.get(norm) {
                    eprintln!("    📦 cache hit: {}", norm);
                    // Track cache hits for auto-refs (same as fresh reads)
                    if let Ok(mut reads) = self.recent_reads.lock() {
                        let p = norm.to_string();
                        if !reads.contains(&p) {
                            reads.push(p);
                            if reads.len() > 50 { reads.remove(0); }
                        }
                    }
                    return Ok(cached.clone());
                }
            }
        }

        self.count_rpc();
        let resp = self.inner.read(proto::ReadRequest {
            path: path.into(), number, start_line, end_line, ..Default::default()
        }).await.map_err(|e| Self::err("Read", e))?;
        let v = resp.view();
        let header = if start_line > 0 || end_line > 0 {
            format!("$ sed -n '{},{}p' {}", if start_line > 0 { start_line } else { 1 },
                if end_line > 0 { end_line.to_string() } else { "$".into() }, path)
        } else if number { format!("$ cat -n {}", path) }
        else { format!("$ cat {}", path) };
        let result = format!("{}\n{}", header, v.content);

        // Track read paths for auto-refs + cache
        if let Ok(mut reads) = self.recent_reads.lock() {
            let p = norm.to_string();
            if !reads.contains(&p) {
                reads.push(p);
                if reads.len() > 50 { reads.remove(0); }
            }
        }
        if is_cacheable {
            if let Ok(mut cache) = self.read_cache.lock() {
                cache.insert(norm.to_string(), result.clone());
            }
        }
        Ok(result)
    }

    /// Get recent read paths (for auto-refs in AnswerTool).
    pub fn recent_read_paths(&self) -> Vec<String> {
        self.recent_reads.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub async fn write(&self, path: &str, content: &str, start_line: i32, end_line: i32) -> Result<()> {
        if let Some(reason) = crate::policy::check_write(path) {
            anyhow::bail!("BLOCKED: '{}' is protected ({}) — cannot overwrite", path, reason);
        }
        // AI-NOTE: write dedup removed — copy_file needs write RPC even when content unchanged
        let norm = path.trim_start_matches('/');
        self.count_rpc();
        self.inner.write(proto::WriteRequest {
            path: path.into(), content: content.into(), start_line, end_line, ..Default::default()
        }).await.map_err(|e| Self::err("Write", e))?;
        // Invalidate read cache — file content changed
        if let Ok(mut cache) = self.read_cache.lock() {
            cache.remove(norm);
        }
        // Track written path for auto-refs (agent wrote this file → include in answer refs)
        if let Ok(mut reads) = self.recent_reads.lock() {
            let p = norm.to_string();
            if !reads.contains(&p) {
                reads.push(p);
                if reads.len() > 50 { reads.remove(0); }
            }
        }
        Ok(())
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        if let Some(reason) = crate::policy::check_write(path) {
            anyhow::bail!("BLOCKED: '{}' is protected ({}) — cannot delete", path, reason);
        }
        self.count_rpc();
        self.inner.delete(proto::DeleteRequest { path: path.into(), ..Default::default() })
            .await.map_err(|e| Self::err("Delete", e))?;
        if let Ok(mut cache) = self.read_cache.lock() {
            cache.remove(path.trim_start_matches('/'));
        }
        Ok(())
    }

    pub async fn mkdir(&self, path: &str) -> Result<()> {
        self.count_rpc();
        self.inner.mk_dir(proto::MkDirRequest { path: path.into(), ..Default::default() })
            .await.map_err(|e| Self::err("MkDir", e))?;
        Ok(())
    }

    pub async fn move_file(&self, from: &str, to: &str) -> Result<()> {
        self.count_rpc();
        self.inner.r#move(proto::MoveRequest {
            from_name: from.into(), to_name: to.into(), ..Default::default()
        }).await.map_err(|e| Self::err("Move", e))?;
        Ok(())
    }

    pub async fn find(&self, root: &str, name: &str, file_type: &str, limit: i32) -> Result<String> {
        let type_val = match file_type {
            "files" => proto::find_request::Type::TYPE_FILES,
            "dirs" => proto::find_request::Type::TYPE_DIRS,
            _ => proto::find_request::Type::TYPE_ALL,
        };
        self.count_rpc();
        let resp = self.inner.find(proto::FindRequest {
            root: root.into(), name: name.into(), r#type: type_val.into(), limit, ..Default::default()
        }).await.map_err(|e| Self::err("Find", e))?;
        let v = resp.view();
        let results: Vec<&str> = v.items.iter().map(|s| s.as_ref()).collect();
        Ok(format!("$ find {} -name '{}'\n{}", root, name, results.join("\n")))
    }

    pub async fn search(&self, root: &str, pattern: &str, limit: i32) -> Result<String> {
        self.count_rpc();
        let resp = self.inner.search(proto::SearchRequest {
            root: root.into(), pattern: pattern.into(), limit, ..Default::default()
        }).await.map_err(|e| Self::err("Search", e))?;
        let v = resp.view();
        let mut out = format!("$ rg -n --no-heading -e '{}' {}\n", pattern, root);
        for m in &v.matches {
            out.push_str(&format!("{}:{}:{}\n", m.path, m.line, m.line_text));
        }
        // Track search result paths for auto-refs (accounts, contacts, invoices)
        if let Ok(mut reads) = self.recent_reads.lock() {
            for m in &v.matches {
                let p = m.path.trim_start_matches('/').to_string();
                if crate::policy::is_auto_ref_path(&p) && !reads.contains(&p)
                {
                    reads.push(p);
                    if reads.len() > 50 { reads.remove(0); }
                }
            }
        }
        Ok(out)
    }

    /// Discover nested AGENTS.md locations — two parallel finds (lowercase + uppercase TLD
    /// mix in prod workspaces). Content is NOT read here — only paths cached for lazy fetch.
    /// Cheap: 2 find RPC, no reads. Returns number of nested subtrees discovered.
    pub async fn preload_nested_agents(&self) -> Result<usize> {
        let (lower, upper) = tokio::join!(
            self.find("/", "AGENTS.md", "files", 50),
            self.find("/", "AGENTS.MD", "files", 50),
        );
        let merged = format!("{}\n{}", lower.unwrap_or_default(), upper.unwrap_or_default());
        let mut map = self.nested_agents_paths.lock().unwrap();
        for line in merged.lines() {
            if line.is_empty() || line.starts_with("$ find") { continue; }
            let path = line.trim_start_matches('/').to_string();
            if path.is_empty() || path.eq_ignore_ascii_case("AGENTS.md") { continue; }
            let dir = match path.rsplit_once('/') {
                Some((d, _)) if !d.is_empty() => d.to_string(),
                _ => continue,
            };
            map.entry(dir).or_insert(path);
        }
        Ok(map.len())
    }

    /// Fetch nested AGENTS.md content for a dir (on demand). First call reads via silent path,
    /// subsequent calls hit the in-memory map. Returns None if dir has no nested AGENTS.md.
    pub async fn fetch_nested_agents(&self, dir: &str) -> Option<String> {
        if let Ok(map) = self.nested_agents.lock()
            && let Some(hit) = map.get(dir)
        {
            return Some(hit.clone());
        }
        let path = {
            let paths = self.nested_agents_paths.lock().ok()?;
            paths.get(dir).cloned()?
        };
        let content = self.read_silent(&path).await.ok()?;
        if let Ok(mut map) = self.nested_agents.lock() {
            map.insert(dir.to_string(), content.clone());
        }
        Some(content)
    }

    /// Return preloaded nested AGENTS.MD entries whose dir is an ancestor of any of `target_paths`.
    /// Per Model Spec §5: nested = local refinement, only applies within its subtree.
    /// Example: paths=["inbox/msg.md"] → returns `inbox/AGENTS.MD` (and any ancestor nested),
    /// but NOT sibling `accounts/AGENTS.md`.
    /// Sorted from shallowest to deepest dir (natural reading order).
    pub async fn relevant_nested_agents(&self, target_paths: &[&str]) -> Vec<(String, String)> {
        // Collect dirs on the ancestor chain where a nested AGENTS.md was discovered.
        let chain_dirs: Vec<String> = {
            let paths = match self.nested_agents_paths.lock() { Ok(m) => m, Err(_) => return Vec::new() };
            let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
            let mut out: Vec<String> = Vec::new();
            for raw in target_paths {
                let normalized = raw.trim_start_matches('/').trim_end_matches('/');
                let mut current: &str = normalized;
                loop {
                    if paths.contains_key(current) && seen.insert(current.to_string()) {
                        out.push(current.to_string());
                    }
                    match current.rsplit_once('/') {
                        Some((parent, _)) => current = parent,
                        None => break,
                    }
                }
            }
            out
        };
        // Fetch content lazily for each relevant dir (usually 0-2 reads).
        let mut out = Vec::with_capacity(chain_dirs.len());
        for dir in chain_dirs {
            if let Some(content) = self.fetch_nested_agents(&dir).await {
                out.push((dir, content));
            }
        }
        out.sort_by(|a, b| a.0.split('/').count().cmp(&b.0.split('/').count()).then_with(|| a.0.cmp(&b.0)));
        out
    }

    /// Mark a subtree's nested AGENTS.md as already injected into the LLM context.
    /// Returns true if this is the first mark (caller should inject), false if already seen.
    /// Separate from `nearest_nested_agents()` to keep the read API pure.
    pub fn mark_subtree_injected(&self, dir: &str) -> bool {
        match self.injected_subtrees.lock() {
            Ok(mut set) => set.insert(dir.to_string()),
            Err(_) => false,
        }
    }

    pub async fn context(&self) -> Result<String> {
        let inner = &self.inner;
        self.cached("__context__", || async move {
            let resp = inner.context(proto::ContextRequest::default())
                .await.map_err(|e| Self::err("Context", e))?;
            Ok(format!("$ date\n{}", resp.view().time))
        }).await
    }

    pub async fn answer(&self, message: &str, outcome: &str, refs: &[String]) -> Result<()> {
        let outcome_val = match outcome {
            "OUTCOME_OK" => proto::Outcome::OUTCOME_OK,
            "OUTCOME_DENIED_SECURITY" => proto::Outcome::OUTCOME_DENIED_SECURITY,
            "OUTCOME_NONE_CLARIFICATION" => proto::Outcome::OUTCOME_NONE_CLARIFICATION,
            "OUTCOME_NONE_UNSUPPORTED" => proto::Outcome::OUTCOME_NONE_UNSUPPORTED,
            "OUTCOME_ERR_INTERNAL" => proto::Outcome::OUTCOME_ERR_INTERNAL,
            _ => proto::Outcome::OUTCOME_OK,
        };
        self.count_rpc();
        self.inner.answer(proto::AnswerRequest {
            message: message.into(), outcome: outcome_val.into(), refs: refs.to_vec(), ..Default::default()
        }).await.map_err(|e| Self::err("Answer", e))?;
        self.answer_submitted.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Store a proposed answer without submitting via RPC.
    /// Overwrites any previously proposed answer.
    pub fn propose_answer(&self, message: &str, outcome: &str, refs: &[String]) {
        let mut guard = self.proposed_answer.lock().unwrap();
        *guard = Some(ProposedAnswer {
            message: message.to_string(),
            outcome: outcome.to_string(),
            refs: refs.to_vec(),
        });
        // Mark as submitted so auto_submit doesn't fire
        self.answer_submitted.store(true, Ordering::SeqCst);
    }

    /// Read the proposed answer (if any) without consuming it.
    pub fn get_proposed_answer(&self) -> Option<ProposedAnswer> {
        self.proposed_answer.lock().unwrap().clone()
    }

    /// Submit the proposed answer via RPC, or a given override.
    /// Falls back to error if nothing proposed and no override given.
    pub async fn submit_proposed(&self, override_outcome: Option<&str>) -> Result<()> {
        let proposed = self.proposed_answer.lock().unwrap().take();
        match proposed {
            Some(p) => {
                let outcome = override_outcome.unwrap_or(&p.outcome);
                self.answer_submitted.store(false, Ordering::SeqCst); // reset so answer() can set it
                self.answer(&p.message, outcome, &p.refs).await
            }
            None => anyhow::bail!("No proposed answer to submit"),
        }
    }
}

// AI-NOTE: FileBackend impl — enables sgr-agent-tools generic tools with PcmClient.
#[async_trait::async_trait]
impl sgr_agent_tools::FileBackend for PcmClient {
    async fn read(&self, path: &str, number: bool, start_line: i32, end_line: i32) -> Result<String> {
        self.read(path, number, start_line, end_line).await
    }
    async fn write(&self, path: &str, content: &str, start_line: i32, end_line: i32) -> Result<()> {
        self.write(path, content, start_line, end_line).await
    }
    async fn delete(&self, path: &str) -> Result<()> { self.delete(path).await }
    async fn search(&self, root: &str, pattern: &str, limit: i32) -> Result<String> {
        self.search(root, pattern, limit).await
    }
    async fn list(&self, path: &str) -> Result<String> { self.list(path).await }
    async fn tree(&self, root: &str, level: i32) -> Result<String> { self.tree(root, level).await }
    async fn context(&self) -> Result<String> { self.context().await }
    async fn mkdir(&self, path: &str) -> Result<()> { self.mkdir(path).await }
    async fn move_file(&self, from: &str, to: &str) -> Result<()> { self.move_file(from, to).await }
    async fn find(&self, root: &str, name: &str, file_type: &str, limit: i32) -> Result<String> {
        self.find(root, name, file_type, limit).await
    }
}

fn format_tree_entry(entry: &proto::tree_response::EntryView<'_>, prefix: &str, is_last: bool) -> String {
    let connector = if prefix.is_empty() { "" } else if is_last { "└── " } else { "├── " };
    let display = if entry.is_dir { format!("{}/", entry.name) } else { entry.name.to_string() };
    let mut result = format!("{}{}{}\n", prefix, connector, display);
    let child_prefix = if prefix.is_empty() { String::new() }
        else if is_last { format!("{}    ", prefix) }
        else { format!("{}│   ", prefix) };
    for (i, child) in entry.children.iter().enumerate() {
        result.push_str(&format_tree_entry(&child, &child_prefix, i == entry.children.len() - 1));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a PcmClient pointing at a dummy URL (RPC calls will fail, but propose/get work).
    fn dummy_pcm() -> PcmClient {
        PcmClient::new("http://localhost:0")
    }

    #[test]
    fn propose_stores_answer() {
        let pcm = dummy_pcm();
        assert!(pcm.get_proposed_answer().is_none());
        pcm.propose_answer("hello", "OUTCOME_OK", &[]);
        let p = pcm.get_proposed_answer().unwrap();
        assert_eq!(p.message, "hello");
        assert_eq!(p.outcome, "OUTCOME_OK");
        assert!(p.refs.is_empty());
    }

    #[test]
    fn propose_sets_answer_submitted() {
        let pcm = dummy_pcm();
        assert!(!pcm.answer_submitted.load(Ordering::SeqCst));
        pcm.propose_answer("msg", "OUTCOME_OK", &[]);
        assert!(pcm.answer_submitted.load(Ordering::SeqCst));
    }

    #[test]
    fn double_propose_overwrites() {
        let pcm = dummy_pcm();
        pcm.propose_answer("first", "OUTCOME_OK", &[]);
        pcm.propose_answer("second", "OUTCOME_DENIED_SECURITY", &["ref.md".into()]);
        let p = pcm.get_proposed_answer().unwrap();
        assert_eq!(p.message, "second");
        assert_eq!(p.outcome, "OUTCOME_DENIED_SECURITY");
        assert_eq!(p.refs, vec!["ref.md"]);
    }

    #[tokio::test]
    async fn submit_proposed_without_proposal_fails() {
        let pcm = dummy_pcm();
        assert!(pcm.submit_proposed(None).await.is_err());
    }
}
