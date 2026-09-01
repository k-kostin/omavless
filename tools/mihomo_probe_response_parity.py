#!/usr/bin/env python3
"""Credential-free Python oracle for Mihomo delay-response semantics."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import backend  # noqa: E402

SAFE_ID = re.compile(r"[A-Za-z0-9_.-]{1,96}\Z")
SAFE_ALIAS = re.compile(r"probe-[0-9]{1,3}\Z")


def main(argv: list[str]) -> int:
    if argv:
        return 2
    try:
        envelope = json.loads(sys.stdin.buffer.read(1024 * 1024 + 1).decode("utf-8"))
        if not isinstance(envelope, dict) or set(envelope) != {"cases"}:
            return 1
        output = []
        for case in envelope["cases"]:
            if not isinstance(case, dict) or set(case) != {"id", "status", "payload", "samples"}:
                return 1
            if not isinstance(case["id"], str) or not SAFE_ID.fullmatch(case["id"]):
                return 1
            samples = case["samples"]
            if not isinstance(samples, dict) or len(samples) > 64:
                return 1
            if any(not isinstance(alias, str) or not SAFE_ALIAS.fullmatch(alias)
                   or not isinstance(values, list) or any(type(value) is not int for value in values)
                   for alias, values in samples.items()):
                return 1
            accepted = backend.collect_probe_response(samples, case["status"], case["payload"])
            normalized = {alias: [int(value) for value in values]
                          for alias, values in samples.items()}
            output.append({"id": case["id"], "accepted": accepted, "samples": normalized})
        sys.stdout.write(json.dumps({"cases": output}, sort_keys=True, separators=(",", ":")))
        return 0
    except Exception:
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
