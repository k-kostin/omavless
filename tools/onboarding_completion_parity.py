#!/usr/bin/env python3
"""Run actual legacy onboarding command with all filesystem/host effects replaced.

Synthetic input arrives on bounded stdin; output contains only fixed acceptance
and normalized-store digests. This probe never reads the installed store.
"""
import contextlib
import copy
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
    if not isinstance(cases, list) or len(cases) > 128:
        raise ValueError
    results = []
    for source in cases:
        saved = []
        def load(_paths):
            return backend.validate_store(copy.deepcopy(source))
        def save(_paths, value):
            saved.append(backend.validate_store(copy.deepcopy(value)))
        with contextlib.ExitStack() as stack:
            stack.enter_context(patch.object(backend, "run", side_effect=AssertionError("Unexpected host work")))
            stack.enter_context(patch.object(backend, "build_parser", return_value=SimpleNamespace(
                parse_args=lambda: SimpleNamespace(command="onboarding-complete"))))
            stack.enter_context(patch.object(backend.Paths, "current", return_value=SimpleNamespace(config_dir=Path("/synthetic"))))
            stack.enter_context(patch.object(backend, "ensure_private_dir"))
            stack.enter_context(patch.object(backend, "operation_lock", side_effect=lambda _: contextlib.nullcontext()))
            stack.enter_context(patch.object(backend, "legacy_mutation_lock", side_effect=lambda _: contextlib.nullcontext()))
            stack.enter_context(patch.object(backend, "read_ownership_marker", return_value=SimpleNamespace(phase="legacy")))
            stack.enter_context(patch.object(backend, "migrate_legacy_data"))
            stack.enter_context(patch.object(backend, "load_store", side_effect=load))
            stack.enter_context(patch.object(backend, "save_store", side_effect=save))
            try:
                if backend.main() != 0 or len(saved) != 1:
                    raise ValueError
            except backend.BackendError:
                results.append({"accepted": False})
                continue
        digest = hashlib.sha256(json.dumps(saved[0], ensure_ascii=False,
            sort_keys=True, separators=(",", ":")).encode()).hexdigest()
        results.append({"accepted": True, "digest": digest})
    print(json.dumps(results))


if __name__ == "__main__":
    try:
        main()
    except Exception:
        print("Onboarding oracle failed", file=sys.stderr)
        sys.exit(1)
