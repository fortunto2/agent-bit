//! `eval` tool with FULL workspace access — Pangolin-style `execute_code` as a
//! 17th tool alongside the 16 atomic FC tools.
//!
//! Gives JS live access to PcmClient: `ws_read/write/delete/list/search/find/
//! tree/move/context`. Writes/deletes still pass through PcmClient, so policy.rs
//! guards on protected paths fire normally — no sandbox escape vs atomic tools.
//!
//! Use case: batch aggregation (sum N bills), multi-file transforms (OCR 5
//! invoices in one block), filtering (oldest record from sender X). The LLM
//! stays in atomic-tool mode for security/decision work; switches to `eval`
//! when it sees a deterministic multi-file compute pattern.
//!
//! # Architecture
//! - JS runs in a dedicated OS thread per call (Boa Context is !Send, !Sync).
//! - Host fns use a thread-local shared state for PcmClient + tokio Handle.
//! - `scratchpad` JSON object persists across `eval` calls within one trial
//!   via `EvalSession`. Lets the agent accumulate state across steps.
//!
//! Ported from `experiment/pangolin-arch` src/pangolin.rs. `ws_answer` removed
//! (main agent uses its own AnswerTool for final submission).

use std::cell::RefCell;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use sgr_agent::agent_tool::{Tool, ToolError, ToolOutput, parse_args};
use sgr_agent::context::AgentContext;
use sgr_agent::schema::json_schema_for;

use crate::pcm::PcmClient;

// ─── Shared state between tool calls ────────────────────────────────────────

/// State that persists across multiple `eval` invocations within one task.
/// `scratchpad` is `Arc<Mutex<_>>` so the same handle can be cloned into the
/// agent side for cross-tool `<scratchpad>` injection.
pub struct EvalSession {
    pub pcm: Arc<PcmClient>,
    pub scratchpad: Arc<std::sync::Mutex<Value>>,
    pub refs_tracking: std::sync::Mutex<Vec<String>>,
    pub log_buffer: std::sync::Mutex<Vec<String>>,
}

impl EvalSession {
    pub fn new(pcm: Arc<PcmClient>) -> Arc<Self> {
        Arc::new(Self {
            pcm,
            scratchpad: Arc::new(std::sync::Mutex::new(json!({ "refs": [] }))),
            refs_tracking: std::sync::Mutex::new(Vec::new()),
            log_buffer: std::sync::Mutex::new(Vec::new()),
        })
    }

    pub fn with_context(pcm: Arc<PcmClient>, context_json: Value) -> Arc<Self> {
        Arc::new(Self {
            pcm,
            scratchpad: Arc::new(std::sync::Mutex::new(json!({ "refs": [], "context": context_json }))),
            refs_tracking: std::sync::Mutex::new(Vec::new()),
            log_buffer: std::sync::Mutex::new(Vec::new()),
        })
    }

    pub fn scratchpad_snapshot(&self) -> Value {
        self.scratchpad.lock().unwrap().clone()
    }

    pub fn tracked_refs(&self) -> Vec<String> {
        self.refs_tracking.lock().unwrap().clone()
    }
}

/// Extract the workspace timestamp from `pcm.context()` output. PcmClient returns
/// `"$ date\n<RFC3339>"`; we publish it as `{time, unixTime}` so JS can read
/// `ws_context().time` and `scratchpad.context.time` uniformly (mirrors Pangolin
/// original: `scratchpad.context = { time, unixTime }`).
pub fn parse_context_output(text: &str) -> Value {
    parse_context_text(text)
}

fn parse_context_text(text: &str) -> Value {
    // PCM context output: "$ date\n<RFC3339 time>\n". Second line is the time.
    let time = text.lines().nth(1).unwrap_or(text).trim().to_string();
    let unix = chrono::DateTime::parse_from_rfc3339(&time)
        .ok()
        .map(|dt| dt.timestamp());
    match unix {
        Some(u) => json!({ "time": time, "unixTime": u }),
        None => json!({ "time": time }),
    }
}

// ─── Thread-local handoff so NativeFunctionPointer fns can reach state ──────

