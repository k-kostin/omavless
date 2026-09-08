#!/usr/bin/env python3
"""Bounded synthetic oracle; prints classifications and digests, never names."""
import contextlib
import hashlib
import io
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import backend


def main():
    raw = sys.stdin.buffer.read(1024 * 1024 + 1)
    if len(raw) > 1024 * 1024:
        return 2
    cases = json.loads(raw)
    if not isinstance(cases, list) or len(cases) > 128:
        return 2
    results = []
    for case in cases:
        try:
            paths = []
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                names = backend.refreshable_rule_provider_names(case)
                def request(_socket, method, endpoint, timeout):
                    if method != "PUT" or timeout != 60:
                        raise ValueError("invalid fixed request")
                    paths.append(endpoint)
                    return 204, b""
                original = backend.controller_request
                try:
                    backend.controller_request = request
                    for name in names:
                        backend._refresh_rule_provider(Path("/synthetic/mihomo.sock"), name)
                finally:
                    backend.controller_request = original
            digest = hashlib.sha256(json.dumps(sorted(paths), ensure_ascii=False, separators=(",", ":")).encode()).hexdigest()
            results.append({"ok": True, "digest": digest})
        except Exception:
            results.append({"ok": False})
    print(json.dumps(results, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception:
        sys.exit(2)
