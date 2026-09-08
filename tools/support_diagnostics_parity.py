#!/usr/bin/env python3
"""Actual Python support snapshot, with all host effects replaced; digest only."""
import contextlib
import hashlib
import json
import sys
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import backend


class AbsentFile:
    def exists(self):
        return False

    def is_file(self):
        return False

    def is_symlink(self):
        return False


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
        paths = SimpleNamespace(**{name: AbsentFile() for name in ("store", "template", "config", "unit")})
        routing = {"mode": store["startup"]["mode"], "source": "custom",
                   "preset": store["routingPreset"], "configured": bool(store["routingPreset"]),
                   "ruleCount": 0, "providerCount": 0, "rulesUpdatedAt": store["rulesUpdatedAt"]}
        with contextlib.ExitStack() as stack:
            for name, result in {
                "load_store": store, "service_active": False, "service_enabled": False,
                "routing_status": routing, "routing_conflicts": [],
                "core_setup_status": {"installed": False, "tunReady": False},
                "controller_socket": AbsentFile(),
            }.items():
                stack.enter_context(patch.object(backend, name, return_value=result))
            # Any unexpected host helper is a test failure, not a real command.
            stack.enter_context(patch.object(backend, "run", side_effect=AssertionError))
            payload = backend.diagnostics_payload(paths)
        projected = {key: payload[key] for key in ("inventory", "startup", "updates")}
        projected["routing"] = {key: payload["routing"][key] for key in
                                ("preset", "configured", "lastManualRuleUpdate")}
        projected["onboardingComplete"] = store["onboardingComplete"]
        canonical = json.dumps(projected, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
        results.append(hashlib.sha256(canonical).hexdigest())
    print(json.dumps(results))


if __name__ == "__main__":
    try:
        main()
    except Exception:
        print("Support diagnostics oracle failed", file=sys.stderr)
        sys.exit(1)
