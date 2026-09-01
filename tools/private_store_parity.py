#!/usr/bin/env python3
"""Credential-free projection oracle for the Rust private-store migration."""

from __future__ import annotations

import copy
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import backend  # noqa: E402


def project(raw: object) -> dict[str, object]:
    try:
        store = backend.validate_store(copy.deepcopy(raw))
    except (backend.BackendError, TypeError, ValueError):
        return {"accepted": False}
    counts = {name: 0 for name in ("vless", "trojan", "hysteria2", "tuic")}
    for profile in store["profiles"]:
        counts[str(profile["protocol"])] += 1
    return {
        "accepted": True,
        "version": store["version"],
        "profileCount": len(store["profiles"]),
        "subscriptionCount": len(store["subscriptions"]),
        "protocolCounts": counts,
        "activePresent": bool(store["activeId"]),
        "lastPresent": bool(store["lastId"]),
        "routingPreset": store["routingPreset"],
        "customRuleCount": len(store["customRules"]),
        "startupConfigured": store["startupConfigured"],
        "onboardingComplete": store["onboardingComplete"],
    }


def main() -> int:
    request = json.load(sys.stdin)
    cases = request.get("cases") if isinstance(request, dict) else None
    if not isinstance(cases, list):
        return 2
    result = []
    for case in cases:
        if not isinstance(case, dict) or not isinstance(case.get("id"), str):
            return 2
        result.append({"id": case["id"], **project(case.get("store"))})
    json.dump({"cases": result}, sys.stdout, separators=(",", ":"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
