#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Test-only archived-oracle replies, never an application/backend fallback.

Exact request bytes bind every reply. A new/changed vector MUST fail until its
independent expected behavior is reviewed; never derive it from the Rust result.
The separate opt-in recorder executes only the pinned archived Python oracle.
Ordinary replay is offline and cannot execute a backend or access private state.
"""
import hashlib
import json
from pathlib import Path
import sys

REFERENCE = "aa5873783c019edc303a732e55ea8c85f1f0b090"
ROOT = Path(__file__).resolve().parents[1]
LIMIT = 16 * 1024 * 1024


def request_key(arguments, payload):
    encoded = json.dumps(arguments, ensure_ascii=True, separators=(",", ":")).encode()
    if len(payload) > LIMIT or len(encoded) > 128 * 1024:
        raise ValueError("reference_input_bound")
    return hashlib.sha256(encoded + b"\0" + payload).hexdigest()


def fixture_path(tool, key, root=None):
    if not tool.endswith(".py") or any(c not in "abcdefghijklmnopqrstuvwxyz_0123456789." for c in tool):
        raise ValueError("reference_tool_invalid")
    if len(key) != 64 or any(c not in "0123456789abcdef" for c in key):
        raise ValueError("reference_key_invalid")
    return (ROOT / "tests/frozen_reference" if root is None else root) / tool.removesuffix(".py") / (key + ".json")


def replay(tool, arguments, payload, root=None):
    key = request_key(arguments, payload)
    path = fixture_path(tool, key, root)
    if path.is_symlink() or not path.is_file() or path.stat().st_size > LIMIT:
        raise ValueError("reference_fixture_missing")
    def unique(pairs):
        obj = {}
        for name, value in pairs:
            if name in obj:
                raise ValueError("reference_fixture_invalid")
            obj[name] = value
        return obj
    value = json.loads(path.read_bytes(), object_pairs_hook=unique)
    if (not isinstance(value, dict)
            or set(value) != {"schemaVersion", "referenceCommit", "tool", "requestSha256", "inputBytes",
                       "argumentCount", "returncode", "stdout", "stderr"}
            or type(value["schemaVersion"]) is not int or value["schemaVersion"] != 1
            or value["referenceCommit"] != REFERENCE
            or value["tool"] != tool or value["requestSha256"] != key
            or type(value["inputBytes"]) is not int or value["inputBytes"] != len(payload)
            or type(value["argumentCount"]) is not int or value["argumentCount"] != len(arguments)
            or type(value["returncode"]) is not int or value["returncode"] not in (0, 1, 2)
            or not isinstance(value["stdout"], str) or not isinstance(value["stderr"], str)):
        raise ValueError("reference_fixture_invalid")
    return value


def main(script, arguments):
    try:
        tool = Path(script).name
        payload = sys.stdin.buffer.read(LIMIT + 1)
        # Explicit developer-only regeneration. Default has no subprocess path.
        import os
        if os.environ.get("OMAVLESS_RECORD_REFERENCE") == "1":
            from record_frozen_reference import record
            value = record(tool, arguments, payload)
        else:
            value = replay(tool, arguments, payload)
        sys.stdout.write(value["stdout"])
        sys.stderr.write(value["stderr"])
        return value["returncode"]
    except Exception:
        print("Frozen reference unavailable or invalid; review the independent fixture.", file=sys.stderr)
        return 2
