//! Pangolin experiment: Python via subprocess (python3.13) — zero heavy deps.
//!
//! Architecture: each `execute_code` spawns `python3 -c <prelude+user>`. The
//! Python prelude imports `workspace.py`, which talks to the Rust parent over
//! stdin/stdout JSON-RPC. Rust routes each `ws.read/write/...` to the shared
//! PcmClient (same channel as main agent — one source of truth).
//!
//! State between calls is JSON on disk (scratchpad + user globals). Same
//! pattern as original Pangolin (TypeScript host, Python subprocess, temp
//! files for persistence).
//!
//! # Status: scaffold PoC
//! Host methods: ws.read / ws.write / ws.delete / ws.list / ws.search / ws.find /
//! ws.tree / ws.move / ws.context / ws.answer. Binary cost: zero (uses system python3).
//!
//! Runtime prerequisite: a Python 3.10+ interpreter. Default: `python3`.
//! Override via `PANGOLIN_PY_CMD` env var, e.g. `uv run python3` or `python3.13`.
//! Uses only stdlib (json, datetime, collections, pathlib) — no venv required.

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;

use anyhow::{Context as _, Result};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sgr_agent::agent_tool::{Tool, ToolError, ToolOutput, parse_args};
use sgr_agent::context::AgentContext;
use sgr_agent::schema::json_schema_for;

use crate::pcm::PcmClient;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnswerPayload {
    pub message: String,
    pub outcome: String,
    pub refs: Vec<String>,
}

pub struct PangolinSession {
    pub pcm: Arc<PcmClient>,
    pub scratchpad_path: PathBuf,
    pub answer_path: PathBuf,
    pub state_path: PathBuf,
    pub answer: std::sync::Mutex<Option<AnswerPayload>>,
    pub refs_tracking: std::sync::Mutex<Vec<String>>,
}

impl PangolinSession {
    pub fn new(pcm: Arc<PcmClient>, tag: &str) -> Arc<Self> {
        let tmp = std::env::temp_dir();
        Self::init(pcm, tmp, tag, json!({ "refs": [] }))
    }

    pub fn with_context(pcm: Arc<PcmClient>, ctx: Value, tag: &str) -> Arc<Self> {
        let tmp = std::env::temp_dir();
        Self::init(pcm, tmp, tag, json!({ "refs": [], "context": ctx }))
    }

    fn init(pcm: Arc<PcmClient>, tmp: PathBuf, tag: &str, seed: Value) -> Arc<Self> {
        let pid = std::process::id();
        let scratchpad_path = tmp.join(format!("pangolin-py-{tag}-{pid}-sp.json"));
        let answer_path = tmp.join(format!("pangolin-py-{tag}-{pid}-ans.json"));
        let state_path = tmp.join(format!("pangolin-py-{tag}-{pid}-state.json"));
        // Seed scratchpad; clear answer + state.
        std::fs::write(&scratchpad_path, serde_json::to_string(&seed).unwrap_or("{}".into()))
            .ok();
        let _ = std::fs::remove_file(&answer_path);
        let _ = std::fs::write(&state_path, "{}");
        Arc::new(Self {
            pcm,
            scratchpad_path,
            answer_path,
            state_path,
            answer: std::sync::Mutex::new(None),
            refs_tracking: std::sync::Mutex::new(Vec::new()),
        })
    }

