#!/usr/bin/env python3
"""Bounded synthetic oracle for the actual Python controller projections."""
import json
import hashlib
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import backend


def main():
    raw = sys.stdin.buffer.read(1024 * 1024 + 1)
    if len(raw) > 1024 * 1024:
        raise ValueError
    cases = json.loads(raw)
    if not isinstance(cases, list) or len(cases) > 128:
        raise ValueError
    results = []
    for case in cases:
        function = {"rules": backend.loaded_rules_payload,
                    "providers": backend.loaded_rule_providers_payload}[case["kind"]]
        try:
            result = function(case["payload"], tuple(case.get("private", [])))
            # Map ordering is not a controller semantic; Rust canonicalizes it.
            if case["kind"] == "providers":
                result["items"].sort(key=lambda row: row["name"])
            canonical = json.dumps(result, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
            results.append({"ok": True, "digest": hashlib.sha256(canonical).hexdigest()})
        except backend.BackendError:
            results.append({"ok": False})
    # Output contains only acceptance and digests, never controller strings.
    print(json.dumps(results, ensure_ascii=False))


if __name__ == "__main__":
    try:
        main()
    except Exception:
        print("Diagnostic projection oracle failed", file=sys.stderr)
        sys.exit(1)
