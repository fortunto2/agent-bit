"""Pangolin-py workspace client: Python 3.13, spoken from subprocess to Rust host.

The Rust side writes commands to our stdin as JSON lines; we route ws.* calls
through an RPC bridge (stdout: JSON requests, stdin: JSON responses). Keeps the
Rust PcmClient authoritative — all reads/writes / deletes land in the same
Connect-RPC channel as the main agent. No separate bitgn protobuf client needed.

Environment:
  AGENT_SCRATCHPAD_PATH   — JSON file persisted between calls (Rust writes seed)
  AGENT_ANSWER_PATH       — JSON file to write on ws.answer()
  AGENT_STATE_PATH        — JSON file for variable persistence between calls
"""

import json
import os
import sys
import atexit
from datetime import datetime, timedelta, date
from collections import defaultdict, Counter
from pathlib import PurePosixPath


_SCRATCHPAD_PATH = os.environ.get("AGENT_SCRATCHPAD_PATH", "/tmp/agent-scratchpad.json")
_ANSWER_PATH = os.environ.get("AGENT_ANSWER_PATH", "/tmp/agent-answer.json")
_STATE_PATH = os.environ.get("AGENT_STATE_PATH", "/tmp/agent-state.json")


def _load_json(path, default):
    try:
        with open(path) as f:
            return json.load(f)
    except (FileNotFoundError, json.JSONDecodeError):
        return default


def _rpc(method, **kwargs):
    """Send a JSON-RPC line to stdout (Rust host reads it), then read reply from stdin."""
    req = {"method": method, **kwargs}
    sys.stdout.write(f"__RPC__ {json.dumps(req)}\n")
    sys.stdout.flush()
    line = sys.stdin.readline()
    if not line:
        return {"error": "rpc closed"}
    try:
        return json.loads(line)
    except json.JSONDecodeError:
        return {"error": f"bad rpc reply: {line!r}"}


_OUTCOMES = {
    "OUTCOME_OK",
    "OUTCOME_DENIED_SECURITY",
    "OUTCOME_NONE_CLARIFICATION",
    "OUTCOME_NONE_UNSUPPORTED",
    "OUTCOME_ERR_INTERNAL",
}


class _Workspace:
    def __init__(self):
        # Track paths touched for pre-submit refs-completeness check.
        self._reads = []
        self._writes = []
        self._deletes = []

    def read(self, path):
        r = _rpc("read", path=path)
        if "error" not in r and path not in self._reads:
            self._reads.append(path)
        return r

    def write(self, path, content, start_line=0, end_line=0):
        r = _rpc("write", path=path, content=content, start_line=start_line, end_line=end_line)
        if r == "ok" and path not in self._writes:
            self._writes.append(path)
        return r

    def overwrite(self, path, content):
        """Explicit full rewrite — bypass the read-before-write guard. Use when
        the task genuinely calls for replacing the whole file (rare)."""
        r = _rpc("write", path=path, content=content, start_line=0, end_line=0)
        if r == "ok" and path not in self._writes:
            self._writes.append(path)
        return r

    def prepend(self, path, content):
        """Insert `content` before line 1 — preserves original body byte-for-byte.
        Use for frontmatter/header adds on existing files."""
        return self.write(path, content, start_line=1, end_line=1)

    def delete(self, path):
        r = _rpc("delete", path=path)
        if r == "ok" and path not in self._deletes:
            self._deletes.append(path)
        return r

    def list(self, path="/"):
        return _rpc("list", path=path)

    def search(self, root="/", pattern="", limit=10):
        return _rpc("search", root=root, pattern=pattern, limit=limit)

    def find(self, root="/", name="", kind="all", limit=10):
        return _rpc("find", root=root, name=name, kind=kind, limit=limit)

    def tree(self, root="/", level=0):
        return _rpc("tree", root=root, level=level)

    def move(self, from_name, to_name):
        return _rpc("move", **{"from": from_name, "to": to_name})

    def context(self):
        return _rpc("context")

    def answer(self, sp, verify=None):
        """Submit final answer. `sp` is a dict with answer/outcome/refs.
        `verify(sp) -> bool` — optional; if provided, False/exception blocks submission."""
        outcome = sp.get("outcome", "OUTCOME_OK")

        # Optional user-provided verify (non-breaking — only blocks if explicitly False).
        if verify is not None and callable(verify):
            try:
                ok = verify(sp)
            except Exception as e:
                msg = f"VERIFY ERROR: {e}. Fix verify() and retry ws.answer()."
                print(msg); raise ValueError(msg)
            if not ok:
                msg = "SUBMISSION BLOCKED: verify(sp) returned False. Fix scratchpad and retry."
                print(msg); raise ValueError(msg)

        # Persist answer for Rust host to pick up.
        with open(_ANSWER_PATH, "w") as f:
            json.dump({
                "message": sp.get("answer", ""),
                "outcome": outcome,
                "refs": sp.get("refs", []),
            }, f)
        return "submitted"


ws = _Workspace()

# Load scratchpad from persisted file — Rust seeds it between calls.
scratchpad = _load_json(_SCRATCHPAD_PATH, {"refs": []})

# Restore user-defined state variables (not prelude names).
_PRELUDE_NAMES = set(dir())
globals().update(_load_json(_STATE_PATH, {}))


def _save_scratchpad():
    with open(_SCRATCHPAD_PATH, "w") as f:
        json.dump(scratchpad, f)


def _save_state():
    state = {}
    for k, v in list(globals().items()):
        if k.startswith("_") or k in _PRELUDE_NAMES or k == "scratchpad":
            continue
        try:
            json.dumps(v)
            state[k] = v
        except (TypeError, ValueError):
            pass
    with open(_STATE_PATH, "w") as f:
        json.dump(state, f)


atexit.register(_save_scratchpad)
atexit.register(_save_state)
