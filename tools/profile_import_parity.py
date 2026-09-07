#!/usr/bin/env python3
"""Bounded synthetic oracle; executes Python import with all effects replaced."""
import hashlib
import json
import sys
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
    paths = SimpleNamespace(config=SimpleNamespace(exists=lambda: False))
    for case in cases:
        captured = {}
        def save(_paths, store):
            captured["store"] = backend.validate_store(store)
        try:
            with patch.object(backend, "load_store", return_value=backend.validate_store(case["store"])), \
                 patch.object(backend, "save_store", side_effect=save), \
                 patch.object(backend, "service_active", return_value=False), \
                 patch.object(backend.uuidlib, "uuid4", return_value=case["id"]):
                backend.import_profile(paths, case["name"], "", case["input"])
            canonical = json.dumps(captured["store"], ensure_ascii=False, sort_keys=True,
                                   separators=(",", ":")).encode()
            results.append(hashlib.sha256(canonical).hexdigest())
        except backend.BackendError:
            results.append(None)
    print(json.dumps(results))


if __name__ == "__main__":
    try:
        main()
    except Exception:
        print("Profile import oracle failed", file=sys.stderr)
        sys.exit(1)