thread_local! {
    static CURRENT: RefCell<Option<Arc<EvalSession>>> = const { RefCell::new(None) };
    static TOKIO_HANDLE: RefCell<Option<tokio::runtime::Handle>> = const { RefCell::new(None) };
}

fn with_state<F, R>(f: F) -> R
where
    F: FnOnce(&Arc<EvalSession>, &tokio::runtime::Handle) -> R,
{
    CURRENT.with(|c| {
        TOKIO_HANDLE.with(|h| {
            let c_ref = c.borrow();
            let h_ref = h.borrow();
            let state = c_ref.as_ref().expect("EvalSession not installed on this thread");
            let handle = h_ref.as_ref().expect("tokio Handle not installed on this thread");
            f(state, handle)
        })
    })
}

// ─── Host functions (sync, block_on async PcmClient) ────────────────────────

mod host {
    use super::*;
    use boa_engine::{Context, JsResult, JsString, JsValue, Source, js_string};

    fn arg_string(args: &[JsValue], i: usize, ctx: &mut Context) -> JsResult<String> {
        args.get(i)
            .ok_or_else(|| boa_engine::JsNativeError::typ().with_message("missing arg").into())
            .and_then(|v| v.to_string(ctx))
            .map(|s| s.to_std_string_escaped())
    }

    fn arg_opt_i32(args: &[JsValue], i: usize, ctx: &mut Context) -> JsResult<i32> {
        match args.get(i) {
            None => Ok(0),
            Some(v) if v.is_undefined() || v.is_null() => Ok(0),
            Some(v) => v.to_i32(ctx),
        }
    }

    fn json_to_jsvalue(v: &Value, ctx: &mut Context) -> JsResult<JsValue> {
        // Value→string→JsValue (use llm_json repair as fallback — cheap safety net).
        let s = serde_json::to_string(v).unwrap_or_else(|_| "null".into());
        JsValue::from_json(&sgr_agent::str_ext::parse_tool_args(&s), ctx)
    }

    fn err_to_jsval(e: impl std::fmt::Display, ctx: &mut Context) -> JsValue {
        json_to_jsvalue(&json!({ "error": e.to_string() }), ctx).unwrap_or(JsValue::null())
    }

    pub fn ws_read(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let path = arg_string(args, 0, ctx)?;
        let result = with_state(|s, h| h.block_on(s.pcm.read(&path, false, 0, 0)));
        match result {
            Ok(content) => {
                with_state(|s, _| {
                    let mut r = s.refs_tracking.lock().unwrap();
                    if !r.contains(&path) { r.push(path.clone()); }
                });
                // Strip "$ cat …\n" header if present.
                let stripped = content.split_once('\n').map(|(_, rest)| rest).unwrap_or(&content).to_string();
                let out = json!({ "content": stripped, "raw": content });
                json_to_jsvalue(&out, ctx)
            }
            Err(e) => Ok(err_to_jsval(e, ctx)),
        }
    }

