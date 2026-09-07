#!/usr/bin/env python3
"""Actual Python add/delete oracle; synthetic bounded stdin, digests only."""
import copy
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
        saved = []
        try:
            store = backend.validate_store(copy.deepcopy(case["store"]))
            with patch.object(backend, "load_store", return_value=store), \
                 patch.object(backend, "save_store", side_effect=lambda p, s: saved.append(copy.deepcopy(backend.validate_store(s)))), \
                 patch.object(backend, "service_active", return_value=False), \
                 patch.object(backend, "connect_profile", side_effect=AssertionError("Unexpected host operation")), \
                 patch.object(backend.uuidlib, "uuid4", return_value=case["generatedId"]):
                if case["method"] == "add":
                    backend.save_custom_rule(None, case["kind"], case["action"], case["value"])
                else:
                    backend.delete_custom_rule(None, case["ruleId"])
            if len(saved) != 1:
                raise AssertionError
            canonical = json.dumps(saved[0], ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
            results.append(hashlib.sha256(canonical).hexdigest())
        except backend.BackendError:
            results.append("rejected")
    print(json.dumps(results))


if __name__ == "__main__":
    try:
        main()
    except Exception:
        print("Custom-rule mutation oracle failed", file=sys.stderr)
        sys.exit(1)
