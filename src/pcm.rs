//! BitGN PcmRuntime client — typed Connect-RPC via bitgn-sdk.
//!
//! Wraps the generated PcmRuntimeClient with shell-like output formatting
//! that the LLM expects (e.g. `$ cat file\ncontent`).

use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};

use bitgn_sdk::vm::pcm::{
    self as proto,
    PcmRuntimeClient,
};
use connectrpc::client::{HttpClient, ClientConfig};

pub struct PcmClient {
    inner: PcmRuntimeClient<HttpClient>,
    pub answer_submitted: AtomicBool,
}

impl PcmClient {
    pub fn new(harness_url: &str) -> Self {
        let url = harness_url.trim_end_matches('/');
        let http = if url.starts_with("https://") {
            let _ = connectrpc::rustls::crypto::ring::default_provider().install_default();
            let roots = connectrpc::rustls::RootCertStore::from_iter(
                webpki_roots::TLS_SERVER_ROOTS.iter().cloned()
            );
            let tls = connectrpc::rustls::ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth();
            HttpClient::with_tls(std::sync::Arc::new(tls))
        } else {
            HttpClient::plaintext()
        };
        let config = ClientConfig::new(url.parse().expect("invalid harness URL"));
        Self {
            inner: PcmRuntimeClient::new(http, config),
            answer_submitted: AtomicBool::new(false),
        }
    }

    fn err(method: &str, e: connectrpc::ConnectError) -> anyhow::Error {
        anyhow::anyhow!("PCM {} failed: {}", method, e)
    }

    pub async fn tree(&self, root: &str, level: i32) -> Result<String> {
        let resp = self.inner.tree(proto::TreeRequest {
            root: root.into(), level, ..Default::default()
        }).await.map_err(|e| Self::err("Tree", e))?;
        let v = resp.view();
        let tree_output = format_tree_entry(&v.root, "", true);
        Ok(format!("$ tree -L {} {}\n{}", level, root, tree_output))
    }

    pub async fn list(&self, path: &str) -> Result<String> {
        let resp = self.inner.list(proto::ListRequest {
            name: path.into(), ..Default::default()
        }).await.map_err(|e| Self::err("List", e))?;
        let v = resp.view();
        let mut out = format!("$ ls {}\n", path);
        for entry in &v.entries {
            if entry.is_dir {
                out.push_str(&format!("{}/\n", entry.name));
            } else {
                out.push_str(&format!("{}\n", entry.name));
            }
        }
        Ok(out)
    }

    pub async fn read(&self, path: &str, number: bool, start_line: i32, end_line: i32) -> Result<String> {
        let resp = self.inner.read(proto::ReadRequest {
            path: path.into(), number, start_line, end_line, ..Default::default()
        }).await.map_err(|e| Self::err("Read", e))?;
        let v = resp.view();
        let header = if start_line > 0 || end_line > 0 {
            format!("$ sed -n '{},{}p' {}", if start_line > 0 { start_line } else { 1 },
                if end_line > 0 { end_line.to_string() } else { "$".into() }, path)
        } else if number {
            format!("$ cat -n {}", path)
        } else {
            format!("$ cat {}", path)
        };
        Ok(format!("{}\n{}", header, v.content))
    }

    pub async fn write(&self, path: &str, content: &str, start_line: i32, end_line: i32) -> Result<()> {
        self.inner.write(proto::WriteRequest {
            path: path.into(), content: content.into(), start_line, end_line, ..Default::default()
        }).await.map_err(|e| Self::err("Write", e))?;
        Ok(())
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        self.inner.delete(proto::DeleteRequest {
            path: path.into(), ..Default::default()
        }).await.map_err(|e| Self::err("Delete", e))?;
        Ok(())
    }

    pub async fn mkdir(&self, path: &str) -> Result<()> {
        self.inner.mk_dir(proto::MkDirRequest {
            path: path.into(), ..Default::default()
        }).await.map_err(|e| Self::err("MkDir", e))?;
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
        Ok(out)
    }

    pub async fn context(&self) -> Result<String> {
        let resp = self.inner.context(proto::ContextRequest::default())
            .await.map_err(|e| Self::err("Context", e))?;
        let v = resp.view();
        Ok(format!("$ date\n{}", v.time))
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
            message: message.into(), outcome: outcome_val.into(), refs: refs.to_vec(),
            ..Default::default()
        }).await.map_err(|e| Self::err("Answer", e))?;
        self.answer_submitted.store(true, Ordering::SeqCst);
        Ok(())
    }
}

/// Format tree entry recursively.
fn format_tree_entry(entry: &proto::tree_response::EntryView<'_>, prefix: &str, is_last: bool) -> String {
    let name = &entry.name;
    let is_dir = entry.is_dir;

    let connector = if prefix.is_empty() { "" } else if is_last { "└── " } else { "├── " };
    let display_name = if is_dir { format!("{}/", name) } else { name.to_string() };
    let mut result = format!("{}{}{}\n", prefix, connector, display_name);

    let child_prefix = if prefix.is_empty() {
        String::new()
    } else if is_last {
        format!("{}    ", prefix)
    } else {
        format!("{}│   ", prefix)
    };

    for (i, child) in entry.children.iter().enumerate() {
        let is_last_child = i == entry.children.len() - 1;
        result.push_str(&format_tree_entry(&child, &child_prefix, is_last_child));
    }
    result
}