    pub fn ws_list(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let path = arg_string(args, 0, ctx).unwrap_or_else(|_| "/".into());
        let result = with_state(|s, h| h.block_on(s.pcm.list(&path)));
        match result {
            Ok(lines) => {
                let entries: Vec<Value> = lines.lines().skip(1)
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| json!({ "name": l.trim() }))
                    .collect();
                json_to_jsvalue(&json!({ "entries": entries }), ctx)
            }
            Err(e) => Ok(err_to_jsval(e, ctx)),
        }
    }

    pub fn ws_search(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let root = arg_string(args, 0, ctx).unwrap_or_else(|_| "/".into());
        let pattern = arg_string(args, 1, ctx)?;
        let limit = arg_opt_i32(args, 2, ctx).unwrap_or(10).max(1);
        let result = with_state(|s, h| h.block_on(s.pcm.search(&root, &pattern, limit)));
        match result {
            Ok(text) => {
                let matches: Vec<Value> = text.lines()
                    .filter_map(|l| {
                        let mut it = l.splitn(3, ':');
                        let p = it.next()?.to_string();
                        let ln: i64 = it.next()?.parse().ok()?;
                        let t = it.next().unwrap_or("").to_string();
                        Some(json!({ "path": p, "line": ln, "lineText": t }))
                    })
                    .collect();
                json_to_jsvalue(&json!({ "matches": matches }), ctx)
            }
            Err(e) => Ok(err_to_jsval(e, ctx)),
        }
    }

    pub fn ws_write(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let path = arg_string(args, 0, ctx)?;
        let content = arg_string(args, 1, ctx)?;
        let start_line = arg_opt_i32(args, 2, ctx).unwrap_or(0);
        let end_line = arg_opt_i32(args, 3, ctx).unwrap_or(0);
        let result = with_state(|s, h| h.block_on(s.pcm.write(&path, &content, start_line, end_line)));
        match result {
            Ok(_) => {
                with_state(|s, _| {
                    let mut r = s.refs_tracking.lock().unwrap();
                    if !r.contains(&path) { r.push(path.clone()); }
                });
                Ok(JsValue::from(js_string!("ok")))
            }
            Err(e) => Ok(err_to_jsval(e, ctx)),
        }
    }

    pub fn ws_delete(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let path = arg_string(args, 0, ctx)?;
        let result = with_state(|s, h| h.block_on(s.pcm.delete(&path)));
        match result {
            Ok(_) => Ok(JsValue::from(js_string!("ok"))),
            Err(e) => Ok(err_to_jsval(e, ctx)),
        }
    }

    pub fn ws_find(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let root = arg_string(args, 0, ctx).unwrap_or_else(|_| "/".into());
        let name = arg_string(args, 1, ctx)?;
        let kind = arg_string(args, 2, ctx).unwrap_or_else(|_| "all".into());
        let limit = arg_opt_i32(args, 3, ctx).unwrap_or(10).max(1);
        let result = with_state(|s, h| h.block_on(s.pcm.find(&root, &name, &kind, limit)));
        match result {
            Ok(text) => {
                let entries: Vec<Value> = text.lines().skip(1)
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| json!({ "name": l.trim() }))
                    .collect();
                json_to_jsvalue(&json!({ "entries": entries }), ctx)
            }
            Err(e) => Ok(err_to_jsval(e, ctx)),
        }
    }

    pub fn ws_tree(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let root = arg_string(args, 0, ctx).unwrap_or_else(|_| "/".into());
        let level = arg_opt_i32(args, 1, ctx).unwrap_or(0);
        let result = with_state(|s, h| h.block_on(s.pcm.tree(&root, level)));
        match result {
            Ok(text) => Ok(JsValue::from(JsString::from(text.as_str()))),
            Err(e) => Ok(err_to_jsval(e, ctx)),
        }
    }

    pub fn ws_move(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let from = arg_string(args, 0, ctx)?;
        let to = arg_string(args, 1, ctx)?;
        let result = with_state(|s, h| h.block_on(s.pcm.move_file(&from, &to)));
        match result {
            Ok(_) => Ok(JsValue::from(js_string!("ok"))),
            Err(e) => Ok(err_to_jsval(e, ctx)),
        }
    }

    pub fn ws_context(_this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let result = with_state(|s, h| h.block_on(s.pcm.context()));
        match result {
            Ok(text) => json_to_jsvalue(&parse_context_text(&text), ctx),
            Err(e) => Ok(err_to_jsval(e, ctx)),
        }
    }

    pub fn console_log(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        // Route through JSON.stringify for objects (handles undefined safely), to_string otherwise.
        let parts: Vec<String> = args.iter().enumerate().map(|(i, v)| {
            if v.is_object() {
                let name = format!("__log_arg_{i}");
                ctx.global_object().set(JsString::from(name.as_str()), v.clone(), true, ctx).ok();
                ctx.eval(Source::from_bytes(&format!("JSON.stringify(globalThis.{name} ?? null)")))
                    .ok()
                    .and_then(|r| r.to_string(ctx).ok())
                    .map(|s| s.to_std_string_escaped())
                    .unwrap_or_default()
            } else {
                v.to_string(ctx).map(|s| s.to_std_string_escaped()).unwrap_or_default()
            }
        }).collect();
        let line = parts.join(" ");
        with_state(|s, _| s.log_buffer.lock().unwrap().push(line));
        Ok(JsValue::undefined())
    }

    // ws_answer intentionally omitted: main agent uses its own AnswerTool
    // for final submission — eval is a compute/mutate step, not a terminal call.
}

