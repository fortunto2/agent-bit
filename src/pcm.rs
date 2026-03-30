//! BitGN PcmRuntime client — virtual file system operations via Connect-RPC.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct PcmClient {
    client: reqwest::Client,
    base_url: String,
    pub answer_submitted: AtomicBool,
}

impl PcmClient {
    pub fn new(harness_url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: harness_url.trim_end_matches('/').to_string(),
            answer_submitted: AtomicBool::new(false),
        }
    }

    async fn call(&self, method: &str, body: &Value) -> Result<Value> {
        let url = format!("{}/bitgn.vm.pcm.PcmRuntime/{}", self.base_url, method);
        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .context(format!("PCM {}", method))?;

        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() {
            bail!("PCM {} failed ({}): {}", method, status, text);
        }

        if std::env::var("PAC1_DEBUG").is_ok() {
            let preview = if text.len() > 300 { &text[..300] } else { &text };
            eprintln!("      [pcm] {} → {}", method, preview);
        }

        serde_json::from_str(&text).context(format!("parse PCM {} response", method))
    }

    pub async fn tree(&self, root: &str, level: i32) -> Result<String> {
        let resp = self
            .call("Tree", &json!({"root": root, "level": level}))
            .await?;
        Ok(format_tree_entry(&resp["root"], "", true))
    }

    pub async fn list(&self, path: &str) -> Result<String> {
        let resp = self.call("List", &json!({"name": path})).await?;
        let entries = resp["entries"]
            .as_array()
            .map(|a| a.as_slice())
            .unwrap_or_default();
        let mut out = String::new();
        for entry in entries {
            let name = entry["name"].as_str().unwrap_or("");
            let is_dir = entry["isDir"].as_bool().unwrap_or(false);
            if is_dir {
                out.push_str(&format!("{}/\n", name));
            } else {
                out.push_str(&format!("{}\n", name));
            }
        }
        Ok(out)
    }

    pub async fn read(
        &self,
        path: &str,
        number: bool,
        start_line: i32,
        end_line: i32,
    ) -> Result<String> {
        let mut body = json!({"path": path});
        if number {
            body["number"] = Value::Bool(true);
        }
        if start_line > 0 {
            body["startLine"] = Value::Number(start_line.into());
        }
        if end_line > 0 {
            body["endLine"] = Value::Number(end_line.into());
        }

        let resp = self.call("Read", &body).await?;
        Ok(resp["content"].as_str().unwrap_or("").to_string())
    }

    pub async fn write(
        &self,
        path: &str,
        content: &str,
        start_line: i32,
        end_line: i32,
    ) -> Result<()> {
        let mut body = json!({"path": path, "content": content});
        if start_line > 0 {
            body["startLine"] = Value::Number(start_line.into());
        }
        if end_line > 0 {
            body["endLine"] = Value::Number(end_line.into());
        }

        self.call("Write", &body).await?;
        Ok(())
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        self.call("Delete", &json!({"path": path})).await?;
        Ok(())
    }

    pub async fn mkdir(&self, path: &str) -> Result<()> {
        self.call("MkDir", &json!({"path": path})).await?;
        Ok(())
    }

    pub async fn move_file(&self, from: &str, to: &str) -> Result<()> {
        self.call("Move", &json!({"fromName": from, "toName": to}))
            .await?;
        Ok(())
    }

    pub async fn find(&self, root: &str, name: &str, file_type: &str, limit: i32) -> Result<String> {
        let mut body = json!({"root": root, "name": name});
        if !file_type.is_empty() {
            let type_val = match file_type {
                "files" => "TYPE_FILES",
                "dirs" => "TYPE_DIRS",
                _ => "TYPE_ALL",
            };
            body["type"] = Value::String(type_val.to_string());
        }
        if limit > 0 {
            body["limit"] = Value::Number(limit.into());
        }

        let resp = self.call("Find", &body).await?;
        let items = resp["items"]
            .as_array()
            .map(|a| a.as_slice())
            .unwrap_or_default();
        Ok(items
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join("\n"))
    }

    pub async fn search(&self, root: &str, pattern: &str, limit: i32) -> Result<String> {
        let mut body = json!({"root": root, "pattern": pattern});
        if limit > 0 {
            body["limit"] = Value::Number(limit.into());
        }

        let resp = self.call("Search", &body).await?;
        let matches = resp["matches"]
            .as_array()
            .map(|a| a.as_slice())
            .unwrap_or_default();
        let mut out = String::new();
        for m in matches {
            let path = m["path"].as_str().unwrap_or("");
            let line = m["line"].as_i64().unwrap_or(0);
            let text = m["lineText"].as_str().unwrap_or("");
            out.push_str(&format!("{}:{}:{}\n", path, line, text));
        }
        Ok(out)
    }

    pub async fn context(&self) -> Result<String> {
        let resp = self.call("Context", &json!({})).await?;
        Ok(resp["time"].as_str().unwrap_or("").to_string())
    }

    pub async fn answer(&self, message: &str, outcome: &str, refs: &[String]) -> Result<()> {
        self.call(
            "Answer",
            &json!({
                "message": message,
                "outcome": outcome,
                "refs": refs,
            }),
        )
        .await?;
        self.answer_submitted.store(true, Ordering::SeqCst);
        Ok(())
    }
}

fn format_tree_entry(entry: &Value, prefix: &str, is_last: bool) -> String {
    let name = entry["name"].as_str().unwrap_or("");
    let is_dir = entry["isDir"].as_bool().unwrap_or(false);
    let children = entry["children"].as_array();

    let connector = if prefix.is_empty() {
        ""
    } else if is_last {
        "└── "
    } else {
        "├── "
    };
    let display_name = if is_dir {
        format!("{}/", name)
    } else {
        name.to_string()
    };

    let mut result = format!("{}{}{}\n", prefix, connector, display_name);

    if let Some(children) = children {
        let child_prefix = if prefix.is_empty() {
            String::new()
        } else if is_last {
            format!("{}    ", prefix)
        } else {
            format!("{}│   ", prefix)
        };

        for (i, child) in children.iter().enumerate() {
            let is_last_child = i == children.len() - 1;
            result.push_str(&format_tree_entry(child, &child_prefix, is_last_child));
        }
    }

    result
}
