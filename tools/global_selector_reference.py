#!/usr/bin/env python3
"""Effect-isolated actual Python D1 selection; emit only digest/count."""
import contextlib
import hashlib
import io
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import backend


def main():
    raw = sys.stdin.buffer.read(1025)
    if not 1 <= len(raw) <= 1024:
        raise ValueError()
    name = raw.decode("utf-8")
    if any(ord(char) < 32 or ord(char) == 127 for char in name):
        raise ValueError()
    actions = []
    selected = {}

    def request(_socket, method, endpoint, _timeout, body):
        if method != "PUT" or endpoint not in ("/proxies/PROXY", "/proxies/GLOBAL"):
            raise ValueError()
        group = endpoint.rsplit("/", 1)[1]
        actions.append([group, body["name"]])
        selected[endpoint] = body["name"]
        return 204, {}

    backend.wait_private_controller = lambda _paths: Path("synthetic.sock")
    backend.controller_request = request
    backend.controller_json = lambda _socket, endpoint, _timeout: (200, {"now": selected[endpoint]})
    with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
        backend.select_global_proxy(None, name)
    digest = hashlib.sha256(json.dumps(actions, ensure_ascii=False, separators=(",", ":")).encode()).hexdigest()
    print(json.dumps({"count": len(actions), "digest": digest}))


if __name__ == "__main__":
    try:
        main()
    except Exception:
        print('{"error":"reference_failed"}')
        sys.exit(1)