// ─── Boa eval driver ────────────────────────────────────────────────────────

fn run_js_sync(code: &str, session: Arc<EvalSession>, handle: tokio::runtime::Handle) -> String {
    use boa_engine::{Context, NativeFunction, Source, js_string};

    CURRENT.with(|c| *c.borrow_mut() = Some(session.clone()));
    TOKIO_HANDLE.with(|h| *h.borrow_mut() = Some(handle));

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut ctx = Context::default();

        // Register host functions.
        let reg = |ctx: &mut Context, name: &str, len: usize, f: fn(&boa_engine::JsValue, &[boa_engine::JsValue], &mut Context) -> boa_engine::JsResult<boa_engine::JsValue>| {
            let _ = ctx.register_global_callable(
                boa_engine::JsString::from(name),
                len,
                NativeFunction::from_fn_ptr(f),
            );
        };
        reg(&mut ctx, "ws_read", 1, host::ws_read);
        reg(&mut ctx, "ws_list", 1, host::ws_list);
        reg(&mut ctx, "ws_search", 3, host::ws_search);
        reg(&mut ctx, "ws_find", 4, host::ws_find);
        reg(&mut ctx, "ws_tree", 2, host::ws_tree);
        reg(&mut ctx, "ws_write", 2, host::ws_write);
        reg(&mut ctx, "ws_delete", 1, host::ws_delete);
        reg(&mut ctx, "ws_move", 2, host::ws_move);
        reg(&mut ctx, "ws_context", 0, host::ws_context);
        reg(&mut ctx, "__console_log", 1, host::console_log);
        // Wire console.log/error/warn → __console_log (one host fn, all levels).
        let _ = ctx.eval(Source::from_bytes(
            "globalThis.console = { log: __console_log, error: __console_log, warn: __console_log };",
        ));

        // Inject scratchpad as global.
        let sp_json = serde_json::to_string(&session.scratchpad.lock().unwrap().clone())
            .unwrap_or_else(|_| "{}".into());
        let init = format!("globalThis.scratchpad = {sp_json};\n");
        let _ = ctx.eval(Source::from_bytes(&init));

        // Drain any stale log buffer from a previous call.
        session.log_buffer.lock().unwrap().clear();

        // Run user code; capture any JS error.
        let eval_result = ctx.eval(Source::from_bytes(code));

        // Extract updated scratchpad back to Rust.
        if let Ok(v) = ctx.eval(Source::from_bytes("JSON.stringify(globalThis.scratchpad ?? {})")) {
            if let Ok(s) = v.to_string(&mut ctx) {
                // llm_json repair fallback — agent sometimes produces not-quite-JSON in scratchpad.
                let parsed = sgr_agent::str_ext::parse_tool_args(&s.to_std_string_escaped());
                *session.scratchpad.lock().unwrap() = parsed;
            }
        }

        // Build output: console logs first, then last-expression value or error.
        let logs = std::mem::take(&mut *session.log_buffer.lock().unwrap());
        let mut output = logs.join("\n");
        match eval_result {
            Ok(val) if !val.is_undefined() && !val.is_null() => {
                if let Ok(s) = val.to_string(&mut ctx) {
                    let s = s.to_std_string_escaped();
                    if !output.is_empty() { output.push('\n'); }
                    output.push_str(&s);
                }
            }
            Err(e) => {
                if !output.is_empty() { output.push('\n'); }
                output.push_str(&format!("JS error: {e}"));
            }
            _ => {}
        }
        if output.is_empty() { output = "(no output)".into(); }
        output
    }));

    CURRENT.with(|c| *c.borrow_mut() = None);
    TOKIO_HANDLE.with(|h| *h.borrow_mut() = None);

    match result {
        Ok(s) => s,
        Err(_) => "JS panic".to_string(),
    }
}

