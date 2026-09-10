#!/usr/bin/env python3
"""Actual Python details oracle; synthetic input over stdin, digest-only output."""
import contextlib
import hashlib
import io
import json
from pathlib import Path
import sys
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
    quic_fallbacks = 0
    for case in cases:
        try:
            capture = io.StringIO()
            with patch.object(backend, "load_store", return_value=backend.validate_store(case["store"])), \
                 patch.object(backend, "interface_addresses", return_value=[]), contextlib.redirect_stdout(capture):
                try:
                    backend.details(None, case["id"])
                    value = json.loads(capture.getvalue())
                    value.pop("address")
                except KeyError:
                    profile = backend.profile_by_id(backend.load_store(None), case["id"])
                    preview = backend.preview_profile(profile["uri"])
                    if preview["protocol"] not in ("hysteria2", "tuic"):
                        raise
                    quic_fallbacks += 1
                    value = {"version": 1, "server": f"{preview['server']}:{preview['port']}",
                             "transport": f"{preview['transport']} / {preview['security']}",
                             "sni": preview["sni"] or "--"}
            encoded = json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(",", ":")).encode()
            results.append(hashlib.sha256(encoded).hexdigest())
        except backend.BackendError:
            results.append(None)
    print(json.dumps({"results": results, "quicFallbacks": quic_fallbacks}))

if __name__ == "__main__":
    try:
        main()
    except Exception:
        print("Profile details oracle failed", file=sys.stderr)
        sys.exit(1)
