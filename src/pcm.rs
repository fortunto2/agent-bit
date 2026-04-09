//! BitGN PcmRuntime client — typed Connect-RPC via bitgn-sdk.

use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
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
        }
    }

    fn err(method: &str, e: connectrpc::ConnectError) -> anyhow::Error {
        anyhow::anyhow!("PCM {} failed: {}", method, e)
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
        let result = fetch().await?;
        if let Ok(mut cache) = self.read_cache.lock() {
            cache.insert(key.to_string(), result.clone());
        }
        Ok(result)
    }

    pub async fn tree(&self, root: &str, level: i32) -> Result<String> {
        let inner = &self.inner;
        self.cached(&format!("__tree__{root}_{level}"), || async move {
            let resp = inner.tree(proto::TreeRequest {
                root: root.into(), level, ..Default::default()
            }).await.map_err(|e| Self::err("Tree", e))?;
            let v = resp.view();
            Ok(format!("$ tree -L {} {}\n{}", level, root, format_tree_entry(&v.root, "", true)))
        }).await
    }

    pub async fn list(&self, path: &str) -> Result<String> {
        let inner = &self.inner;
        self.cached(&format!("__list__{path}"), || async move {
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
        self.inner.write(proto::WriteRequest {
            path: path.into(), content: content.into(), start_line, end_line, ..Default::default()
        }).await.map_err(|e| Self::err("Write", e))?;
        // Invalidate read cache — file content changed
        if let Ok(mut cache) = self.read_cache.lock() {
            cache.remove(path.trim_start_matches('/'));
        }
        Ok(())
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        if let Some(reason) = crate::policy::check_write(path) {
            anyhow::bail!("BLOCKED: '{}' is protected ({}) — cannot delete", path, reason);
        }
        self.inner.delete(proto::DeleteRequest { path: path.into(), ..Default::default() })
            .await.map_err(|e| Self::err("Delete", e))?;
        if let Ok(mut cache) = self.read_cache.lock() {
            cache.remove(path.trim_start_matches('/'));
        }
        Ok(())
    }

    pub async fn mkdir(&self, path: &str) -> Result<()> {
        self.inner.mk_dir(proto::MkDirRequest { path: path.into(), ..Default::default() })
            .await.map_err(|e| Self::err("MkDir", e))?;
        Ok(())
    }

    pub async fn move_file(&self, from: &str, to: &str) -> Result<()> {
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
        let resp = self.inner.find(proto::FindRequest {
            root: root.into(), name: name.into(), r#type: type_val.into(), limit, ..Default::default()
        }).await.map_err(|e| Self::err("Find", e))?;
        let v = resp.view();
        let results: Vec<&str> = v.items.iter().map(|s| s.as_ref()).collect();
        Ok(format!("$ find {} -name '{}'\n{}", root, name, results.join("\n")))
    }

    pub async fn search(&self, root: &str, pattern: &str, limit: i32) -> Result<String> {
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
