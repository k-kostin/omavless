#!/usr/bin/env python3
"""Credential-free Python oracle for R3 store relationship semantics."""

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
PROFILE_URI = "vless://00000000-0000-0000-0000-00000000aaaa@example.invalid:443?type=tcp&security=tls"


def classification(message: str) -> str:
    for marker, code in (
        ("Unsupported profile store version", "unsupported_version"),
        ("Profile #", "profile_error"),
        ("duplicate id", "duplicate_id"),
        ("invalid routing preset", "invalid_routing_preset"),
        ("invalid startup target", "invalid_startup_target"),
        ("invalid startup routing mode", "invalid_startup_mode"),
    ):
        if marker in message:
            return code
    return "internal_error"


def exact_classification(message: str) -> str:
    if "Profile #" in message:
        if "invalid id" in message:
            return "invalid_profile_id"
        if "incomplete subscription metadata" in message:
            return "incomplete_subscription_metadata"
        if "unknown subscription" in message:
            return "unknown_subscription"
        if "invalid subscription key" in message:
            return "invalid_subscription_key"
    if "Subscription #" in message and "invalid id" in message:
        return "invalid_subscription_id"
    if "duplicate id" in message:
        return "duplicate_subscription_id" if "Subscription store" in message else "duplicate_profile_id"
    return classification(message)


def project(store: dict) -> dict:
    ids = [profile["id"] for profile in store["profiles"]]
    def index(value: str) -> int:
        try:
            return ids.index(value)
        except ValueError:
            return -1
    startup = store["startup"]
    return {
        "version": store["version"],
        "activeIndex": index(store["activeId"]),
        "lastIndex": index(store["lastId"]),
        "routingPreset": store["routingPreset"],
        "startupConfigured": store["startupConfigured"],
        "startupEnabled": startup["enabled"],
        "startupTarget": startup["target"],
        "startupProfileIndex": index(startup.get("profileId", "")),
        "startupMode": startup["mode"],
        "onboardingComplete": store["onboardingComplete"],
        "profileFacts": [
            [bool(profile.get("subscriptionId")), profile["missing"] if profile.get("subscriptionId") else False, profile["favorite"]]
            for profile in store["profiles"]
        ],
    }


def build(case: dict) -> dict:
    store = {
        "version": case["storeVersion"],
        "profiles": [],
        "subscriptions": [
            {"id": item["id"], "name": f"Subscription {index}",
             "url": f"https://example.invalid/sub/{index}", "updatedAt": 0}
            for index, item in enumerate(case["subscriptions"], 1)
        ],
        "activeId": case["activeId"],
        "lastId": case["lastId"],
    }
    for index, item in enumerate(case["profiles"], 1):
        profile = {"id": item["id"], "name": f"Profile {index}", "uri": PROFILE_URI,
                   "favorite": item["favorite"]}
        if case["storeVersion"] >= 3:
            profile["protocol"] = "vless"
        if item["subscriptionId"] or item["subscriptionKey"]:
            profile.update({"subscriptionId": item["subscriptionId"],
                            "subscriptionKey": item["subscriptionKey"],
                            "missing": item["missing"]})
        store["profiles"].append(profile)
    for source, target in (("routingPreset", "routingPreset"),
                           ("startup", "startup"),
                           ("startupConfigured", "startupConfigured"),
                           ("onboardingComplete", "onboardingComplete")):
        if case[source] is not None:
            store[target] = case[source]
    return store


def main(argv: list[str]) -> int:
    if argv:
        return 2
    try:
        envelope = json.loads(sys.stdin.buffer.read(1024 * 1024 + 1).decode())
        if not isinstance(envelope, dict) or set(envelope) != {"cases"}:
            return 1
        output = []
        for case in envelope["cases"]:
            if not isinstance(case, dict) or not SAFE_ID.fullmatch(case.get("id", "")):
                return 1
            try:
                normalized = backend.validate_store(build(case))
                result = {"accepted": True, "classification": "accepted", **project(normalized)}
            except backend.BackendError as error:
                result = {"accepted": False, "classification": exact_classification(str(error))}
            output.append({"id": case["id"], **result})
        sys.stdout.write(json.dumps({"cases": output}, sort_keys=True, separators=(",", ":")))
        return 0
    except Exception:
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
