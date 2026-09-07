#!/usr/bin/env python3
"""Bounded synthetic oracle for actual Python custom-rule editor projection."""
import hashlib
import json
import sys
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import backend


def main():
    raw = sys.stdin.buffer.read(8 * 1024 * 1024 + 1)
    if len(raw) > 8 * 1024 * 1024:
        raise ValueError
    cases = json.loads(raw)
    if not isinstance(cases, list) or len(cases) > 256:
        raise ValueError
    results = []
    for case in cases:
        store = backend.validate_store(case)
        with patch.object(backend, "load_store", return_value=store):
            payload = json.loads(backend.custom_rules_text(None))
        canonical = json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
        results.append(hashlib.sha256(canonical).hexdigest())
    print(json.dumps(results))


if __name__ == "__main__":
    try:
        main()
    except Exception:
        print("Custom rule read oracle failed", file=sys.stderr)
        sys.exit(1)
