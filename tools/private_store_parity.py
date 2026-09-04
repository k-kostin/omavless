#!/usr/bin/env python3
"""Credential-free projection oracle for the Rust private-store migration."""

from __future__ import annotations

import copy
import hashlib
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
    subscription_counts = {
        str(subscription["id"]): {
            "profileCount": sum(
                1 for profile in store["profiles"]
                if profile.get("subscriptionId") == subscription["id"]
            ),
            "staleCount": sum(
                1 for profile in store["profiles"]
                if profile.get("subscriptionId") == subscription["id"]
                and bool(profile.get("missing", False))
            ),
        }
        for subscription in store["subscriptions"]
    }
    list_projection = {
        "profiles": [
            {
                "id": str(profile["id"]),
                "name": str(profile["name"]),
                "protocol": str(profile["protocol"]),
                "subscriptionId": str(profile.get("subscriptionId", "")),
                "missing": bool(profile.get("missing", False)),
                "favorite": bool(profile.get("favorite", False)),
            }
            for profile in store["profiles"]
        ],
        "subscriptions": [
            {
                "id": str(subscription["id"]),
                "name": str(subscription["name"]),
                "updatedAt": int(subscription.get("updatedAt", 0)),
                **subscription_counts[str(subscription["id"])],
            }
            for subscription in store["subscriptions"]
        ],
        "lastProfileId": str(store["lastId"]),
    }
    canonical_list = json.dumps(
        list_projection, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode("utf-8")
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
        # Compare exact private display metadata without publishing it from
        # the differential adapter. Only the one-way digest crosses stdout.
        "listDigest": hashlib.sha256(canonical_list).hexdigest(),
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
