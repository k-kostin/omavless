#!/usr/bin/env python3
"""Credential-safe Python oracle for Rust connection-pointer parity."""

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
        if not isinstance(request, dict) or set(request) != {"store", "target"}:
            raise ValueError
        target = request["target"]
        if not isinstance(target, dict) or not isinstance(target.get("kind"), str):
            raise ValueError
        store = backend.validate_store(request["store"])
        before = json.dumps(
            store, ensure_ascii=False, sort_keys=True, separators=(",", ":")
        )
        pruned = 0
        if target["kind"] == "connected" and set(target) == {"kind", "profileId"}:
            profile_id = target["profileId"]
            if not isinstance(profile_id, str):
                raise ValueError
            try:
                backend.profile_by_id(store, profile_id)
            except backend.BackendError:
                fail("profile_not_found")
                return 0
            store["activeId"] = profile_id
            store["lastId"] = profile_id
        elif target["kind"] == "disconnected" and set(target) == {
            "kind",
            "pruneMissing",
        }:
            prune = target["pruneMissing"]
            if not isinstance(prune, bool):
                raise ValueError
            store["activeId"] = ""
            if prune:
                removed = {
                    str(profile["id"])
                    for profile in store["profiles"]
                    if profile.get("missing", False)
                }
                store["profiles"] = [
                    profile
                    for profile in store["profiles"]
                    if profile.get("id") not in removed
                ]
                pruned = len(removed)
                if store.get("lastId") in removed:
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
                    "changed": before != canonical.decode(),
                    "pruned": pruned,
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
