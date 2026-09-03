#!/usr/bin/env python3
"""Credential-safe Python oracle for bounded subscription-feed decoding."""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import backend  # noqa: E402


def result(body: object) -> dict[str, object]:
    if not isinstance(body, str):
        return {"ok": False, "code": "subscription_feed_invalid_utf8"}
    try:
        entries, skipped = backend.parse_subscription(body)
    except backend.BackendError as error:
        message = str(error)
        if "more than" in message and "supported links" in message:
            code = "too_many_subscription_entries"
        else:
            code = "subscription_contains_no_supported_profiles"
        return {"ok": False, "code": code}
    return {
        "ok": True,
        "accepted": len(entries),
        "skipped": skipped,
    }


def main() -> int:
    try:
        request = json.load(sys.stdin)
        if (
            not isinstance(request, dict)
            or set(request) != {"version", "cases"}
            or request["version"] != 1
            or not isinstance(request["cases"], list)
        ):
            return 2
        output = []
        for case in request["cases"]:
            if not isinstance(case, dict) or set(case) != {"id", "body"}:
                return 2
            if not isinstance(case["id"], str):
                return 2
            output.append({"id": case["id"], **result(case["body"])})
        json.dump({"version": 1, "cases": output}, sys.stdout, separators=(",", ":"))
        return 0
    except (ValueError, TypeError, KeyError, json.JSONDecodeError):
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
