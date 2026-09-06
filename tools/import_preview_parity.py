#!/usr/bin/env python3
"""Synthetic-test oracle: emit only a digest, never private import previews."""
import hashlib
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import backend


def main():
    raw = sys.stdin.buffer.read(2 * 1024 * 1024 + 1)
    if len(raw) > 2 * 1024 * 1024:
        raise ValueError("bounded input required")
    payload = json.loads(raw)
    if not isinstance(payload, list) or len(payload) > 1024:
        raise ValueError("bounded corpus required")
    results = []
    for item in payload:
        if (not isinstance(item, dict)
                or not isinstance(item.get("input"), str)
                or len(item["input"].encode()) > 65536
                or not isinstance(item.get("urls"), list)
                or len(item["urls"]) > 1024
                or any(not isinstance(url, str) or len(url.encode()) > 8192
                       for url in item["urls"])):
            raise ValueError("bounded case required")
        try:
            result = backend.classify_import(item["input"], {
                "subscriptions": [{"url": url} for url in item["urls"]],
            })
            encoded = json.dumps(result, sort_keys=True, ensure_ascii=False,
                                 separators=(",", ":")).encode()
            results.append(hashlib.sha256(encoded).hexdigest())
        except backend.BackendError:
            results.append(None)
    print(json.dumps(results))


if __name__ == "__main__":
    try:
        main()
    except Exception:
        print("Import preview oracle failed", file=sys.stderr)
        sys.exit(1)
