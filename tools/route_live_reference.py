#!/usr/bin/env python3
"""Installed-core gate: actual Python live probe, synthetic loopback only."""
import importlib.util
import json
from pathlib import Path
import sys
from unittest import mock

spec = importlib.util.spec_from_file_location("route_live_reference", Path(__file__).resolve().parents[1] / "backend.py")
backend = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = backend
spec.loader.exec_module(backend)
root = Path(sys.argv[1])
paths = backend.Paths(root, root, root / "unused-store", root / "unused-template",
                      root / "config.yaml", root / "unused-unit", root, root, root)
try:
    with mock.patch.object(backend, "controller_socket", return_value=root / "mihomo.sock"), \
         mock.patch.object(backend, "load_store", side_effect=AssertionError("store forbidden")), \
         mock.patch.object(backend, "run", side_effect=AssertionError("host command forbidden")):
        result = backend.live_route_match(paths, "127.0.0.2", True)
    assert result["outcome"] == "block" and result["target"] == "REJECT" and result["source"] == "live"
    print(json.dumps({"actualPythonLiveProbe": "PASS"}))
except Exception:
    print(json.dumps({"actualPythonLiveProbe": "FAIL"}))
    raise SystemExit(1)
