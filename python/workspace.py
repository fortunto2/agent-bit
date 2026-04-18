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


class _Workspace:
    def read(self, path):
        return _rpc("read", path=path)

    def write(self, path, content, start_line=0, end_line=0):
        return _rpc("write", path=path, content=content, start_line=start_line, end_line=end_line)

    def delete(self, path):
        return _rpc("delete", path=path)

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

    def answer(self, sp):
        """Submit final answer — sp is a dict with answer/outcome/refs."""
        with open(_ANSWER_PATH, "w") as f:
            json.dump({
                "message": sp.get("answer", ""),
                "outcome": sp.get("outcome", "OUTCOME_OK"),
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
