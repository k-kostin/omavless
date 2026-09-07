#!/usr/bin/env python3
"""Actual Python login preference transaction with all host effects replaced."""
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
    output = []
    for case in cases:
        stored = backend.validate_store(case["store"])
        saved = []
        def save(_paths, value):
            saved.append(backend.validate_store(value))
        with patch.object(backend, "load_store", return_value=stored), \
             patch.object(backend, "save_store", side_effect=save), \
             patch.object(backend, "service_enabled", return_value=False), \
             patch.object(backend, "core_setup_status", return_value={"installed":True,"tunReady":True}), \
             patch.object(backend, "find_core", return_value=Path("/synthetic/core")), \
             patch.object(backend, "ensure_unit"), patch.object(backend, "ensure_startup_unit"), \
             patch.object(backend, "render_config_mode", return_value="synthetic"), \
             patch.object(backend, "test_config", return_value=SimpleNamespace(unlink=lambda: None)), \
             patch.object(backend, "systemctl", return_value=SimpleNamespace(returncode=0,stderr="",stdout="")):
            backend.configure_startup(None,case["enabled"],case["target"],case["profileId"],case["mode"])
        if len(saved) != 1:
            raise ValueError
        output.append(hashlib.sha256(json.dumps(saved[0],ensure_ascii=False,sort_keys=True,separators=(",",":")).encode()).hexdigest())
    print(json.dumps(output))


if __name__ == "__main__":
    try:
        main()
    except Exception:
        print("Startup preference oracle failed",file=sys.stderr)
        sys.exit(1)
