#!/usr/bin/env python3
"""Credential-safe Python oracle for native subscription-store mutations."""

from __future__ import annotations

import contextlib
import hashlib
import json
import sys
import tempfile
import uuid
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import backend  # noqa: E402


MAX_INPUT = backend.MAX_STORE_BYTES + backend.MAX_SUBSCRIPTION_BYTES + 16 * 1024


def fail(code: str) -> None:
    print(json.dumps({"ok": False, "code": code}, separators=(",", ":")))


def generated_uuid(value: str, label: str) -> uuid.UUID:
    try:
        parsed = uuid.UUID(value)
    except (AttributeError, ValueError) as exc:
        raise ValueError(f"invalid generated {label} id") from exc
    if str(parsed) != value or parsed.version != 4 or parsed.variant != uuid.RFC_4122:
        raise ValueError(f"invalid generated {label} id")
    return parsed


def error_code(message: str) -> str:
    if message == "too many subscription entries":
        return "too_many_subscription_entries"
    if message == "invalid generated profile id":
        return "invalid_profile_id"
    if message == "invalid generated subscription id":
        return "invalid_subscription_id"
    if "No such subscription" in message:
        return "subscription_not_found"
    if "already added" in message or "duplicate URL" in message:
        return "duplicate_subscription_url"
    if "Disconnect the active subscribed profile" in message:
        return "active_subscription"
    if "name must" in message:
        return "invalid_name"
    if "subscription url" in message.lower():
        return "invalid_subscription_url"
    if "more than one existing profile" in message:
        return "ambiguous_subscription_identity"
    if "Profile store contains duplicate id" in message:
        return "ambiguous_subscription_identity"
    if "profile limit" in message:
        return "too_many_profiles"
    return "invalid_store"


def paths_for(home: Path) -> backend.Paths:
    config = home / ".config" / "omavless"
    runtime = home / "runtime"
    runtime.mkdir(mode=0o700)
    return backend.Paths(
        home,
        config,
        config / "profiles.json",
        config / "route-template.yaml",
        config / "config.yaml",
        home / ".config" / "systemd" / "user" / backend.SERVICE,
        home / "legacy",
        home / "legacy-last",
        runtime,
        home / "state",
    )


def parsed_entries(raw_entries: list[dict[str, object]]) -> list[dict[str, object]]:
    result = []
    for entry in raw_entries:
        if set(entry) != {"uri", "newId"}:
            raise ValueError
        uri = entry["uri"]
        new_id = entry["newId"]
        if not isinstance(uri, str) or not isinstance(new_id, str):
            raise ValueError
        node = backend.parse_profile(uri)
        result.append(
            {
                "key": backend.profile_subscription_key(node["uri"]),
                "uri": node["uri"],
                "node": node,
                "newId": new_id,
            }
        )
    return result


def generated_ids(
    store: dict[str, object], subscription_id: str, entries: list[dict[str, object]]
) -> list[uuid.UUID]:
    existing: dict[str, str] = {}
    for profile in store["profiles"]:
        if profile.get("subscriptionId") != subscription_id:
            continue
        keys = {str(profile.get("subscriptionKey", ""))}
        with contextlib.suppress(backend.BackendError):
            keys.add(backend.profile_subscription_key(str(profile.get("uri", ""))))
        for key in keys - {""}:
            existing[key] = str(profile["id"])
    result = []
    for entry in entries:
        if str(entry["key"]) in existing:
            continue
        candidate = str(entry["newId"])
        result.append(generated_uuid(candidate, "profile"))
    return result


def canonical_result(
    paths: backend.Paths, changed: bool, counts: dict[str, int]
) -> None:
    normalized = backend.load_store(paths)
    canonical = json.dumps(
        normalized, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode()
    print(
        json.dumps(
            {
                "ok": True,
                "changed": changed,
                "counts": counts,
                "sha256": hashlib.sha256(canonical).hexdigest(),
            },
            separators=(",", ":"),
        )
    )


def main() -> int:
    raw = sys.stdin.buffer.read(MAX_INPUT + 1)
    if len(raw) > MAX_INPUT:
        fail("invalid_store")
        return 0
    try:
        request = json.loads(raw)
        if not isinstance(request, dict) or set(request) != {"store", "mutation", "context"}:
            raise ValueError
        mutation = request["mutation"]
        context = request["context"]
        if not isinstance(mutation, dict) or not isinstance(mutation.get("kind"), str):
            raise ValueError
        if (
            not isinstance(context, dict)
            or set(context) != {"activeService"}
            or not isinstance(context["activeService"], bool)
        ):
            raise ValueError
        with tempfile.TemporaryDirectory() as temporary:
            paths = paths_for(Path(temporary))
            backend.ensure_private_dir(paths.config_dir)
            backend.save_store(paths, request["store"])
            kind = mutation["kind"]
            if kind in {"add", "update"}:
                expected = {
                    "kind",
                    "subscriptionId",
                    "name",
                    "url",
                    "entries",
                    "updatedAt",
                }
                if set(mutation) != expected:
                    raise ValueError
                subscription_id = mutation["subscriptionId"]
                updated_at = mutation["updatedAt"]
                if (
                    not isinstance(subscription_id, str)
                    or not isinstance(mutation["name"], str)
                    or not isinstance(mutation["url"], str)
                    or not isinstance(mutation["entries"], list)
                    or not isinstance(updated_at, int)
                    or isinstance(updated_at, bool)
                    or updated_at < 0
                ):
                    raise ValueError
                entries = parsed_entries(mutation["entries"])
                if len(entries) > backend.MAX_SUBSCRIPTION_LINK_COUNT:
                    raise ValueError("too many subscription entries")
                store = backend.load_store(paths)
                generated = generated_ids(store, subscription_id, entries)
                if kind == "add":
                    generated_subscription = generated_uuid(subscription_id, "subscription")
                    generated.insert(0, generated_subscription)
                with mock.patch.object(backend.uuidlib, "uuid4", side_effect=generated), mock.patch.object(
                    backend.time, "time_ns", return_value=updated_at * 1_000_000
                ):
                    result = backend.commit_subscription(
                        paths,
                        mutation["name"],
                        "" if kind == "add" else subscription_id,
                        mutation["url"],
                        entries,
                        0,
                    )
                counts = {key: int(result[key]) for key in ("added", "removed", "stale", "total")}
                canonical_result(paths, True, counts)
                return 0
            if kind == "delete" and set(mutation) == {"kind", "subscriptionId"}:
                subscription_id = mutation["subscriptionId"]
                if not isinstance(subscription_id, str):
                    raise ValueError
                store = backend.load_store(paths)
                removed = sum(
                    profile.get("subscriptionId") == subscription_id
                    for profile in store["profiles"]
                )
                with mock.patch.object(
                    backend, "service_active", return_value=context["activeService"]
                ):
                    backend.delete_subscription(paths, subscription_id)
                canonical_result(
                    paths,
                    True,
                    {"added": 0, "removed": removed, "stale": 0, "total": 0},
                )
                return 0
            raise ValueError
    except (ValueError, TypeError, KeyError, json.JSONDecodeError, backend.BackendError) as error:
        fail(error_code(str(error)))
        return 0


if __name__ == "__main__":
    raise SystemExit(main())
