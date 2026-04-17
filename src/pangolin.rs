//! Pangolin-style single-tool agent (experiment).
//!
//! One tool: `execute_code` — runs JavaScript in Boa sandbox with host functions
//! bound to our PcmClient. Persistent `scratchpad` across eval calls. Inspired by
//! Operation Pangolin (BitGN PAC1 winner, 92/104 on Opus + Python execute_code).
//!
//! # Architecture
//! - Agent has exactly ONE tool: `execute_code(code: string)`.
//! - Each call: JS runs in a dedicated OS thread (Boa Context is !Send, !Sync).
//! - Host functions (`ws_read/write/search/list/find/tree/delete/move/context/answer`)
//!   use a thread-local `PangolinShared` that holds `Arc<PcmClient>` + tokio `Handle`
//!   so we can `block_on(async fn)` from inside sync JS host functions.
//! - `scratchpad` JSON object is injected as a Boa global before eval, serialized
//!   back to Rust after eval. `ws_answer({...})` captures the answer payload and
//!   the Rust side submits via PcmClient post-eval.
//!
//! # Status: scaffold / PoC
//! Minimum host API (read/list/search/write/delete/answer). TODO: tree, find,
//! move, mkdir, context, compaction. Agent loop is not wired up — see `prompts/pangolin.md`
//! and `main.rs --arch pangolin` for the flow.

use std::cell::RefCell;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sgr_agent::agent_tool::{Tool, ToolError, ToolOutput, parse_args};
use sgr_agent::context::AgentContext;
use sgr_agent::schema::json_schema_for;

use crate::pcm::PcmClient;

// ─── Shared state between tool calls ────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnswerPayload {
    pub message: String,
    pub outcome: String,
    pub refs: Vec<String>,
}

/// State that persists across multiple `execute_code` invocations within one task.
pub struct PangolinSession {
    pub pcm: Arc<PcmClient>,
    pub scratchpad: std::sync::Mutex<Value>,
    pub answer: std::sync::Mutex<Option<AnswerPayload>>,
    pub refs_tracking: std::sync::Mutex<Vec<String>>, // auto-tracked reads/writes
    pub log_buffer: std::sync::Mutex<Vec<String>>,    // console.log output, drained each call
}

impl PangolinSession {
    pub fn new(pcm: Arc<PcmClient>) -> Arc<Self> {
        Arc::new(Self {
            pcm,
            scratchpad: std::sync::Mutex::new(json!({ "refs": [] })),
            answer: std::sync::Mutex::new(None),
            refs_tracking: std::sync::Mutex::new(Vec::new()),
            log_buffer: std::sync::Mutex::new(Vec::new()),
        })
    }

    pub fn with_context(pcm: Arc<PcmClient>, context_json: Value) -> Arc<Self> {
        Arc::new(Self {
            pcm,
            scratchpad: std::sync::Mutex::new(json!({ "refs": [], "context": context_json })),
            answer: std::sync::Mutex::new(None),
            refs_tracking: std::sync::Mutex::new(Vec::new()),
            log_buffer: std::sync::Mutex::new(Vec::new()),
        })
    }

    pub fn take_answer(&self) -> Option<AnswerPayload> {
        self.answer.lock().unwrap().take()
    }

    pub fn scratchpad_snapshot(&self) -> Value {
        self.scratchpad.lock().unwrap().clone()
    }
}

// ─── Thread-local handoff so NativeFunctionPointer fns can reach state ──────

thread_local! {
    static CURRENT: RefCell<Option<Arc<PangolinSession>>> = const { RefCell::new(None) };
    static TOKIO_HANDLE: RefCell<Option<tokio::runtime::Handle>> = const { RefCell::new(None) };
}