// ─── EvalFullTool — registered alongside the 16 atomic tools ────────────────

pub struct EvalFullTool {
    pub session: Arc<EvalSession>,
}

#[derive(Deserialize, JsonSchema)]
struct ExecuteArgs {
    /// JavaScript (ES2022) code. Host fns (sync): ws_read(path), ws_write(path, content, start_line?, end_line?),
    /// ws_delete(path), ws_list(path), ws_search(root, pattern, limit?), ws_find(root, name, kind?, limit?),
    /// ws_tree(root?, level?), ws_move(from, to), ws_context(). `scratchpad` is a persistent global object.
    /// Last expression = output. Use for batch compute/mutate; final answer goes through AnswerTool, not here.
    code: String,
}

#[async_trait]
impl Tool for EvalFullTool {
    fn name(&self) -> &str { "eval" }
    fn description(&self) -> &str {
        "Run JavaScript with LIVE workspace access. Host fns: ws_read/write/delete/list/search/find/tree/move/context. \
         `scratchpad` persists across eval calls. Use for batch aggregation (sum totals across N files), \
         multi-file transforms (OCR 5 invoices in one block), filtering/sorting. \
         Writes/deletes still pass through workspace policy — same safety as atomic tools. \
         Final answer goes via AnswerTool, not here."
    }
    fn is_read_only(&self) -> bool { false }
    fn parameters_schema(&self) -> Value { json_schema_for::<ExecuteArgs>() }
    async fn execute(&self, args: Value, _ctx: &mut AgentContext) -> Result<ToolOutput, ToolError> {
        let a: ExecuteArgs = parse_args(&args)?;
        let session = self.session.clone();
        let handle = tokio::runtime::Handle::current();
        let code = a.code.clone();

        // Boa Context is !Send — run in a dedicated OS thread.
        let output = tokio::task::spawn_blocking(move || {
            std::thread::scope(|s| {
                s.spawn(|| run_js_sync(&code, session, handle)).join().unwrap_or_else(|_| "thread panic".into())
            })
        })
        .await
        .map_err(|e| ToolError::Execution(format!("join: {e}")))?;

        Ok(ToolOutput::text(output))
    }
    async fn execute_readonly(&self, args: Value, ctx: &AgentContext) -> Result<ToolOutput, ToolError> {
        // Readonly path — same impl (sandbox confines side effects via host-fn gating TODO).
        let mut ctx = ctx.clone();
        self.execute(args, &mut ctx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercise pure JS-eval path with no host calls — checks Boa wiring +
    /// scratchpad round-trip across two calls. PcmClient is not touched.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn js_scratchpad_roundtrip() {
        // Dummy PcmClient — never called because test JS doesn't touch ws_* except ws_answer.
        let pcm = Arc::new(PcmClient::new("http://127.0.0.1:1"));
        let session = EvalSession::new(pcm);

        let handle = tokio::runtime::Handle::current();
        let s1 = session.clone();
        let h1 = handle.clone();
        let out = tokio::task::spawn_blocking(move || {
            std::thread::scope(|sc| {
                sc.spawn(|| run_js_sync(
                    "scratchpad.step1 = 'done'; scratchpad.counter = 1;",
                    s1, h1,
                )).join().unwrap()
            })
        }).await.unwrap();
        let _ = out; // last expression is the assignment = 1; we only care about state below.
        let sp = session.scratchpad_snapshot();
        assert_eq!(sp["step1"], "done");
        assert_eq!(sp["counter"], 1);

        // Second call: scratchpad persists; add to counter.
        let s2 = session.clone();
        let h2 = handle;
        tokio::task::spawn_blocking(move || {
            std::thread::scope(|sc| {
                sc.spawn(|| run_js_sync(
                    "scratchpad.counter += 5;",
                    s2, h2,
                )).join().unwrap()
            })
        }).await.unwrap();

        let sp = session.scratchpad_snapshot();
        assert_eq!(sp["counter"], 6);
    }
}
