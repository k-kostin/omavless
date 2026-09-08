#!/usr/bin/env python3
"""Effect-isolated actual Python route attribution oracle; synthetic stdin only."""
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
from unittest import mock

spec = importlib.util.spec_from_file_location("route_reference", Path(__file__).resolve().parents[1] / "backend.py")
backend = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = backend
spec.loader.exec_module(backend)
cases = json.loads(sys.stdin.buffer.read(512 * 1024))
results = []
with mock.patch.object(backend, "run", side_effect=AssertionError("host effects forbidden")), \
     mock.patch.object(backend, "load_store", side_effect=AssertionError("private store forbidden")), \
     mock.patch.object(backend, "controller_json", side_effect=AssertionError("controller forbidden")), \
     mock.patch.object(backend.socket, "create_connection", side_effect=AssertionError("network forbidden")):
    for case in cases:
        try:
            result = backend.exact_route_connection(case["payload"], case["query"], 40000, 7890)
            canonical = json.dumps(result, sort_keys=True, ensure_ascii=False, separators=(",", ":")).encode()
            results.append({"digest": hashlib.sha256(canonical).hexdigest()})
        except backend.BackendError:
            results.append({"error": True})
print(json.dumps(results, separators=(",", ":")))