fn with_state<F, R>(f: F) -> R
where
    F: FnOnce(&Arc<PangolinSession>, &tokio::runtime::Handle) -> R,
{
    CURRENT.with(|c| {
        TOKIO_HANDLE.with(|h| {
            let c_ref = c.borrow();
            let h_ref = h.borrow();
            let state = c_ref.as_ref().expect("PangolinSession not installed on this thread");
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
        let s = serde_json::to_string(v).unwrap_or_else(|_| "null".into());
        JsValue::from_json(&serde_json::from_str(&s).unwrap_or(Value::Null), ctx)
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
        // start_line/end_line: 0,0 = overwrite. 1,1 = insert before line 1 (prepend).
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
            Ok(text) => {
                let parsed: Value = serde_json::from_str(&text).unwrap_or(json!({ "raw": text }));
                json_to_jsvalue(&parsed, ctx)
            }
            Err(e) => Ok(err_to_jsval(e, ctx)),
        }
    }

    pub fn console_log(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let parts: Vec<String> = args.iter().map(|v| {
            // Non-object primitives: skip to_json (undefined/unpairable -> panic in Boa).
            if v.is_object() {
                match v.to_json(ctx) {
                    Ok(json) => serde_json::to_string(&json).unwrap_or_default(),
                    Err(_) => v.to_string(ctx).map(|s| s.to_std_string_escaped()).unwrap_or_default(),
                }
            } else {
                v.to_string(ctx).map(|s| s.to_std_string_escaped()).unwrap_or_default()
            }
        }).collect();
        let line = parts.join(" ");
        with_state(|s, _| s.log_buffer.lock().unwrap().push(line));
        Ok(JsValue::undefined())
    }

    pub fn ws_answer(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        // Accepts: ws_answer({message, outcome, refs}) or ws_answer(scratchpad_with_those_keys).
        let obj = args.first().cloned().unwrap_or(JsValue::undefined());
        // Guard against Boa's "undefined to JSON" panic — only object-like values are serializable.
        let json_str = if obj.is_object() {
            obj.to_json(ctx).ok()
                .and_then(|v| serde_json::to_string(&v).ok())
                .unwrap_or_else(|| "{}".into())
        } else {
            "{}".into()
        };
        let parsed: Value = serde_json::from_str(&json_str).unwrap_or(Value::Null);
        let message = parsed.get("answer").and_then(|v| v.as_str())
            .or_else(|| parsed.get("message").and_then(|v| v.as_str()))
            .unwrap_or("").to_string();
        let outcome = parsed.get("outcome").and_then(|v| v.as_str()).unwrap_or("OUTCOME_OK").to_string();
        let refs: Vec<String> = parsed.get("refs").and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
            .unwrap_or_default();
        with_state(|s, _| {
            *s.answer.lock().unwrap() = Some(AnswerPayload { message, outcome, refs });
        });
        Ok(JsValue::from(js_string!("submitted")))
    }
}

// ─── Boa eval driver ────────────────────────────────────────────────────────

fn run_js_sync(code: &str, session: Arc<PangolinSession>, handle: tokio::runtime::Handle) -> String {
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
        reg(&mut ctx, "ws_answer", 1, host::ws_answer);
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
                if let Ok(parsed) = serde_json::from_str::<Value>(&s.to_std_string_escaped()) {
                    *session.scratchpad.lock().unwrap() = parsed;
                }
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

// ─── ExecuteCodeTool — the ONE tool the agent sees ──────────────────────────

pub struct ExecuteCodeTool {
    pub session: Arc<PangolinSession>,
}

#[derive(Deserialize, JsonSchema)]
struct ExecuteArgs {
    /// JavaScript code. Host fns (sync): ws_read/write/list/search/find/tree/delete/move/context/answer.
    /// `scratchpad` is a persistent global object. Call `ws_answer({answer,outcome,refs})` as the terminal step.
    code: String,
}

#[async_trait]
impl Tool for ExecuteCodeTool {
    fn name(&self) -> &str { "execute_code" }
    fn description(&self) -> &str {
        "Execute JavaScript in a sandbox. Host functions: ws_read(path), ws_write(path, content), \
         ws_delete(path), ws_list(path), ws_search(root, pattern, limit), ws_find(root, name, kind, limit), \
         ws_tree(root, level), ws_move(from, to), ws_context(), ws_answer({answer,outcome,refs}). \
         `scratchpad` is a global JSON object persisted across calls. Use ws_answer() as the terminal call."
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

    /// Exercise pure JS-eval path with no host calls — checks Boa wiring,
    /// scratchpad round-trip, and ws_answer capture. PcmClient is not touched.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn js_scratchpad_and_answer_capture() {
        // Dummy PcmClient — never called because test JS doesn't touch ws_* except ws_answer.
        let pcm = Arc::new(PcmClient::new("http://127.0.0.1:1"));
        let session = PangolinSession::new(pcm);

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

        // Second call: scratchpad should persist, mutate further, then submit.
        let s2 = session.clone();
        let h2 = handle;
        tokio::task::spawn_blocking(move || {
            std::thread::scope(|sc| {
                sc.spawn(|| run_js_sync(
                    "scratchpad.counter += 5; ws_answer({answer: 'hello', outcome: 'OUTCOME_OK', refs: ['/a.md']});",
                    s2, h2,
                )).join().unwrap()
            })
        }).await.unwrap();

        let sp = session.scratchpad_snapshot();
        assert_eq!(sp["counter"], 6);
        let ans = session.take_answer().expect("ws_answer captured");
        assert_eq!(ans.message, "hello");
        assert_eq!(ans.outcome, "OUTCOME_OK");
        assert_eq!(ans.refs, vec!["/a.md".to_string()]);
    }
}