    pub fn scratchpad_snapshot(&self) -> Value {
        std::fs::read_to_string(&self.scratchpad_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(Value::Null)
    }

    pub fn take_answer(&self) -> Option<AnswerPayload> {
        // Prefer fresh file on disk (Python wrote it), fall back to cached.
        let disk = std::fs::read_to_string(&self.answer_path).ok().and_then(|s| {
            serde_json::from_str::<Value>(&s).ok().map(|v| AnswerPayload {
                message: v.get("message").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                outcome: v.get("outcome").and_then(|x| x.as_str()).unwrap_or("OUTCOME_OK").to_string(),
                refs: v.get("refs").and_then(|x| x.as_array())
                    .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                    .unwrap_or_default(),
            })
        });
        if let Some(a) = disk {
            let _ = std::fs::remove_file(&self.answer_path);
            return Some(a);
        }
        self.answer.lock().unwrap().take()
    }

    pub fn cleanup(&self) {
        let _ = std::fs::remove_file(&self.scratchpad_path);
        let _ = std::fs::remove_file(&self.answer_path);
        let _ = std::fs::remove_file(&self.state_path);
    }
}

/// Route one ws.* RPC line from Python to PcmClient.
/// Returns JSON reply that the Python side will deserialize.
async fn handle_rpc(session: &Arc<PangolinSession>, req: Value) -> Value {
    let method = req.get("method").and_then(|v| v.as_str()).unwrap_or("");
    let pcm = &session.pcm;
    match method {
        "read" => {
            let path = req.get("path").and_then(|v| v.as_str()).unwrap_or("");
            match pcm.read(path, false, 0, 0).await {
                Ok(content) => {
                    let mut r = session.refs_tracking.lock().unwrap();
                    if !r.contains(&path.to_string()) { r.push(path.to_string()); }
                    let stripped = content.split_once('\n').map(|(_, r)| r).unwrap_or(&content).to_string();
                    json!({ "content": stripped, "raw": content })
                }
                Err(e) => json!({ "error": e.to_string() }),
            }
        }
        "write" => {
            let path = req.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = req.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let sl = req.get("start_line").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let el = req.get("end_line").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            match pcm.write(path, content, sl, el).await {
                Ok(_) => {
                    let mut r = session.refs_tracking.lock().unwrap();
                    if !r.contains(&path.to_string()) { r.push(path.to_string()); }
                    json!("ok")
                }
                Err(e) => json!({ "error": e.to_string() }),
            }
        }
        "delete" => {
            let path = req.get("path").and_then(|v| v.as_str()).unwrap_or("");
            match pcm.delete(path).await { Ok(_) => json!("ok"), Err(e) => json!({ "error": e.to_string() }) }
        }
        "list" => {
            let path = req.get("path").and_then(|v| v.as_str()).unwrap_or("/");
            match pcm.list(path).await {
                Ok(text) => {
                    let entries: Vec<Value> = text.lines().skip(1)
                        .filter(|l| !l.trim().is_empty())
                        .map(|l| json!({ "name": l.trim() })).collect();
                    json!({ "entries": entries })
                }
                Err(e) => json!({ "error": e.to_string() }),
            }
        }
        "search" => {
            let root = req.get("root").and_then(|v| v.as_str()).unwrap_or("/");
            let pattern = req.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            let lim = req.get("limit").and_then(|v| v.as_i64()).unwrap_or(10).max(1) as i32;
            match pcm.search(root, pattern, lim).await {
                Ok(text) => {
                    let matches: Vec<Value> = text.lines().filter_map(|l| {
                        let mut it = l.splitn(3, ':');
                        let p = it.next()?.to_string();
                        let ln: i64 = it.next()?.parse().ok()?;
                        let t = it.next().unwrap_or("").to_string();
                        Some(json!({ "path": p, "line": ln, "lineText": t }))
                    }).collect();
                    json!({ "matches": matches })
                }
                Err(e) => json!({ "error": e.to_string() }),
            }
        }
        "find" => {
            let root = req.get("root").and_then(|v| v.as_str()).unwrap_or("/");
            let name = req.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let kind = req.get("kind").and_then(|v| v.as_str()).unwrap_or("all");
            let lim = req.get("limit").and_then(|v| v.as_i64()).unwrap_or(10).max(1) as i32;
            match pcm.find(root, name, kind, lim).await {
                Ok(text) => {
                    let entries: Vec<Value> = text.lines().skip(1)
                        .filter(|l| !l.trim().is_empty())
                        .map(|l| json!({ "name": l.trim() })).collect();
                    json!({ "entries": entries })
                }
                Err(e) => json!({ "error": e.to_string() }),
            }
        }
        "tree" => {
            let root = req.get("root").and_then(|v| v.as_str()).unwrap_or("/");
            let lvl = req.get("level").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            match pcm.tree(root, lvl).await {
                Ok(text) => json!({ "tree": text }),
                Err(e) => json!({ "error": e.to_string() }),
            }
        }
        "move" => {
            let from = req.get("from").and_then(|v| v.as_str()).unwrap_or("");
            let to = req.get("to").and_then(|v| v.as_str()).unwrap_or("");
            match pcm.move_file(from, to).await { Ok(_) => json!("ok"), Err(e) => json!({ "error": e.to_string() }) }
        }
        "context" => match pcm.context().await {
            Ok(text) => parse_context_text(&text),
            Err(e) => json!({ "error": e.to_string() }),
        },
        _ => json!({ "error": format!("unknown method: {method}") }),
    }
}

pub fn parse_context_output(text: &str) -> Value { parse_context_text(text) }
fn parse_context_text(text: &str) -> Value {
    let time = text.lines().nth(1).unwrap_or(text).trim().to_string();
    let unix = chrono::DateTime::parse_from_rfc3339(&time).ok().map(|dt| dt.timestamp());
    match unix {
        Some(u) => json!({ "time": time, "unixTime": u }),
        None => json!({ "time": time }),
    }
}

/// Execute one Python `code` block as a fresh subprocess.
/// Blocks waiting for RPC loop to finish; every ws.* call routes to pcm.
pub async fn run_py(code: &str, session: Arc<PangolinSession>) -> String {
    // Prelude runs in `__main__` globals so user variables persist.
    // - import workspace.py for `ws` + `scratchpad`
    // - load user-variable state from disk into __main__ globals
    // - atexit handler saves JSON-serializable globals back to disk
    let prelude = r#"
import sys, os, json, atexit, re, math, hashlib, base64  # noqa: F401
from datetime import datetime, timedelta, date  # noqa: F401
from collections import defaultdict, Counter  # noqa: F401
from pathlib import PurePosixPath  # noqa: F401
sys.path.insert(0, "python")
from workspace import ws, scratchpad  # noqa: F401

_STATE_PATH = os.environ.get("AGENT_STATE_PATH", "/tmp/pangolin-py-state.json")
try:
    with open(_STATE_PATH) as _f:
        globals().update({k: v for k, v in json.load(_f).items() if k not in ('ws', 'scratchpad')})
except (FileNotFoundError, json.JSONDecodeError):
    pass

_PRELUDE_KEYS = set(globals().keys())

def _save_main_state():
    out = {}
    for k, v in list(globals().items()):
        if k.startswith('_') or k in _PRELUDE_KEYS or k in ('ws', 'scratchpad'):
            continue
        try:
            json.dumps(v)
            out[k] = v
        except (TypeError, ValueError):
            pass
    try:
        with open(_STATE_PATH, "w") as _f:
            json.dump(out, _f)
    except OSError:
        pass

atexit.register(_save_main_state)
"#;
    let full = format!("{prelude}\n# --- user code ---\n{code}");

    // Support `PANGOLIN_PY_CMD="uv run python3"` / `"python3.13"` / etc.
    let py_cmd = std::env::var("PANGOLIN_PY_CMD").unwrap_or_else(|_| "python3".into());
    let parts: Vec<&str> = py_cmd.split_whitespace().collect();
    let (bin, extra_args) = parts.split_first().unwrap_or((&"python3", &[]));
    let child_res = Command::new(bin)
        .args(extra_args.iter().copied())
        .arg("-c").arg(&full)
        .env("AGENT_SCRATCHPAD_PATH", &session.scratchpad_path)
        .env("AGENT_ANSWER_PATH", &session.answer_path)
        .env("AGENT_STATE_PATH", &session.state_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match child_res {
        Ok(c) => c,
        Err(e) => return format!("subprocess spawn failed: {e}"),
    };

    let mut stdin = match child.stdin.take() { Some(s) => s, None => return "stdin closed".into() };
    let stdout = match child.stdout.take() { Some(s) => s, None => return "stdout closed".into() };
    let mut out = BufReader::new(stdout);

    let mut captured = String::new();
    // RPC loop: read stdout; lines starting with __RPC__ are JSON requests,
    // anything else is passed through to captured output.
    loop {
        let mut line = String::new();
        match out.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }
        if let Some(rest) = line.strip_prefix("__RPC__ ") {
            let req: Value = serde_json::from_str(rest.trim()).unwrap_or(Value::Null);
            let reply = handle_rpc(&session, req).await;
            let _ = writeln!(stdin, "{}", reply);
            let _ = stdin.flush();
        } else {
            captured.push_str(&line);
        }
    }
    drop(stdin);
    let status = child.wait();
    let mut stderr = String::new();
    if let Some(mut err) = child.stderr { use std::io::Read; let _ = err.read_to_string(&mut stderr); }
    if let Ok(s) = status {
        if !s.success() {
            if !captured.is_empty() { captured.push('\n'); }
            captured.push_str(&format!("(python exit {}): {}", s.code().unwrap_or(-1), stderr.trim()));
        }
    }
    if captured.is_empty() { "(no output)".into() } else { captured.trim_end().to_string() }
}

pub struct ExecutePyTool {
    pub session: Arc<PangolinSession>,
}

#[derive(Deserialize, JsonSchema)]
struct ExecuteArgs {
    /// Python 3 code. Uses `ws.read/write/delete/list/search/find/tree/move/context/answer`.
    /// `scratchpad` dict persists across calls. Call `ws.answer({answer, outcome, refs})` to submit.
    code: String,
}

#[async_trait]
impl Tool for ExecutePyTool {
    fn name(&self) -> &str { "execute_code" }
    fn description(&self) -> &str {
        "Execute Python 3. Pre-loaded: ws (workspace), scratchpad (persistent dict), \
         json/re/math/datetime/timedelta/defaultdict/Counter/PurePosixPath. \
         Methods: ws.read(path), ws.write(path, content, start_line=0, end_line=0), \
         ws.delete(path), ws.list(path), ws.search(root, pattern, limit), \
         ws.find(root, name, kind, limit), ws.tree(root, level), ws.move(from, to), \
         ws.context() → {time, unixTime}, ws.answer({answer, outcome, refs}) to submit."
    }
    fn is_read_only(&self) -> bool { false }
    fn parameters_schema(&self) -> Value { json_schema_for::<ExecuteArgs>() }
    async fn execute(&self, args: Value, _ctx: &mut AgentContext) -> Result<ToolOutput, ToolError> {
        let a: ExecuteArgs = parse_args(&args)?;
        let output = run_py(&a.code, self.session.clone()).await;
        Ok(ToolOutput::text(output))
    }
    async fn execute_readonly(&self, args: Value, ctx: &AgentContext) -> Result<ToolOutput, ToolError> {
        let mut ctx = ctx.clone();
        self.execute(args, &mut ctx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn py_scratchpad_and_answer() {
        // Needs python3 on PATH. If not, skip gracefully.
        if Command::new("python3").arg("--version").output().is_err() { return; }
        let pcm = Arc::new(PcmClient::new("http://127.0.0.1:1"));
        let session = PangolinSession::new(pcm, "test");

        run_py(
            "scratchpad['step1'] = 'done'\nscratchpad['counter'] = 1",
            session.clone(),
        ).await;
        let sp = session.scratchpad_snapshot();
        assert_eq!(sp["step1"], "done");
        assert_eq!(sp["counter"], 1);

        run_py(
            "scratchpad['counter'] += 5\nws.answer({'answer': 'hello', 'outcome': 'OUTCOME_OK', 'refs': ['/a.md']})",
            session.clone(),
        ).await;
        let sp = session.scratchpad_snapshot();
        assert_eq!(sp["counter"], 6);
        let ans = session.take_answer().expect("ws.answer captured");
        assert_eq!(ans.message, "hello");
        assert_eq!(ans.outcome, "OUTCOME_OK");
        assert_eq!(ans.refs, vec!["/a.md".to_string()]);
        session.cleanup();
    }
}
