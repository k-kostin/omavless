#!/usr/bin/env python3
"""Credential-safe Python oracle for Rust profile-mutation parity tests."""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import backend  # noqa: E402


MAX_INPUT = backend.MAX_STORE_BYTES + 4096


def fail(code: str) -> None:
    print(json.dumps({"ok": False, "code": code}, separators=(",", ":")))


def main() -> int:
    raw = sys.stdin.buffer.read(MAX_INPUT + 1)
    if len(raw) > MAX_INPUT:
        fail("invalid_store")
        return 0
    try:
        request = json.loads(raw)
        if not isinstance(request, dict) or set(request) != {"store", "mutation"}:
            raise ValueError
        mutation = request["mutation"]
        if not isinstance(mutation, dict) or not isinstance(mutation.get("kind"), str):
            raise ValueError
        store = backend.validate_store(request["store"])
        profile_id = mutation.get("profileId")
        if not isinstance(profile_id, str):
            raise ValueError
        try:
            profile = backend.profile_by_id(store, profile_id)
        except backend.BackendError:
            fail("profile_not_found")
            return 0
        kind = mutation["kind"]
        changed = True
        if kind == "rename" and set(mutation) == {"kind", "profileId", "newName"}:
            if profile.get("subscriptionId"):
                fail("subscribed_profile")
                return 0
            try:
                new_name = backend.clean_name(mutation["newName"])
            except (backend.BackendError, TypeError):
                fail("invalid_name")
                return 0
            if any(
                item.get("name") == new_name and item.get("id") != profile_id
                for item in store["profiles"]
            ):
                fail("duplicate_profile_name")
                return 0
            changed = profile["name"] != new_name
            profile["name"] = new_name
        elif kind == "favorite" and set(mutation) == {"kind", "profileId", "enabled"}:
            enabled = mutation["enabled"]
            if not isinstance(enabled, bool):
                raise ValueError
            changed = profile.get("favorite", False) != enabled
            profile["favorite"] = enabled
        elif kind == "delete" and set(mutation) == {"kind", "profileId"}:
            if profile.get("subscriptionId"):
                fail("subscribed_profile")
                return 0
            store["profiles"] = [
                item for item in store["profiles"] if item.get("id") != profile_id
            ]
            if store.get("activeId") == profile_id:
                store["activeId"] = ""
            if store.get("lastId") == profile_id:
                store["lastId"] = (
                    store["profiles"][0]["id"] if store["profiles"] else ""
                )
        else:
            raise ValueError
        normalized = backend.validate_store(store)
        canonical = json.dumps(
            normalized, ensure_ascii=False, sort_keys=True, separators=(",", ":")
        ).encode()
        print(
            json.dumps(
                {
                    "ok": True,
                    "changed": changed,
                    "sha256": hashlib.sha256(canonical).hexdigest(),
                },
                separators=(",", ":"),
            )
        )
        return 0
    except (ValueError, TypeError, KeyError, json.JSONDecodeError, backend.BackendError):
        fail("invalid_store")
        return 0


if __name__ == "__main__":
    raise SystemExit(main())
