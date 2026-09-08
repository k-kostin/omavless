#!/usr/bin/env python3
"""Bounded actual use_bundled_template oracle; no real store/host writes."""
import copy
import hashlib
import json
import sys
import tempfile
from pathlib import Path
from types import SimpleNamespace
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
        store = backend.validate_store(copy.deepcopy(case["store"]))
        result = {"store": copy.deepcopy(store), "template": case["template"]}
        def save(p, s):
            result["store"] = copy.deepcopy(backend.validate_store(s))
        def write(p, s):
            result["template"] = s
        original_ensure = backend.ensure_template
        with tempfile.TemporaryDirectory(prefix="omavless-preset-oracle-") as private, \
             patch.object(backend, "load_store", return_value=store), \
             patch.object(backend, "ensure_template", side_effect=lambda p: original_ensure(p) if case.get("missingTemplate") else case["template"]), \
             patch.object(backend, "save_store", side_effect=save), \
             patch.object(backend, "atomic_write", side_effect=write), \
             patch.object(backend, "service_active", return_value=False), \
             patch.object(backend, "connect_profile", side_effect=AssertionError("Unexpected host action")):
            backend.use_bundled_template(SimpleNamespace(template=Path(private) / "route-template.yaml"), case["preset"], case["keepMode"])
        canonical = json.dumps(result, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
        results.append(hashlib.sha256(canonical).hexdigest())
    print(json.dumps(results))


if __name__ == "__main__":
    try:
        main()
    except Exception:
        print("Routing preset oracle failed", file=sys.stderr)
        sys.exit(1)
