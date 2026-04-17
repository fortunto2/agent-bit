# Pangolin-style architecture experiment

**Branch:** `experiment/pangolin-arch`
**Inspired by:** [Operation Pangolin](https://github.com/...) — BitGN PAC1 winner (92/104 on Opus).

## Thesis

One agent, one tool (`execute_code`), all workspace access via a single JavaScript sandbox. Replaces our 16-tool function-calling loop + 15 skills + ML routing with:

- one persistent `scratchpad` (JSON) — gates + accumulated data + refs
- `verify(sp)` function defined by agent before submit — self-gated
- target call budget: **2-3 `execute_code` calls** per task (call 1 = all reads, call 2 = decide + writes + answer)

Pangolin used Python subprocess. We use **Boa (JS)** — in-process, no spawn overhead, deep integration with our existing `Arc<PcmClient>`.

## What's in the branch (PoC)

### New files (no existing files touched except `main.rs` module declaration)
- `src/pangolin.rs` — `PangolinSession` + `ExecuteCodeTool`. Host functions bound to `PcmClient`.
- `prompts/pangolin.md` — system prompt (ported from Pangolin Python → JS idioms; our 5 outcomes).
- `docs/pangolin-arch.md` — this file.

### Host functions (sync — block_on internally)
`ws_read`, `ws_write`, `ws_delete`, `ws_list`, `ws_search`, `ws_find`, `ws_tree`, `ws_move`, `ws_context`, `ws_answer`.

### Threading model
`Context` from Boa is `!Send`, so each `execute_code` call runs in a dedicated OS thread (`std::thread::scope` inside `tokio::task::spawn_blocking`). The thread owns a thread-local `Arc<PangolinSession>` + `tokio::runtime::Handle`; sync host fns `Handle::block_on(pcm.read(...))` to bridge to async.

### Persistence across calls
- `scratchpad` JSON lives in `Arc<Mutex<Value>>` on the session. Before eval, it's injected as a JS global. After eval, we re-extract via `JSON.stringify(globalThis.scratchpad)`.
- `ws_answer({answer, outcome, refs})` writes to `Arc<Mutex<Option<AnswerPayload>>>`. The outer agent loop checks this after each eval and submits to the harness.
- **Not yet persistent:** user-defined top-level JS variables (Pangolin Python does this via reflection on `globals()`). If we need it, mirror the approach in Boa via iterating `ctx.global_object()` and serializing JSON-serializable keys.

## What's NOT in this PoC (next steps)

1. **Agent loop wiring** — no `PangolinAgent` yet. Right now the tool is a standalone unit tested in isolation. Need:
   - `src/pangolin_agent.rs` — custom `Agent` impl with ONE tool = `ExecuteCodeTool`, conversation loop, context-tag injection (`<workspace-tree>`, `<scratchpad>`, `<task-instruction>`).
   - `--arch pangolin` flag in `main.rs` that routes past `pregrounding` (minimal pregrounding — only tree + context + instruction).
2. **`verify()` function capture** — right now `ws_answer` ignores the verify arg. Need to either:
   - evaluate `verify(scratchpad)` inside Boa before `ws_answer` captures, OR
   - pass `verify` as a `JsFunction` ref, call it via `.call()` in host fn.
3. **Tracking-vs-refs completeness warning** — Pangolin's Python wrapper warns if `scratchpad.refs` misses auto-tracked reads. We track in `refs_tracking` but don't yet diff.
4. **Context compaction** — Pangolin uses Anthropic `compact_20260112` beta with `thinking: adaptive`. Our `LlmClient` doesn't expose these; wire via a new anthropic-specific path for this arch.
5. **Error recovery call 3** — prompt says "call 3 = only if call 2 raised an error". Need to detect exception inside Boa and pass it back as a nudge.
6. **Selective test** — run on t014/t015/t035/t043 via a minimal driver that just loops `execute_code` until `session.take_answer()` returns Some.

## Why Boa over Python subprocess

| | Python subprocess | **Boa (JS) — chosen** |
|---|---|---|
| Cold start | 100-500 ms per call | ~1 ms |
| Access to PCM | HTTP round-trip via generated pypi client | direct `Arc<PcmClient>` (cached, tracked) |
| Sandbox | needs Docker or seccomp | built-in (no fs/net/require) |
| Deps | uv/pip infra + bitgn buf index | zero — already in `boa_engine 0.20` |
| Claude training | excellent | good (Opus/Haiku fluent in JS) |
| Persistent vars | via globals reflection | possible via `globalThis` snapshot |

## Tests

`cargo test pangolin::` — 1 test:
- `js_scratchpad_and_answer_capture` — mutates `scratchpad` across two `execute_code` calls, verifies `ws_answer` captures payload. No `PcmClient` mocking needed (test JS doesn't touch `ws_read`/`ws_write` etc.).

## Merge criteria

Do NOT merge until:
1. Selective test passes on 3+ real tasks (t014 baseline, t015 trap, t035 multilingual).
2. Full-leaderboard dry-run on Gemma 4 ≥ current `main` (78% Haiku / 63% Gemma).
3. Pangolin-only system prompt passes review — no regressions from our security signals (ML injection, sender mismatch, etc.).

Until then, this branch is research scaffold. `main` untouched.
