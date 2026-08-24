#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 OmaVLESS contributors
"""Secure VLESS profile manager for the OmaVLESS Omarchy widget."""

from __future__ import annotations

import argparse
import base64
import binascii
import concurrent.futures
import contextlib
import fcntl
import functools
import hashlib
import http.client
import ipaddress
import json
import os
import re
import signal
import shutil
import socket
import stat
import statistics
import struct
import subprocess
import sys
import tempfile
import time
import urllib.parse
import urllib.error
import urllib.request
import uuid as uuidlib
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Iterator


PLUGIN_DIR = Path(__file__).resolve().parent
PLUGIN_VERSION = "0.7.0"
USER_AGENT = f"OmaVLESS/{PLUGIN_VERSION}"
PROFILE_MARKER = "{{OMAVLESS_PROXY}}"
SERVICE = "omavless.service"
STARTUP_SERVICE = "omavless-autostart.service"
CONFLICT_SERVICE = "mihomo.service"
TUN_DEVICE = "Meta"
STATUS_CACHE_SECONDS = 1.0
MAX_VLESS_URI_BYTES = 16 * 1024
MAX_PROFILE_COUNT = 256
MAX_STORE_BYTES = 5 * 1024 * 1024
MAX_TEMPLATE_BYTES = 2 * 1024 * 1024
MAX_IMPORT_BYTES = 64 * 1024
MAX_SUBSCRIPTION_COUNT = 64
MAX_SUBSCRIPTION_LINK_COUNT = 1024
MAX_SUBSCRIPTION_URL_BYTES = 8 * 1024
MAX_SUBSCRIPTION_BYTES = 5 * 1024 * 1024
MAX_CUSTOM_RULE_COUNT = 128
MAX_CUSTOM_RULE_VALUE_BYTES = 1024
SUBSCRIPTION_TIMEOUT_SECONDS = 25.0
PROBE_DNS_TIMEOUT_SECONDS = 2.0
PROBE_HTTP_TIMEOUT_MILLISECONDS = 5000
PROBE_CORE_START_SECONDS = 5.0
PROBE_CORE_STOP_SECONDS = 2.0
PROBE_RESULT_BYTES = 1024 * 1024
PROBE_MAX_ADDRESSES = 4
PROBE_ROUTING_MARK = 0x80000
PROBE_GROUP_NAME = "OMAVLESS_TEST"
PROBE_URLS = (
    "https://www.gstatic.com/generate_204",
    "https://cp.cloudflare.com/generate_204",
    "https://www.google.com/generate_204",
)
ROUTING_PRESETS = {
    "roscomvpn-default": PLUGIN_DIR / "templates" / "default.yaml",
    "china-cn-direct": PLUGIN_DIR / "templates" / "china.yaml",
    "iran-ir-direct": PLUGIN_DIR / "templates" / "iran.yaml",
}
ROUTING_PRESET_VALUES = frozenset((*ROUTING_PRESETS, "custom"))
LOCK_TIMEOUT_SECONDS = 15.0
COMMAND_TIMEOUT_SECONDS = 30.0
REALITY_SPX_COMPATIBILITY_NOTE = (
    "Mihomo does not use the Xray-only REALITY spider path (spx); "
    "the profile will be imported without it."
)
PUBLIC_CAPABILITIES = {
    "subscriptions": True,
    "subscriptionSearch": True,
    "routingModes": True,
    "connectionTest": True,
    "liveTraffic": True,
    "trafficHistory": True,
    "exitIp": True,
    "conflictDetection": True,
    "qr": True,
    "protocols": ["vless"],
    "core": "mihomo",
}


class BackendError(RuntimeError):
    def __init__(self, message: str, exit_code: int = 1):
        super().__init__(message)
        self.exit_code = exit_code


@dataclass(frozen=True)
class Paths:
    home: Path
    config_dir: Path
    store: Path
    template: Path
    config: Path
    unit: Path
    legacy_data_dir: Path
    legacy_last: Path
    runtime: Path

    @classmethod
    def current(cls) -> "Paths":
        home = Path(os.environ.get("OMAVLESS_HOME", str(Path.home()))).expanduser().resolve()
        config_dir = home / ".config" / "omavless"
        legacy_data_dir = home / ".config" / "omarchy" / "omavless"
        runtime = Path(os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}"))
        return cls(home, config_dir, config_dir / "profiles.json",
                   config_dir / "route-template.yaml", config_dir / "config.yaml",
                   home / ".config" / "systemd" / "user" / SERVICE,
                   legacy_data_dir,
                   home / ".local" / "state" / "omarchy" / "vless-last", runtime)


def fail(message: str, code: int = 1) -> None:
    print(message, file=sys.stderr)
    raise SystemExit(code)


def atomic_write(path: Path, data: str | bytes, mode: int = 0o600) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    raw = data.encode() if isinstance(data, str) else data
    fd, name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    temp = Path(name)
    try:
        os.fchmod(fd, mode)
        with os.fdopen(fd, "wb") as handle:
            handle.write(raw)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temp, path)
        os.chmod(path, mode)
        directory_fd = os.open(path.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
    finally:
        with contextlib.suppress(FileNotFoundError):
            temp.unlink()


def ensure_private_dir(path: Path) -> None:
    path.mkdir(mode=0o700, parents=True, exist_ok=True)
    if path.is_symlink():
        raise BackendError(f"Refusing symlinked private directory: {path}")
    info = path.stat()
    if info.st_uid != os.getuid():
        raise BackendError(f"Private directory is not owned by the current user: {path}")
    os.chmod(path, 0o700)


def read_text_file(path: Path, limit: int, label: str, *, private: bool = False) -> str:
    """Read a bounded regular UTF-8 file with useful, non-traceback errors."""
    if private and path.is_symlink():
        raise BackendError(f"Refusing symlinked {label}: {path}")
    try:
        info = path.stat()
    except OSError as exc:
        raise BackendError(f"Cannot open {label}: {exc}") from exc
    if not stat.S_ISREG(info.st_mode):
        raise BackendError(f"{label.capitalize()} must be a regular file: {path}")
    if private and info.st_uid != os.getuid():
        raise BackendError(f"{label.capitalize()} is not owned by the current user: {path}")
    if info.st_size > limit:
        raise BackendError(f"{label.capitalize()} is too large (maximum {limit // 1024} KiB)")
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as exc:
        raise BackendError(f"Cannot read {label} as UTF-8: {exc}") from exc
    if private:
        try:
            os.chmod(path, 0o600)
        except OSError as exc:
            raise BackendError(f"Cannot secure {label}: {exc}") from exc
    return text


def read_stdin_text(limit: int, label: str) -> str:
    """Read bounded UTF-8 from stdin without splitting a multibyte sequence."""
    try:
        raw = sys.stdin.buffer.read(limit + 1)
    except (AttributeError, OSError):
        # StringIO is useful to embedders and tests; the real CLI always has
        # a binary buffer, which is what makes the byte limit exact.
        try:
            text = sys.stdin.read(limit + 1)
        except (OSError, UnicodeError) as exc:
            raise BackendError(f"Cannot read {label}: {exc}") from exc
        raw = text.encode("utf-8")
    if len(raw) > limit:
        raise BackendError(f"{label.capitalize()} is too large (maximum {limit // 1024} KiB)")
    try:
        return raw.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise BackendError(f"Cannot read {label} as UTF-8: {exc}") from exc


def ensure_runtime(paths: Paths) -> None:
    if not paths.runtime.is_dir() or paths.runtime.is_symlink():
        raise BackendError("A private XDG_RUNTIME_DIR is required")
    info = paths.runtime.stat()
    if info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) & 0o077:
        raise BackendError("XDG_RUNTIME_DIR has unsafe permissions")


@contextlib.contextmanager
def operation_lock(paths: Paths, timeout: float = LOCK_TIMEOUT_SECONDS) -> Iterator[None]:
    ensure_runtime(paths)
    with (paths.runtime / f"omavless.{os.getuid()}.lock").open("a", encoding="utf-8") as handle:
        deadline = time.monotonic() + timeout
        while True:
            try:
                fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except BlockingIOError:
                if time.monotonic() >= deadline:
                    raise BackendError("Another OmaVLESS operation is taking too long")
                time.sleep(0.05)
        try:
            yield
        finally:
            fcntl.flock(handle.fileno(), fcntl.LOCK_UN)


def status_cache(paths: Paths) -> Path:
    return paths.runtime / f"omavless.{os.getuid()}.status"


def invalidate_status_cache(paths: Paths) -> None:
    with contextlib.suppress(FileNotFoundError):
        status_cache(paths).unlink()


def empty_store() -> dict[str, Any]:
    return {
        "version": 2, "activeId": "", "lastId": "",
        "profiles": [], "subscriptions": [], "routingPreset": "",
        "customRules": [], "rulesUpdatedAt": 0,
        "startupConfigured": True,
        "startup": {
            "enabled": False, "target": "last", "profileId": "", "mode": "rule",
        },
        "onboardingComplete": False,
    }


def validate_store(data: Any) -> dict[str, Any]:
    if not isinstance(data, dict) or not isinstance(data.get("profiles"), list):
        raise BackendError("VLESS profile store has an invalid format")
    version = data.get("version", 1)
    if version not in {1, 2}:
        raise BackendError(f"Unsupported VLESS profile store version: {data.get('version')}")
    # Version 1 had only local profiles. Migration is deliberately in-memory:
    # the next real mutation writes v2 atomically, while a status poll does not
    # churn a private credential file merely because the plugin was upgraded.
    subscriptions = data.setdefault("subscriptions", [])
    if not isinstance(subscriptions, list):
        raise BackendError("VLESS subscription store has an invalid format")
    if len(subscriptions) > MAX_SUBSCRIPTION_COUNT:
        raise BackendError(
            f"VLESS subscription store contains more than {MAX_SUBSCRIPTION_COUNT} subscriptions"
        )
    seen_subscription_ids: set[str] = set()
    seen_subscription_urls: set[str] = set()
    for index, subscription in enumerate(subscriptions, start=1):
        if not isinstance(subscription, dict):
            raise BackendError(f"VLESS subscription #{index} has an invalid format")
        subscription_id = subscription.get("id")
        name = subscription.get("name")
        url = subscription.get("url")
        updated_at = subscription.get("updatedAt", 0)
        if not isinstance(subscription_id, str) or not isinstance(name, str) or not isinstance(url, str):
            raise BackendError(f"VLESS subscription #{index} is missing id, name or url")
        try:
            uuidlib.UUID(subscription_id)
        except ValueError as exc:
            raise BackendError(f"VLESS subscription #{index} has an invalid id") from exc
        if subscription_id in seen_subscription_ids:
            raise BackendError(f"VLESS subscription store contains duplicate id: {subscription_id}")
        seen_subscription_ids.add(subscription_id)
        if clean_name(name) != name:
            raise BackendError(f"VLESS subscription #{index} has a non-canonical name")
        canonical_url = validate_subscription_url(url)
        if canonical_url != url:
            raise BackendError(f"VLESS subscription #{index} has a non-canonical URL")
        if canonical_url in seen_subscription_urls:
            raise BackendError("VLESS subscription store contains a duplicate URL")
        seen_subscription_urls.add(canonical_url)
        if not isinstance(updated_at, int) or isinstance(updated_at, bool) or updated_at < 0:
            raise BackendError(f"VLESS subscription #{index} has an invalid update time")
        subscription["updatedAt"] = updated_at
    profiles = data["profiles"]
    if len(profiles) > MAX_PROFILE_COUNT:
        raise BackendError(f"VLESS profile store contains more than {MAX_PROFILE_COUNT} profiles")
    seen_ids: set[str] = set()
    for index, profile in enumerate(profiles, start=1):
        if not isinstance(profile, dict):
            raise BackendError(f"VLESS profile #{index} has an invalid format")
        profile_id = profile.get("id")
        name = profile.get("name")
        uri = profile.get("uri")
        if not isinstance(profile_id, str) or not isinstance(name, str) or not isinstance(uri, str):
            raise BackendError(f"VLESS profile #{index} is missing id, name or uri")
        try:
            uuidlib.UUID(profile_id)
        except ValueError as exc:
            raise BackendError(f"VLESS profile #{index} has an invalid id") from exc
        if profile_id in seen_ids:
            raise BackendError(f"VLESS profile store contains duplicate id: {profile_id}")
        seen_ids.add(profile_id)
        if clean_name(name) != name:
            raise BackendError(f"VLESS profile #{index} has a non-canonical name")
        parse_vless(uri)
        subscription_id = profile.get("subscriptionId", "")
        subscription_key = profile.get("subscriptionKey", "")
        missing = profile.get("missing", False)
        favorite = profile.get("favorite", False)
        if not isinstance(subscription_id, str) or not isinstance(subscription_key, str):
            raise BackendError(f"VLESS profile #{index} has invalid subscription metadata")
        if not isinstance(missing, bool):
            raise BackendError(f"VLESS profile #{index} has an invalid missing flag")
        if not isinstance(favorite, bool):
            raise BackendError(f"VLESS profile #{index} has an invalid favorite flag")
        if bool(subscription_id) != bool(subscription_key):
            raise BackendError(f"VLESS profile #{index} has incomplete subscription metadata")
        if subscription_id and subscription_id not in seen_subscription_ids:
            raise BackendError(f"VLESS profile #{index} references an unknown subscription")
        if subscription_key and not re.fullmatch(r"[0-9a-f]{64}", subscription_key):
            raise BackendError(f"VLESS profile #{index} has an invalid subscription key")
        if subscription_id:
            profile["subscriptionId"] = subscription_id
            profile["subscriptionKey"] = subscription_key
            profile["missing"] = missing
        else:
            profile.pop("subscriptionId", None)
            profile.pop("subscriptionKey", None)
            profile.pop("missing", None)
        profile["favorite"] = favorite
    for key in ("activeId", "lastId"):
        value = data.setdefault(key, "")
        if not isinstance(value, str):
            raise BackendError(f"VLESS profile store field {key} must be a string")
        if value and value not in seen_ids:
            # These are convenience pointers, not credentials. A service
            # killed outside OmaVLESS or an interrupted old delete can leave
            # one stale; self-heal it instead of hiding every valid profile.
            data[key] = ""
    # Releases before 0.7.0 always shipped the RoscomVPN template but did not
    # remember whether the user had deliberately selected it. Existing stores
    # with profiles are migrated to that historical choice; a new store has an
    # explicit empty value and therefore gets the country prompt on the first
    # Routing click.
    if "routingPreset" not in data:
        data["routingPreset"] = "roscomvpn-default" if profiles else ""
    routing_preset = data["routingPreset"]
    if not isinstance(routing_preset, str) or routing_preset not in ROUTING_PRESET_VALUES | {""}:
        raise BackendError("VLESS profile store has an invalid routing preset")
    custom_rules = data.setdefault("customRules", [])
    if not isinstance(custom_rules, list) or len(custom_rules) > MAX_CUSTOM_RULE_COUNT:
        raise BackendError("VLESS profile store has invalid custom routing rules")
    seen_custom_rules: set[tuple[str, str]] = set()
    for index, rule in enumerate(custom_rules, start=1):
        if not isinstance(rule, dict):
            raise BackendError(f"Custom routing rule #{index} has an invalid format")
        rule_id = rule.get("id")
        kind = rule.get("kind")
        value = rule.get("value")
        action = rule.get("action")
        if not all(isinstance(item, str) for item in (rule_id, kind, value, action)):
            raise BackendError(f"Custom routing rule #{index} is incomplete")
        try:
            uuidlib.UUID(rule_id)
        except ValueError as exc:
            raise BackendError(f"Custom routing rule #{index} has an invalid id") from exc
        if kind not in {"domain", "suffix", "ipcidr"}:
            raise BackendError(f"Custom routing rule #{index} has an invalid match type")
        if action not in {"proxy", "direct", "reject"}:
            raise BackendError(f"Custom routing rule #{index} has an invalid action")
        if canonical_custom_rule_value(kind, value) != value:
            raise BackendError(f"Custom routing rule #{index} has a non-canonical value")
        key = (kind, value)
        if key in seen_custom_rules:
            raise BackendError("VLESS profile store contains a duplicate custom routing rule")
        seen_custom_rules.add(key)
    rules_updated_at = data.setdefault("rulesUpdatedAt", 0)
    if (not isinstance(rules_updated_at, int) or isinstance(rules_updated_at, bool)
            or rules_updated_at < 0):
        raise BackendError("VLESS profile store has an invalid rule update time")
    # New installs start with login autoconnect off. Older stores keep the
    # exact systemd-enabled behavior they already had until the user saves an
    # explicit choice in Settings; startupConfigured distinguishes those two
    # cases without status polling mutating the private credential store.
    if "startup" not in data:
        data["startup"] = {
            "enabled": False, "target": "last", "profileId": "", "mode": "rule",
        }
        data["startupConfigured"] = False
    startup_configured = data.setdefault("startupConfigured", True)
    if not isinstance(startup_configured, bool):
        raise BackendError("VLESS profile store has invalid startup configuration state")
    startup = data["startup"]
    if not isinstance(startup, dict):
        raise BackendError("VLESS profile store has invalid startup settings")
    startup_enabled = startup.get("enabled")
    startup_target = startup.get("target")
    startup_profile_id = startup.get("profileId", "")
    startup_mode = startup.get("mode")
    if not isinstance(startup_enabled, bool):
        raise BackendError("VLESS profile store has an invalid startup enabled value")
    if startup_target not in {"last", "profile"}:
        raise BackendError("VLESS profile store has an invalid startup target")
    if not isinstance(startup_profile_id, str):
        raise BackendError("VLESS profile store has an invalid startup profile")
    if startup_profile_id and startup_profile_id not in seen_ids:
        startup["enabled"] = False
        startup["profileId"] = ""
    if startup_target == "profile" and not startup["profileId"]:
        startup["enabled"] = False
    if startup_target == "last" and not profiles:
        startup["enabled"] = False
    if startup_mode not in {"rule", "global"}:
        raise BackendError("VLESS profile store has an invalid startup routing mode")
    onboarding_complete = data.setdefault("onboardingComplete", bool(profiles))
    if not isinstance(onboarding_complete, bool):
        raise BackendError("VLESS profile store has invalid onboarding state")
    data["version"] = 2
    return data


def load_store(paths: Paths) -> dict[str, Any]:
    if paths.store.is_symlink():
        raise BackendError(f"Refusing symlinked VLESS profile store: {paths.store}")
    if not paths.store.exists():
        return empty_store()
    try:
        data = json.loads(read_text_file(paths.store, MAX_STORE_BYTES, "VLESS profile store", private=True))
    except json.JSONDecodeError as exc:
        raise BackendError(f"Cannot read VLESS profiles: {exc}") from exc
    return validate_store(data)


def save_store(paths: Paths, store: dict[str, Any]) -> None:
    payload = json.dumps(validate_store(store), ensure_ascii=False, indent=2) + "\n"
    if len(payload.encode("utf-8")) > MAX_STORE_BYTES:
        raise BackendError(f"VLESS profile store is too large (maximum {MAX_STORE_BYTES // 1024} KiB)")
    atomic_write(paths.store, payload)
    invalidate_status_cache(paths)


def migrate_legacy_data(paths: Paths) -> None:
    """Move prototype-era state out of Omarchy's own configuration tree.

    Copy-then-unlink makes the migration restartable. A conflicting target is
    never overwritten: identical leftovers are cleaned up, while different
    files stop with both copies intact for explicit recovery.
    """
    pairs = (
        (paths.legacy_data_dir / "profiles.json", paths.store, MAX_STORE_BYTES, "legacy profile store"),
        (paths.legacy_data_dir / "route-template.yaml", paths.template, MAX_TEMPLATE_BYTES, "legacy route template"),
    )
    for source, target, limit, label in pairs:
        if not source.exists():
            continue
        if source.is_symlink() or not source.is_file():
            raise BackendError(f"Refusing unsafe legacy data path: {source}")
        payload = read_text_file(source, limit, label, private=True).encode()
        if target.exists():
            if target.is_symlink() or not target.is_file():
                raise BackendError(f"Refusing unsafe OmaVLESS data path: {target}")
            if read_text_file(target, limit, label, private=True).encode() != payload:
                raise BackendError(
                    f"Both legacy and current OmaVLESS data exist and differ: {source} and {target}"
                )
        else:
            atomic_write(target, payload)
        source.unlink()

    # The old one-value state file predates lastId in profiles.json. Adopt it
    # only when the store has no last selection, then retire that exact file.
    if paths.legacy_last.exists():
        if paths.legacy_last.is_symlink() or not paths.legacy_last.is_file():
            raise BackendError(f"Refusing unsafe legacy state path: {paths.legacy_last}")
        value = read_text_file(paths.legacy_last, 64 * 1024, "legacy selection", private=True).strip()
        store = load_store(paths)
        if value and not store.get("lastId"):
            match = next((p for p in store["profiles"]
                          if p.get("id") == value or p.get("name") == value), None)
            if match:
                store["lastId"] = str(match["id"])
                save_store(paths, store)
        paths.legacy_last.unlink()

    with contextlib.suppress(OSError):
        paths.legacy_data_dir.rmdir()


def clean_name(value: str) -> str:
    value = re.sub(r"[\x00-\x1f\x7f]", "", value).strip()
    if not value:
        raise BackendError("Profile name must not be empty")
    if len(value) > 80:
        raise BackendError("Profile name must be at most 80 characters")
    return value


def canonical_domain(value: str) -> str:
    text = value.strip().rstrip(".")
    if not text or len(text.encode("utf-8")) > MAX_CUSTOM_RULE_VALUE_BYTES:
        raise BackendError("Domain must not be empty or larger than 1 KiB")
    if re.search(r"[\x00-\x20\x7f/:?#@]", text):
        raise BackendError("Enter a domain without a scheme, port, path or spaces")
    try:
        domain = text.encode("idna").decode("ascii").lower()
    except UnicodeError as exc:
        raise BackendError("Domain is not valid IDNA") from exc
    if len(domain) > 253:
        raise BackendError("Domain is too long")
    labels = domain.split(".")
    if len(labels) < 2 or any(
        not re.fullmatch(r"[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?", label)
        for label in labels
    ):
        raise BackendError("Enter a valid domain such as example.com")
    return domain


def canonical_custom_rule_value(kind: str, value: str) -> str:
    if kind in {"domain", "suffix"}:
        text = value.strip()
        if kind == "suffix":
            text = text.removeprefix("*.").removeprefix(".")
        return canonical_domain(text)
    if kind == "ipcidr":
        text = value.strip()
        if not text or len(text.encode("utf-8")) > MAX_CUSTOM_RULE_VALUE_BYTES:
            raise BackendError("IP range must not be empty or larger than 1 KiB")
        try:
            return str(ipaddress.ip_network(text, strict=False))
        except ValueError as exc:
            raise BackendError("Enter an IP address or CIDR range") from exc
    raise BackendError("Unsupported custom routing match type")


def validate_subscription_url(value: str) -> str:
    """Validate a bearer-like subscription URL without ever logging it."""
    value = value.strip()
    if not value:
        raise BackendError("Subscription URL must not be empty")
    if len(value.encode("utf-8")) > MAX_SUBSCRIPTION_URL_BYTES:
        raise BackendError(
            f"Subscription URL is too large (maximum {MAX_SUBSCRIPTION_URL_BYTES // 1024} KiB)"
        )
    if re.search(r"[\x00-\x20\x7f]", value):
        raise BackendError("Subscription URL contains whitespace or control characters")
    try:
        parsed = urllib.parse.urlsplit(value)
        port = parsed.port
    except ValueError as exc:
        raise BackendError("Subscription URL is invalid") from exc
    if parsed.scheme.lower() not in {"http", "https"}:
        raise BackendError("Subscription URL must use http:// or https://")
    if not parsed.hostname:
        raise BackendError("Subscription URL must include a host")
    if parsed.username is not None or parsed.password is not None:
        raise BackendError("Subscription URL must not contain userinfo")
    if port is not None and not 1 <= port <= 65535:
        raise BackendError("Subscription URL has an invalid port")
    if parsed.fragment:
        raise BackendError("Subscription URL must not contain a fragment")
    if parsed.scheme.lower() == "http":
        hostname = (parsed.hostname or "").lower()
        loopback = hostname == "localhost" or hostname.endswith(".localhost")
        try:
            loopback = loopback or ipaddress.ip_address(hostname).is_loopback
        except ValueError:
            pass
        if not loopback:
            raise BackendError("Remote subscription URLs must use HTTPS")
    return value


def subscription_host(url: str) -> str:
    """Return the only URL-derived value safe enough for user-facing errors."""
    try:
        return urllib.parse.urlsplit(url).hostname or "provider"
    except ValueError:
        return "provider"


class SubscriptionRedirectHandler(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req: Any, fp: Any, code: int, msg: str,
                         headers: Any, newurl: str) -> Any:
        validate_subscription_url(newurl)
        return super().redirect_request(req, fp, code, msg, headers, newurl)


def fetch_subscription(url: str) -> str:
    """Fetch bounded UTF-8 subscription text with normal TLS verification."""
    url = validate_subscription_url(url)
    request = urllib.request.Request(
        url,
        headers={
            "User-Agent": USER_AGENT,
            "Accept": "text/plain, application/octet-stream;q=0.9, */*;q=0.1",
            "Accept-Encoding": "identity",
        },
    )
    opener = urllib.request.build_opener(SubscriptionRedirectHandler())
    try:
        with opener.open(request, timeout=SUBSCRIPTION_TIMEOUT_SECONDS) as response:
            validate_subscription_url(response.geturl())
            raw = response.read(MAX_SUBSCRIPTION_BYTES + 1)
    except urllib.error.HTTPError as exc:
        # HTTPError is also a response object and may own a socket-backed
        # stream. We only need the status code; close the response before the
        # redacted error escapes so repeated refresh failures do not leak FDs.
        code = exc.code
        exc.close()
        raise BackendError(
            f"Subscription provider {subscription_host(url)} returned HTTP {code}"
        ) from exc
    except urllib.error.URLError as exc:
        reason = str(exc.reason).replace("\n", " ")[:160]
        raise BackendError(
            f"Could not download subscription from {subscription_host(url)}: {reason}"
        ) from exc
    except TimeoutError as exc:
        raise BackendError(
            f"Subscription provider {subscription_host(url)} timed out"
        ) from exc
    if len(raw) > MAX_SUBSCRIPTION_BYTES:
        raise BackendError(
            f"Subscription response is too large (maximum {MAX_SUBSCRIPTION_BYTES // 1024} KiB)"
        )
    try:
        return raw.decode("utf-8-sig")
    except UnicodeDecodeError as exc:
        raise BackendError("Subscription response is not UTF-8 text") from exc


def subscription_key(uri: str) -> str:
    parsed = urllib.parse.urlsplit(extract_vless_uri(uri))
    # Labels and query ordering are presentation details. Normalizing them
    # keeps a provider-side rename or serializer change attached to the same
    # local UUID, which is what preserves selection and row identity.
    host = (parsed.hostname or "").lower()
    if ":" in host:
        host = f"[{host}]"
    netloc = f"{urllib.parse.unquote(parsed.username or '').lower()}@{host}:{parsed.port}"
    query = urllib.parse.urlencode(sorted(urllib.parse.parse_qsl(
        parsed.query, keep_blank_values=True, max_num_fields=128
    )), doseq=True)
    canonical = urllib.parse.urlunsplit(
        (parsed.scheme.lower(), netloc, parsed.path, query, "")
    )
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()


def _subscription_candidates(text: str) -> list[str]:
    return re.findall(r"(?i)vless://[^\s\"'<>]+", text)


def parse_subscription(text: str) -> tuple[list[dict[str, Any]], int]:
    """Parse raw or base64 VLESS subscriptions, ignoring other protocols."""
    candidates = _subscription_candidates(text)
    if not candidates:
        compact = re.sub(r"\s+", "", text)
        if compact and re.fullmatch(r"[A-Za-z0-9_+/=-]+", compact):
            try:
                padded = compact + "=" * (-len(compact) % 4)
                decoded = base64.b64decode(padded, altchars=b"-_", validate=True)
                candidates = _subscription_candidates(decoded.decode("utf-8-sig"))
            except (binascii.Error, UnicodeDecodeError, ValueError):
                candidates = []
    if len(candidates) > MAX_SUBSCRIPTION_LINK_COUNT:
        raise BackendError(
            f"Subscription contains more than {MAX_SUBSCRIPTION_LINK_COUNT} VLESS links"
        )
    valid: list[dict[str, Any]] = []
    seen: set[str] = set()
    skipped = 0
    for candidate in candidates:
        try:
            node = parse_vless(candidate)
            key = subscription_key(node["uri"])
        except BackendError:
            skipped += 1
            continue
        if key in seen:
            skipped += 1
            continue
        seen.add(key)
        valid.append({"key": key, "uri": node["uri"], "node": node})
    if not valid:
        suffix = f" ({skipped} invalid VLESS links found)" if skipped else ""
        raise BackendError("Subscription contains no supported VLESS profiles" + suffix)
    return valid, skipped


def unique_profile_name(desired: str, subscription_name: str, used: set[str]) -> str:
    # Provider labels are untrusted display data, not local form input. Strip
    # controls and truncate instead of rejecting a whole otherwise-valid feed
    # because one marketing-heavy node name exceeds the local UI limit.
    base = re.sub(r"[\x00-\x1f\x7f]", "", desired).strip() or "VLESS"
    base = base[:80]
    source_suffix = f" · {subscription_name[:76]}"
    choices = [base, clean_name(base[:max(1, 80 - len(source_suffix))] + source_suffix)]
    for choice in choices:
        if choice not in used:
            used.add(choice)
            return choice
    for number in range(2, MAX_PROFILE_COUNT + 2):
        suffix = f" · {number}"
        candidate = clean_name(base[:max(1, 80 - len(suffix))] + suffix)
        if candidate not in used:
            used.add(candidate)
            return candidate
    raise BackendError("Could not create a unique profile name")


def subscription_by_id(store: dict[str, Any], subscription_id: str) -> dict[str, Any]:
    for subscription in store["subscriptions"]:
        if subscription.get("id") == subscription_id:
            return subscription
    raise BackendError("No such VLESS subscription")


def sync_subscription_store(store: dict[str, Any], subscription: dict[str, Any],
                            entries: list[dict[str, Any]], updated_at: int) -> dict[str, int]:
    subscription_id = str(subscription["id"])
    existing = {
        str(profile.get("subscriptionKey")): profile
        for profile in store["profiles"]
        if profile.get("subscriptionId") == subscription_id
    }
    used_names = {
        str(profile["name"])
        for profile in store["profiles"]
        if profile.get("subscriptionId") != subscription_id
    }
    incoming_keys = {str(entry["key"]) for entry in entries}
    active_id = str(store.get("activeId", ""))
    synced: list[dict[str, Any]] = []
    added = 0
    for entry in entries:
        key = str(entry["key"])
        node = entry["node"]
        desired = str(node.get("suggested_name") or node.get("server") or "VLESS")
        name = unique_profile_name(desired, str(subscription["name"]), used_names)
        profile = existing.get(key)
        if profile is None:
            profile = {"id": str(uuidlib.uuid4())}
            added += 1
        profile.update({
            "name": name,
            "uri": str(entry["uri"]),
            "subscriptionId": subscription_id,
            "subscriptionKey": key,
            "missing": False,
        })
        synced.append(profile)

    removed_ids: set[str] = set()
    stale = 0
    for key, profile in existing.items():
        if key in incoming_keys:
            continue
        if profile.get("id") == active_id:
            profile["missing"] = True
            used_names.add(str(profile["name"]))
            synced.append(profile)
            stale += 1
        else:
            removed_ids.add(str(profile["id"]))

    others = [
        profile for profile in store["profiles"]
        if profile.get("subscriptionId") != subscription_id
    ]
    if len(others) + len(synced) > MAX_PROFILE_COUNT:
        raise BackendError(
            f"Subscription would exceed the {MAX_PROFILE_COUNT}-profile limit"
        )
    store["profiles"] = others + synced
    if store.get("lastId") in removed_ids:
        store["lastId"] = store["profiles"][0]["id"] if store["profiles"] else ""
    subscription["updatedAt"] = updated_at
    return {"added": added, "removed": len(removed_ids), "stale": stale, "total": len(entries)}


def commit_subscription(paths: Paths, name: str, subscription_id: str, url: str,
                        entries: list[dict[str, Any]], skipped: int) -> dict[str, int]:
    name = clean_name(name)
    url = validate_subscription_url(url)
    store = load_store(paths)
    if any(item.get("url") == url and item.get("id") != subscription_id
           for item in store["subscriptions"]):
        raise BackendError("That subscription URL is already added")
    if subscription_id:
        subscription = subscription_by_id(store, subscription_id)
        subscription.update({"name": name, "url": url})
    else:
        if len(store["subscriptions"]) >= MAX_SUBSCRIPTION_COUNT:
            raise BackendError(f"At most {MAX_SUBSCRIPTION_COUNT} subscriptions are supported")
        subscription = {
            "id": str(uuidlib.uuid4()), "name": name, "url": url, "updatedAt": 0,
        }
        store["subscriptions"].append(subscription)
    result = sync_subscription_store(store, subscription, entries, time.time_ns() // 1_000_000)
    result["skipped"] = skipped
    save_store(paths, store)
    return result


def save_subscription(paths: Paths, name: str, subscription_id: str, url: str) -> dict[str, int]:
    entries, skipped = parse_subscription(fetch_subscription(validate_subscription_url(url)))
    return commit_subscription(paths, name, subscription_id, url, entries, skipped)


def refresh_subscription(paths: Paths, subscription_id: str) -> dict[str, int]:
    store = load_store(paths)
    subscription = subscription_by_id(store, subscription_id)
    entries, skipped = parse_subscription(fetch_subscription(str(subscription["url"])))
    result = sync_subscription_store(store, subscription, entries, time.time_ns() // 1_000_000)
    result["skipped"] = skipped
    save_store(paths, store)
    return result


def configured_probe_resolvers(paths: Paths) -> list[str]:
    """Return every HTTPS DNS resolver selected by the active policy.

    Mihomo resolver annotations such as ``#PROXY`` are routing metadata, not
    URL fragments understood by urllib, so they are stripped. Order is kept:
    direct resolvers are tried first, then proxy/server resolvers and the main
    nameserver list. A broken first entry must not hide a working fallback.
    """
    result: list[str] = []
    candidates = (paths.config, paths.template)
    for path in candidates:
        if not path.exists() or path.is_symlink() or not path.is_file():
            continue
        with contextlib.suppress(BackendError, OSError):
            lines = read_text_file(path, MAX_TEMPLATE_BYTES, "routing DNS policy", private=True).splitlines()
            for key in ("direct-nameserver", "proxy-server-nameserver", "nameserver"):
                for index, line in enumerate(lines):
                    match = re.match(rf"^(\s*){re.escape(key)}\s*:\s*$", line)
                    if not match:
                        continue
                    indent = len(match.group(1))
                    for value_line in lines[index + 1:]:
                        if value_line.strip() == "" or value_line.lstrip().startswith("#"):
                            continue
                        value_indent = len(value_line) - len(value_line.lstrip())
                        if value_indent <= indent:
                            break
                        item = re.sub(r"^\s*-\s*", "", value_line).strip().strip("'\"")
                        item = item.split("#", 1)[0]
                        with contextlib.suppress(ValueError):
                            parsed = urllib.parse.urlsplit(item)
                            if (parsed.scheme == "https" and parsed.hostname
                                    and parsed.username is None and parsed.password is None):
                                normalized = urllib.parse.urlunsplit(parsed._replace(fragment=""))
                                if normalized not in result:
                                    result.append(normalized)
            if result:
                break
    return result


def configured_probe_resolver(paths: Paths) -> str | None:
    """Compatibility wrapper returning the first configured resolver."""
    resolvers = configured_probe_resolvers(paths)
    return resolvers[0] if resolvers else None


def dns_question(host: str, transaction_id: int, record_type: int = 1) -> bytes:
    labels = host.rstrip(".").encode("idna").split(b".")
    if not labels or any(not label or len(label) > 63 for label in labels):
        raise ValueError("invalid DNS name")
    qname = b"".join(bytes((len(label),)) + label for label in labels) + b"\0"
    if record_type not in (1, 28):
        raise ValueError("unsupported DNS record type")
    return (struct.pack("!HHHHHH", transaction_id, 0x0100, 1, 0, 0, 0)
            + qname + struct.pack("!HH", record_type, 1))


def skip_dns_name(packet: bytes, offset: int) -> int:
    while offset < len(packet):
        length = packet[offset]
        if length & 0xC0 == 0xC0:
            if offset + 1 >= len(packet):
                raise ValueError("truncated DNS pointer")
            return offset + 2
        offset += 1
        if length == 0:
            return offset
        if length > 63 or offset + length > len(packet):
            raise ValueError("invalid DNS label")
        offset += length
    raise ValueError("truncated DNS name")


def parse_dns_addresses(packet: bytes, transaction_id: int) -> list[str]:
    if len(packet) < 12:
        return []
    reply_id, flags, questions, answers, authority, additional = struct.unpack("!HHHHHH", packet[:12])
    if reply_id != transaction_id or flags & 0x8000 == 0 or flags & 0x000F != 0:
        return []
    offset = 12
    result: list[str] = []
    try:
        for _ in range(questions):
            offset = skip_dns_name(packet, offset) + 4
            if offset > len(packet):
                return []
        # Some resolvers put address records for a CNAME into the additional
        # section. Walk every RR instead of assuming all useful A/AAAA values
        # live in the answer section.
        for _ in range(answers + authority + additional):
            offset = skip_dns_name(packet, offset)
            if offset + 10 > len(packet):
                return []
            record_type, record_class, _ttl, length = struct.unpack("!HHIH", packet[offset:offset + 10])
            offset += 10
            if offset + length > len(packet):
                return []
            value = packet[offset:offset + length]
            offset += length
            if record_class != 1 or (record_type, length) not in ((1, 4), (28, 16)):
                continue
            address = ipaddress.ip_address(value)
            text = str(address)
            if address.is_global and text not in result:
                result.append(text)
    except (ValueError, struct.error):
        return []
    return result


def parse_dns_ipv4(packet: bytes, transaction_id: int) -> str | None:
    """Compatibility helper returning the first public IPv4 answer."""
    return next((value for value in parse_dns_addresses(packet, transaction_id)
                 if ipaddress.ip_address(value).version == 4), None)


@functools.lru_cache(maxsize=2048)
def resolve_probe_records(host: str, resolver_url: str, record_type: int) -> tuple[str, ...]:
    """Resolve public A/AAAA records without trusting TUN fake-IP DNS."""
    try:
        address = ipaddress.ip_address(host)
        expected = 4 if record_type == 1 else 6
        return (str(address),) if address.version == expected and address.is_global else ()
    except ValueError:
        pass
    if not resolver_url:
        return ()
    transaction_id = int.from_bytes(os.urandom(2), "big")
    try:
        body = dns_question(host, transaction_id, record_type)
    except (UnicodeError, ValueError):
        return ()
    request = urllib.request.Request(
        resolver_url,
        data=body,
        method="POST",
        headers={
            "User-Agent": USER_AGENT,
            "Accept": "application/dns-message",
            "Content-Type": "application/dns-message",
            "Accept-Encoding": "identity",
        },
    )
    try:
        with urllib.request.urlopen(request, timeout=PROBE_DNS_TIMEOUT_SECONDS) as response:
            raw = response.read(64 * 1024 + 1)
        if len(raw) > 64 * 1024:
            return ()
    except (OSError, TimeoutError, ValueError):
        return ()
    return tuple(parse_dns_addresses(raw, transaction_id))


@functools.lru_cache(maxsize=512)
def resolve_probe_host(host: str, resolver_url: str) -> str | None:
    """Compatibility helper returning one public IPv4 from one resolver."""
    return next((value for value in resolve_probe_records(host, resolver_url, 1)
                 if ipaddress.ip_address(value).version == 4), None)


@functools.lru_cache(maxsize=64)
def probe_resolver_works(resolver_url: str) -> bool:
    """Cheap health check used once per session to skip broken DoH entries."""
    return bool(resolve_probe_records("example.com", resolver_url, 1))


def configured_working_probe_resolvers(paths: Paths) -> list[str]:
    configured = configured_probe_resolvers(paths)
    working = [resolver for resolver in configured if probe_resolver_works(resolver)]
    return working or configured


def system_probe_addresses(host: str) -> list[str]:
    """Use libc DNS only as a last resort, rejecting Mihomo fake-IP results."""
    result: list[str] = []
    with contextlib.suppress(OSError, socket.gaierror):
        for family, _kind, _proto, _canon, sockaddr in socket.getaddrinfo(
                host, None, socket.AF_UNSPEC, socket.SOCK_STREAM):
            if family not in (socket.AF_INET, socket.AF_INET6):
                continue
            with contextlib.suppress(ValueError):
                address = ipaddress.ip_address(sockaddr[0])
                text = str(address)
                if address.is_global and text not in result:
                    result.append(text)
    return result


def resolve_probe_addresses(host: str, resolver_urls: str | list[str] | tuple[str, ...]) -> list[str]:
    """Return bounded public IPv4/IPv6 candidates with resolver fallback."""
    try:
        address = ipaddress.ip_address(host)
        return [str(address)] if address.is_global else []
    except ValueError:
        pass
    urls = [resolver_urls] if isinstance(resolver_urls, str) else list(resolver_urls)
    result: list[str] = []
    for resolver_url in urls:
        for record_type in (1, 28):
            for value in resolve_probe_records(host, resolver_url, record_type):
                if value not in result:
                    result.append(value)
        if result:
            break
    if not result:
        result = system_probe_addresses(host)
    return result[:PROBE_MAX_ADDRESSES]


class UnixHTTPConnection(http.client.HTTPConnection):
    """Minimal HTTP client for a private Mihomo controller Unix socket."""

    def __init__(self, socket_path: Path, timeout: float):
        super().__init__("localhost", timeout=timeout)
        self.socket_path = socket_path

    def connect(self) -> None:
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.settimeout(self.timeout)
        self.sock.connect(str(self.socket_path))


def controller_request(
    socket_path: Path, method: str, path: str, timeout: float
) -> tuple[int, Any]:
    if method not in {"GET", "PUT"} or not path.startswith("/") or len(path) > 4096:
        raise BackendError("Invalid private Mihomo controller request")
    connection = UnixHTTPConnection(socket_path, timeout)
    try:
        connection.request(method, path, headers={"Accept": "application/json"})
        response = connection.getresponse()
        body = response.read(PROBE_RESULT_BYTES + 1)
        if len(body) > PROBE_RESULT_BYTES:
            raise BackendError("Mihomo controller response is too large")
        try:
            payload = json.loads(body) if body else {}
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise BackendError("Mihomo controller returned invalid JSON") from exc
        return response.status, payload
    finally:
        connection.close()


def controller_json(socket_path: Path, path: str, timeout: float) -> tuple[int, Any]:
    return controller_request(socket_path, "GET", path, timeout)


def probe_proxy_yaml(profile: dict[str, Any], alias: str, address: str) -> str:
    """Render one opaque, IP-pinned proxy for the isolated probe core."""
    return proxy_yaml({"name": alias, "uri": profile["uri"]}, server_override=address)


def probe_core_config(targets: list[dict[str, Any]]) -> str:
    proxies = "\n".join(
        probe_proxy_yaml(target["profile"], target["alias"], target["address"])
        for target in targets
    )
    group_members = "\n".join(f"    - {yaml_value(target['alias'])}" for target in targets)
    return f"""log-level: warning
ipv6: true
unified-delay: true
tcp-concurrent: true
find-process-mode: off
routing-mark: {PROBE_ROUTING_MARK}
external-controller-unix: controller.sock
proxies:
{proxies}
proxy-groups:
  - name: {yaml_value(PROBE_GROUP_NAME)}
    type: select
    proxies:
{group_members}
rules:
  - MATCH,DIRECT
"""


def stop_probe_core(process: subprocess.Popen[Any]) -> None:
    if process.poll() is not None:
        return
    process.terminate()
    try:
        process.wait(timeout=PROBE_CORE_STOP_SECONDS)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=PROBE_CORE_STOP_SECONDS)


def wait_probe_controller(process: subprocess.Popen[Any], socket_path: Path) -> None:
    deadline = time.monotonic() + PROBE_CORE_START_SECONDS
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        if process.poll() is not None:
            break
        if socket_path.exists():
            try:
                status_code, _payload = controller_json(socket_path, "/version", 0.5)
                if status_code == 200:
                    return
            except (BackendError, OSError, TimeoutError) as exc:
                last_error = exc
        time.sleep(0.05)
    if last_error:
        raise BackendError(f"Temporary Mihomo probe failed to start: {last_error}")
    exit_detail = f" (exit {process.returncode})" if process.returncode is not None else ""
    raise BackendError(f"Temporary Mihomo probe failed to start{exit_detail}")


def collect_probe_response(samples: dict[str, list[int]], status_code: int,
                           payload: Any) -> bool:
    """Merge one group-delay response; timeouts are valid empty results."""
    if not isinstance(payload, dict):
        return False
    if status_code == 504 and payload.get("message") == "get delay: all proxies timeout":
        return True
    if status_code != 200:
        return False
    for alias, delay in payload.items():
        if alias in samples and isinstance(delay, int) and 0 < delay <= 60_000:
            samples[alias].append(delay)
    return True


def run_mihomo_probe(
        paths: Paths,
        targets: list[dict[str, Any]],
        on_update: Callable[[dict[str, list[int]]], None] | None = None,
) -> dict[str, list[int]]:
    """Run full VLESS handshakes and HTTP checks in an isolated no-TUN core."""
    if not targets:
        return {}
    core = find_core(paths)
    samples: dict[str, list[int]] = {str(target["alias"]): [] for target in targets}
    process: subprocess.Popen[Any] | None = None
    previous_handlers = {
        number: signal.getsignal(number) for number in (signal.SIGINT, signal.SIGTERM)
    }

    def stop_for_signal(number: int, _frame: Any) -> None:
        if process is not None:
            with contextlib.suppress(Exception):
                stop_probe_core(process)
        raise SystemExit(128 + number)

    for number in previous_handlers:
        signal.signal(number, stop_for_signal)
    try:
        with tempfile.TemporaryDirectory(prefix="omavless-probe-") as temporary:
            root = Path(temporary)
            root.chmod(0o700)
            config_path = root / "config.yaml"
            log_path = root / "mihomo.log"
            socket_path = root / "controller.sock"
            atomic_write(config_path, probe_core_config(targets), 0o600)
            with log_path.open("w", encoding="utf-8") as log:
                process = subprocess.Popen(
                    [str(core), "-d", str(root), "-f", str(config_path)],
                    stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT,
                    start_new_session=True,
                )
                wait_probe_controller(process, socket_path)
                group = urllib.parse.quote(PROBE_GROUP_NAME, safe="")
                completed_requests = 0
                for url in PROBE_URLS:
                    query = urllib.parse.urlencode({
                        "url": url,
                        "timeout": PROBE_HTTP_TIMEOUT_MILLISECONDS,
                        "expected": "200-299",
                    })
                    status_code, payload = controller_json(
                        socket_path,
                        f"/group/{group}/delay?{query}",
                        PROBE_HTTP_TIMEOUT_MILLISECONDS / 1000 + 5,
                    )
                    if collect_probe_response(samples, status_code, payload):
                        completed_requests += 1
                        # The controller tests every member concurrently. Let
                        # an interactive caller publish the members that have
                        # answered this round instead of holding all visible
                        # latency numbers until every URL and timeout ends.
                        if on_update is not None:
                            on_update(samples)
                if completed_requests == 0:
                    raise BackendError("Mihomo could not complete the end-to-end probe")
    finally:
        if process is not None:
            with contextlib.suppress(Exception):
                stop_probe_core(process)
        for number, handler in previous_handlers.items():
            signal.signal(number, handler)
    return samples


def probe_subscription(paths: Paths, subscription_id: str) -> dict[str, Any]:
    """Probe managed endpoints end-to-end and return credential-free data."""
    store = load_store(paths)
    subscription = subscription_by_id(store, subscription_id)
    profiles = [
        profile for profile in store["profiles"]
        if profile.get("subscriptionId") == subscription["id"]
        and not profile.get("missing", False)
    ]
    resolver_urls = configured_working_probe_resolvers(paths)

    resolved_ids: set[str] = set()
    targets: list[dict[str, Any]] = []
    for profile_index, profile in enumerate(profiles):
        node = parse_vless(str(profile["uri"]))
        addresses = resolve_probe_addresses(str(node["server"]), resolver_urls)
        if not addresses:
            continue
        profile_id = str(profile["id"])
        resolved_ids.add(profile_id)
        for address_index, address in enumerate(addresses):
            targets.append({
                "alias": f"p{profile_index:04d}a{address_index}",
                "profileId": profile_id,
                "address": address,
                "profile": profile,
            })

    alias_samples = run_mihomo_probe(paths, targets)
    profile_samples: dict[str, list[int]] = {profile_id: [] for profile_id in resolved_ids}
    for target in targets:
        profile_samples[str(target["profileId"])].extend(
            alias_samples.get(str(target["alias"]), [])
        )
    results = []
    for profile in profiles:
        profile_id = str(profile["id"])
        latencies = profile_samples.get(profile_id, [])
        latency = round(statistics.median(latencies)) if latencies else None
        results.append({
            "id": profile_id,
            "resolved": profile_id in resolved_ids,
            "reachable": latency is not None,
            "latencyMs": latency if latency is not None else -1,
        })
    return {
        "version": 1,
        "subscriptionId": str(subscription["id"]),
        "results": results,
    }


def emit_probe_event(payload: dict[str, Any]) -> None:
    """Write one credential-free NDJSON event for the Quickshell widget."""
    print(json.dumps(payload, separators=(",", ":")), flush=True)


def probe_subscription_stream(paths: Paths, subscription_id: str) -> None:
    """Probe a subscription and publish each usable result as it arrives."""
    store = load_store(paths)
    subscription = subscription_by_id(store, subscription_id)
    profiles = [
        profile for profile in store["profiles"]
        if profile.get("subscriptionId") == subscription["id"]
        and not profile.get("missing", False)
    ]
    subscription_id = str(subscription["id"])
    emit_probe_event({
        "version": 1,
        "type": "start",
        "subscriptionId": subscription_id,
        "total": len(profiles),
    })

    resolver_urls = configured_working_probe_resolvers(paths)
    resolved_ids: set[str] = set()
    targets: list[dict[str, Any]] = []
    emitted: dict[str, tuple[bool, bool, int]] = {}

    def publish_result(profile_id: str, resolved: bool,
                       reachable: bool, latency: int) -> None:
        value = (resolved, reachable, latency)
        if emitted.get(profile_id) == value:
            return
        emitted[profile_id] = value
        emit_probe_event({
            "version": 1,
            "type": "result",
            "subscriptionId": subscription_id,
            "id": profile_id,
            "resolved": resolved,
            "reachable": reachable,
            "latencyMs": latency,
        })

    for profile_index, profile in enumerate(profiles):
        node = parse_vless(str(profile["uri"]))
        addresses = resolve_probe_addresses(str(node["server"]), resolver_urls)
        profile_id = str(profile["id"])
        if not addresses:
            # DNS failures are already final and can be shown immediately.
            publish_result(profile_id, False, False, -1)
            continue
        resolved_ids.add(profile_id)
        for address_index, address in enumerate(addresses):
            targets.append({
                "alias": f"p{profile_index:04d}a{address_index}",
                "profileId": profile_id,
                "address": address,
                "profile": profile,
            })

    def profile_samples(alias_samples: dict[str, list[int]]) -> dict[str, list[int]]:
        result: dict[str, list[int]] = {profile_id: [] for profile_id in resolved_ids}
        for target in targets:
            result[str(target["profileId"])].extend(
                alias_samples.get(str(target["alias"]), [])
            )
        return result

    def publish_reachable(alias_samples: dict[str, list[int]]) -> None:
        # A server may get a second, more stable median after the next test
        # URL. Publishing that update is intentional: the first answer is
        # immediate, while the final number still uses every successful run.
        for profile_id, latencies in profile_samples(alias_samples).items():
            if latencies:
                publish_result(
                    profile_id, True, True, round(statistics.median(latencies))
                )

    alias_samples = run_mihomo_probe(paths, targets, publish_reachable)
    final_samples = profile_samples(alias_samples)
    unavailable = 0
    unresolved = 0
    for profile in profiles:
        profile_id = str(profile["id"])
        latencies = final_samples.get(profile_id, [])
        if profile_id not in resolved_ids:
            unresolved += 1
            publish_result(profile_id, False, False, -1)
        elif not latencies:
            unavailable += 1
            publish_result(profile_id, True, False, -1)
        else:
            publish_result(
                profile_id, True, True, round(statistics.median(latencies))
            )

    emit_probe_event({
        "version": 1,
        "type": "complete",
        "subscriptionId": subscription_id,
        "tested": len(profiles),
        "unavailable": unavailable,
        "unresolved": unresolved,
    })


def refresh_all_subscriptions(paths: Paths) -> dict[str, int]:
    store = load_store(paths)
    downloaded: list[tuple[str, list[dict[str, Any]], int]] = []
    for subscription in store["subscriptions"]:
        entries, skipped = parse_subscription(fetch_subscription(str(subscription["url"])))
        downloaded.append((str(subscription["id"]), entries, skipped))
    total = {"added": 0, "removed": 0, "stale": 0, "total": 0, "skipped": 0}
    updated_at = time.time_ns() // 1_000_000
    for subscription_id, entries, skipped in downloaded:
        result = sync_subscription_store(
            store, subscription_by_id(store, subscription_id), entries, updated_at
        )
        for key in ("added", "removed", "stale", "total"):
            total[key] += result[key]
        total["skipped"] += skipped
    save_store(paths, store)
    return total


def delete_subscription(paths: Paths, subscription_id: str) -> None:
    store = load_store(paths)
    subscription_by_id(store, subscription_id)
    managed_ids = {
        str(profile["id"]) for profile in store["profiles"]
        if profile.get("subscriptionId") == subscription_id
    }
    if store.get("activeId") in managed_ids and service_active(SERVICE):
        raise BackendError("Disconnect the active subscribed profile before removing its subscription")
    store["profiles"] = [
        profile for profile in store["profiles"]
        if profile.get("subscriptionId") != subscription_id
    ]
    store["subscriptions"] = [
        item for item in store["subscriptions"] if item.get("id") != subscription_id
    ]
    if store.get("lastId") in managed_ids:
        store["lastId"] = store["profiles"][0]["id"] if store["profiles"] else ""
    save_store(paths, store)


def first_query(query: dict[str, list[str]], *names: str, default: str = "") -> str:
    lowered = {key.lower(): values for key, values in query.items()}
    for name in names:
        values = lowered.get(name.lower())
        if values:
            return values[0]
    return default


def query_bool(query: dict[str, list[str]], *names: str) -> bool:
    return first_query(query, *names).lower() in {"1", "true", "yes", "on"}


def validate_reality_public_key(value: str) -> None:
    """Match Mihomo's canonical raw URL-safe 32-byte X25519 key format."""
    if not re.fullmatch(r"[A-Za-z0-9_-]{43}", value):
        raise BackendError("VLESS Reality public key has an invalid format")
    try:
        decoded = base64.b64decode(value + "=", altchars=b"-_", validate=True)
    except binascii.Error as exc:
        raise BackendError("VLESS Reality public key has an invalid format") from exc
    canonical = base64.urlsafe_b64encode(decoded).decode("ascii").rstrip("=")
    if len(decoded) != 32 or canonical != value:
        raise BackendError("VLESS Reality public key has an invalid format")


def validate_reality_short_id(value: str) -> None:
    """Mihomo accepts an optional, even-length hexadecimal short ID up to 8 bytes."""
    if len(value) > 16 or len(value) % 2 or not re.fullmatch(r"[0-9A-Fa-f]*", value):
        raise BackendError("VLESS Reality short ID has an invalid format")


def extract_vless_uri(text: str) -> str:
    for token in re.split(r"\s+", text.strip()):
        if token.lower().startswith("vless://"):
            uri = token.strip()
            if len(uri.encode("utf-8")) > MAX_VLESS_URI_BYTES:
                raise BackendError(
                    f"VLESS link is too large (maximum {MAX_VLESS_URI_BYTES // 1024} KiB)"
                )
            return uri
    raise BackendError("Input does not contain a VLESS link")


def parse_vless(uri: str) -> dict[str, Any]:
    uri = extract_vless_uri(uri)
    try:
        parsed = urllib.parse.urlsplit(uri)
        port = parsed.port
    except ValueError as exc:
        raise BackendError(f"Invalid VLESS link: {exc}") from exc
    user = urllib.parse.unquote(parsed.username or "")
    if parsed.scheme.lower() != "vless":
        raise BackendError("Only vless:// links are supported")
    if parsed.password is not None:
        raise BackendError("VLESS link must not contain a password field")
    try:
        uuidlib.UUID(user)
    except ValueError as exc:
        raise BackendError("VLESS user id is not a valid UUID") from exc
    if not parsed.hostname or not port or not 1 <= port <= 65535:
        raise BackendError("VLESS server and port are required")

    try:
        query = urllib.parse.parse_qs(parsed.query, keep_blank_values=True, max_num_fields=128)
    except ValueError as exc:
        raise BackendError(f"Invalid VLESS query: {exc}") from exc
    network = first_query(query, "type", "network", default="tcp").lower() or "tcp"
    network = {"raw": "tcp"}.get(network, network)
    if network not in {"tcp", "ws", "http", "h2", "grpc", "xhttp"}:
        raise BackendError(f"Unsupported VLESS transport: {network}")
    security = first_query(query, "security", default="none").lower() or "none"
    if security not in {"none", "tls", "reality"}:
        raise BackendError(f"Unsupported VLESS security: {security}")
    public_key = first_query(query, "pbk", "publickey", "public-key")
    server_name = first_query(query, "sni", "servername")
    if security == "reality" and (not public_key or not server_name):
        raise BackendError("Reality link requires both pbk/public-key and sni/servername")
    short_id = first_query(query, "sid", "short-id")
    spider_x = first_query(query, "spx", "spider-x")
    if security == "reality":
        validate_reality_public_key(public_key)
        validate_reality_short_id(short_id)
        if first_query(query, "mldsa65Verify", "mldsa65-verify"):
            raise BackendError(
                "VLESS Reality ML-DSA verification is not supported by Mihomo"
            )
    encryption = first_query(query, "encryption", default="none") or "none"
    if encryption != "none":
        raise BackendError("VLESS encryption must be none")
    flow = first_query(query, "flow").lower()
    if flow not in {"", "xtls-rprx-vision", "xtls-rprx-vision-udp443"}:
        raise BackendError(f"Unsupported VLESS flow: {flow}")
    if flow and network != "tcp":
        raise BackendError("VLESS Vision flow requires the TCP transport")
    if flow and security not in {"tls", "reality"}:
        raise BackendError("VLESS Vision flow requires TLS or Reality security")
    # Mihomo exposes one Vision flow whose UDP/443 behavior corresponds to
    # Xray's explicit `-udp443` variant. Preserve the source value for preview
    # and compatibility reporting, but emit the only value Mihomo accepts.
    mihomo_flow = "xtls-rprx-vision" if flow else ""
    packet_encoding = first_query(query, "packetEncoding", "packet-encoding").lower()
    if packet_encoding not in {"", "xudp", "packetaddr"}:
        raise BackendError(f"Unsupported VLESS packet encoding: {packet_encoding}")
    mode = first_query(query, "mode")
    if network == "xhttp":
        mode = mode.lower()
        if mode not in {"", "auto", "stream-one", "stream-up", "packet-up"}:
            raise BackendError(f"Unsupported VLESS XHTTP mode: {mode}")
    else:
        # Other transports use their own option names. Some providers retain
        # an unrelated `mode` query field, which must not alter generated YAML.
        mode = ""
    header_type = first_query(query, "headerType", "header-type").lower()
    if network == "tcp" and header_type not in {"", "none"}:
        raise BackendError(f"Unsupported VLESS TCP header type: {header_type}")
    return {
        "uri": uri, "uuid": user, "server": parsed.hostname, "port": port,
        "network": network, "security": security,
        "encryption": encryption,
        "flow": flow, "mihomo_flow": mihomo_flow, "servername": server_name,
        "fingerprint": first_query(query, "fp", "fingerprint", "client-fingerprint"),
        "public_key": public_key, "short_id": short_id,
        # Xray share links commonly carry spx. Current Mihomo has no matching
        # proxy option, so keep only enough private parser state to report the
        # compatibility boundary; never copy it into generated configuration.
        "spider_x": spider_x,
        "compatibility_note": (
            REALITY_SPX_COMPATIBILITY_NOTE
            if security == "reality" and bool(spider_x) else ""
        ),
        "path": urllib.parse.unquote(first_query(query, "path", default="/")) or "/",
        "host": first_query(query, "host"),
        "service_name": first_query(query, "serviceName", "service-name"),
        "mode": mode,
        "alpn": [part.strip() for part in first_query(query, "alpn").split(",") if part.strip()],
        "allow_insecure": query_bool(query, "allowInsecure", "skip-cert-verify"),
        "packet_encoding": packet_encoding,
        "suggested_name": urllib.parse.unquote(parsed.fragment or "").strip(),
    }


def preview_vless(text: str) -> dict[str, Any]:
    """Return useful import facts without returning reusable credentials."""
    node = parse_vless(text)
    suggested = re.sub(r"[\x00-\x1f\x7f]", "", str(node["suggested_name"])).strip()[:80]
    return {
        "version": 1,
        "server": str(node["server"])[:253],
        "port": int(node["port"]),
        "transport": str(node["network"]),
        "security": str(node["security"]),
        "sni": str(node["servername"])[:253],
        "flow": str(node["flow"])[:64],
        "insecure": bool(node["allow_insecure"]),
        "compatibilityNote": str(node["compatibility_note"]),
        # Four trailing characters help compare two keys without making this
        # preview a copyable credential source.
        "credentialHint": "••••" + str(node["uuid"])[-4:],
        "suggestedName": suggested,
    }


def yaml_value(value: Any) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    return json.dumps(str(value), ensure_ascii=False)


def proxy_yaml(profile: dict[str, Any], server_override: str | None = None) -> str:
    node = parse_vless(str(profile["uri"]))
    y = yaml_value
    lines = [f"- name: {y(profile['name'])}", "  type: vless",
             f"  server: {y(server_override or node['server'])}", f"  port: {node['port']}",
             f"  uuid: {y(node['uuid'])}", "  udp: true",
             f"  network: {node['network']}", f"  encryption: {y(node['encryption'])}"]
    if node["mihomo_flow"]:
        lines.append(f"  flow: {y(node['mihomo_flow'])}")
    if node["packet_encoding"]:
        lines.append(f"  packet-encoding: {y(node['packet_encoding'])}")
    if node["security"] in {"tls", "reality"}:
        lines.append("  tls: true")
        if node["servername"]:
            lines.append(f"  servername: {y(node['servername'])}")
        if node["fingerprint"]:
            lines.append(f"  client-fingerprint: {y(node['fingerprint'])}")
        if node["alpn"]:
            lines.append(f"  alpn: {json.dumps(node['alpn'], ensure_ascii=False)}")
        if node["allow_insecure"]:
            lines.append("  skip-cert-verify: true")
    if node["security"] == "reality":
        lines.extend(["  reality-opts:", f"    public-key: {y(node['public_key'])}"])
        if node["short_id"]:
            lines.append(f"    short-id: {y(node['short_id'])}")
    if node["network"] == "ws":
        lines.extend(["  ws-opts:", f"    path: {y(node['path'])}"])
        if node["host"]:
            lines.extend(["    headers:", f"      Host: {y(node['host'])}"])
    elif node["network"] == "grpc":
        lines.extend(["  grpc-opts:", f"    grpc-service-name: {y(node['service_name'])}"])
    elif node["network"] == "h2":
        lines.append("  h2-opts:")
        if node["host"]:
            lines.append(f"    host: [{y(node['host'])}]")
        lines.append(f"    path: {y(node['path'])}")
    elif node["network"] == "http":
        lines.extend(["  http-opts:", f"    path: [{y(node['path'])}]"])
        if node["host"]:
            lines.extend(["    headers:", f"      Host: [{y(node['host'])}]"])
    elif node["network"] == "xhttp":
        lines.extend(["  xhttp-opts:", f"    path: {y(node['path'])}"])
        if node["host"]:
            lines.append(f"    host: {y(node['host'])}")
        if node["mode"]:
            lines.append(f"    mode: {y(node['mode'])}")
    return "\n".join(lines)


def template_from_config(text: str) -> str:
    lines = text.splitlines(keepends=True)
    start = next((i for i, line in enumerate(lines) if re.match(r"^proxies\s*:", line)), -1)
    replacement = ["proxies:\n", PROFILE_MARKER + "\n"]
    if start < 0:
        insert = next((i for i, line in enumerate(lines) if re.match(r"^rules\s*:", line)), len(lines))
        lines[insert:insert] = replacement
        return "".join(lines)
    end = len(lines)
    for index in range(start + 1, len(lines)):
        line = lines[index]
        if line.strip() and not line.startswith((" ", "\t", "#")) and re.match(r"^[A-Za-z0-9_-]+\s*:", line):
            end = index
            break
    lines[start:end] = replacement
    return "".join(lines)


def ensure_template(paths: Paths) -> str:
    if paths.template.is_symlink():
        raise BackendError(f"Refusing symlinked route template: {paths.template}")
    if paths.template.exists():
        text = read_text_file(paths.template, MAX_TEMPLATE_BYTES, "route template", private=True)
    else:
        # The default must be reproducible and independent of other clients.
        # Users can explicitly import any compatible routing YAML through
        # `adopt-template`, but OmaVLESS never discovers one on its own.
        text = (PLUGIN_DIR / "templates" / "default.yaml").read_text(encoding="utf-8")
        atomic_write(paths.template, text)
    if text.count(PROFILE_MARKER) != 1:
        raise BackendError(f"Route template must contain exactly one {PROFILE_MARKER}")
    return text


def controller_socket(paths: Paths) -> Path:
    return paths.runtime / f"omavless.{os.getuid()}.controller.sock"


def strip_controller_config(text: str) -> str:
    """Remove inherited API listeners before adding the private Unix socket."""
    lines = text.splitlines(keepends=True)
    kept: list[str] = []
    index = 0
    while index < len(lines):
        line = lines[index]
        match = re.match(r"^(external-controller(?:-[a-z0-9-]+)?|secret)\s*:", line)
        if not match:
            kept.append(line)
            index += 1
            continue
        index += 1
        while index < len(lines):
            following = lines[index]
            if following.strip() and not following.startswith((" ", "\t", "#")):
                break
            index += 1
    return "".join(kept)


def custom_rule_line(rule: dict[str, Any]) -> str:
    targets = {"proxy": "PROXY", "direct": "DIRECT", "reject": "REJECT-DROP"}
    kind = str(rule["kind"])
    value = str(rule["value"])
    target = targets[str(rule["action"])]
    if kind == "domain":
        return f"  - DOMAIN,{value},{target}\n"
    if kind == "suffix":
        return f"  - DOMAIN-SUFFIX,{value},{target}\n"
    network = ipaddress.ip_network(value, strict=False)
    rule_type = "IP-CIDR6" if network.version == 6 else "IP-CIDR"
    return f"  - {rule_type},{network},{target},no-resolve\n"


def inject_custom_rules(text: str, rules: list[dict[str, Any]]) -> str:
    if not rules:
        return text
    lines = text.splitlines(keepends=True)
    matches = [
        index for index, line in enumerate(lines)
        if re.match(r"^rules\s*:\s*(?:#.*)?$", line.rstrip("\r\n"))
    ]
    if len(matches) != 1:
        raise BackendError("Route template must contain one top-level rules section")
    insertion = ["  # OmaVLESS custom rules — evaluated before the selected preset\n"]
    insertion.extend(custom_rule_line(rule) for rule in rules)
    lines[matches[0] + 1:matches[0] + 1] = insertion
    return "".join(lines)


def private_runtime_config(
    paths: Paths, text: str, store: dict[str, Any]
) -> str:
    ensure_runtime(paths)
    configured = inject_custom_rules(strip_controller_config(text), store["customRules"])
    return f"external-controller-unix: {yaml_value(str(controller_socket(paths)))}\n" + configured


def prepare_controller_socket(paths: Paths) -> None:
    ensure_runtime(paths)
    socket_path = controller_socket(paths)
    with contextlib.suppress(FileNotFoundError):
        socket_path.unlink()


def render_config(
    paths: Paths, profile: dict[str, Any], store: dict[str, Any] | None = None
) -> str:
    current_store = store if store is not None else load_store(paths)
    text = ensure_template(paths).replace(PROFILE_MARKER, proxy_yaml(profile))
    return private_runtime_config(paths, text, current_store)


def render_config_mode(
    paths: Paths, profile: dict[str, Any], mode: str,
    store: dict[str, Any] | None = None,
) -> str:
    """Render one profile for an explicit login mode without changing Settings."""
    current_store = store if store is not None else load_store(paths)
    text = template_with_mode(ensure_template(paths), mode).replace(
        PROFILE_MARKER, proxy_yaml(profile)
    )
    return private_runtime_config(paths, text, current_store)


def bundled_template(name: str) -> str:
    source = ROUTING_PRESETS.get(name)
    if source is None:
        raise BackendError(f"Unknown bundled routing profile: {name}")
    text = read_text_file(source, MAX_TEMPLATE_BYTES, "bundled routing template")
    if text.count(PROFILE_MARKER) != 1:
        raise BackendError(f"Bundled route template must contain exactly one {PROFILE_MARKER}")
    return text


def use_bundled_template(paths: Paths, name: str, keep_mode: bool = False) -> None:
    """Adopt a bundled policy and reconnect transactionally when active.

    The first-run chooser activates Rule mode. Selecting a profile from the
    settings page instead preserves Full VPN, Routing or Direct, so editing a
    preference never changes the live routing mode as a side effect.
    """
    previous = ensure_template(paths)
    replacement = bundled_template(name)
    store = load_store(paths)
    previous_store = json.loads(json.dumps(store))
    if keep_mode:
        previous_mode = yaml_top_level_scalar(previous, "mode")
        if previous_mode in {"rule", "global", "direct"}:
            replacement = template_with_mode(replacement, previous_mode)
    store["routingPreset"] = name
    if replacement == previous and previous_store.get("routingPreset") == name:
        return
    running = service_active(SERVICE)
    active_id = str(store.get("activeId", "")) if running else ""
    if running and not active_id:
        raise BackendError("omavless.service is running without an active profile")
    template_changed = replacement != previous
    if template_changed:
        atomic_write(paths.template, replacement)
    try:
        save_store(paths, store)
        if active_id and template_changed:
            connect_profile(paths, active_id)
    except Exception as exc:
        rollback_errors: list[str] = []
        try:
            if template_changed:
                atomic_write(paths.template, previous)
        except Exception as rollback_exc:
            rollback_errors.append(f"template restore: {rollback_exc}")
        try:
            save_store(paths, previous_store)
        except Exception as rollback_exc:
            rollback_errors.append(f"preference restore: {rollback_exc}")
        if rollback_errors:
            raise BackendError(
                f"{exc}; routing preset rollback needs manual recovery ({'; '.join(rollback_errors)})",
                21,
            ) from exc
        raise


def template_with_mode(text: str, mode: str) -> str:
    """Return a routing template with one explicit Mihomo routing mode."""
    if mode not in {"rule", "global", "direct"}:
        raise BackendError(f"Unsupported routing mode: {mode}")
    pattern = re.compile(r"(?m)^(mode\s*:\s*)([^#\r\n]*?)(\s*(?:#.*)?)$")
    matches = list(pattern.finditer(text))
    if len(matches) > 1:
        raise BackendError("Route template contains more than one top-level mode")
    if matches:
        return pattern.sub(lambda match: match.group(1) + mode + match.group(3), text, count=1)
    return f"mode: {mode}\n" + text


def set_routing_mode(paths: Paths, mode: str) -> None:
    """Persist a mode and reconnect the active profile transactionally."""
    previous = ensure_template(paths)
    replacement = template_with_mode(previous, mode)
    if replacement == previous:
        return
    store = load_store(paths)
    running = service_active(SERVICE)
    active_id = str(store.get("activeId", "")) if running else ""
    if running and not active_id:
        raise BackendError("omavless.service is running without an active profile")
    atomic_write(paths.template, replacement)
    try:
        if active_id:
            connect_profile(paths, active_id)
    except Exception as exc:
        try:
            atomic_write(paths.template, previous)
        except Exception as rollback_exc:
            raise BackendError(
                f"{exc}; routing mode rollback needs manual recovery ({rollback_exc})", 21
            ) from exc
        raise


def yaml_top_level_block(text: str, key: str) -> list[str]:
    """Return one top-level YAML block without depending on PyYAML.

    OmaVLESS only needs a small, non-secret status summary here. The complete
    config is still parsed and validated by Mihomo before it is activated.
    """
    lines = text.splitlines()
    start = next((i for i, line in enumerate(lines)
                  if re.match(rf"^{re.escape(key)}\s*:\s*(?:#.*)?$", line)), -1)
    if start < 0:
        return []
    end = len(lines)
    for index in range(start + 1, len(lines)):
        line = lines[index]
        if line.strip() and not line.startswith((" ", "\t", "#")):
            end = index
            break
    return lines[start + 1:end]


def yaml_top_level_scalar(text: str, key: str) -> str:
    match = re.search(
        rf"(?m)^{re.escape(key)}\s*:\s*([^#\r\n]+?)\s*(?:#.*)?$", text
    )
    if not match:
        return ""
    return match.group(1).strip().strip("\"'").lower()


def yaml_sequence_count(lines: list[str]) -> int:
    return sum(1 for line in lines if re.match(r"^[ \t]+-\s+\S", line))


def yaml_mapping_count(lines: list[str]) -> int:
    candidates: list[tuple[int, str]] = []
    for line in lines:
        if not line.strip() or line.lstrip().startswith(("#", "-")):
            continue
        match = re.match(r"^([ \t]+)([^:#][^:]*):(?:\s|$)", line)
        if match:
            candidates.append((len(match.group(1).expandtabs(2)), match.group(2)))
    if not candidates:
        return 0
    outer_indent = min(indent for indent, _ in candidates)
    return sum(1 for indent, _ in candidates if indent == outer_indent)


def routing_status(paths: Paths, running: bool, store: dict[str, Any] | None = None) -> dict[str, Any]:
    """Describe the policy in the config that is (or will be) used.

    This deliberately exposes no server, profile or provider URL. It gives the
    panel enough structured data to distinguish Rule mode from a useful rule
    set, which are not the same thing.
    """
    try:
        if running and paths.config.exists():
            text = read_text_file(
                paths.config, MAX_TEMPLATE_BYTES, "routing configuration", private=True
            )
        else:
            text = ensure_template(paths)
    except BackendError:
        return {"mode": "unknown", "source": "unknown", "preset": "",
                "configured": False, "ruleCount": 0, "providerCount": 0,
                "customRuleCount": 0, "rulesUpdatedAt": 0,
                "ruleUpdateAvailable": False}

    mode = yaml_top_level_scalar(text, "mode") or "unknown"
    rules = yaml_top_level_block(text, "rules")
    providers = yaml_top_level_block(text, "rule-providers")
    rule_count = yaml_sequence_count(rules)
    provider_count = yaml_mapping_count(providers)
    lowered = text.lower()
    marker = re.search(r"(?m)^#\s*omavless-routing-profile\s*:\s*([^\s#]+)", lowered)
    marked = marker.group(1) if marker else ""

    normalized_rules = {
        re.sub(r"\s+", "", line).upper()
        for line in rules if re.match(r"^[ \t]+-\s+", line)
    }
    basic_rules = {
        "-IP-CIDR,127.0.0.0/8,DIRECT,NO-RESOLVE",
        "-IP-CIDR,10.0.0.0/8,DIRECT,NO-RESOLVE",
        "-IP-CIDR,172.16.0.0/12,DIRECT,NO-RESOLVE",
        "-IP-CIDR,192.168.0.0/16,DIRECT,NO-RESOLVE",
        "-MATCH,PROXY",
    }

    if marked == "roscomvpn-default" or (
        provider_count > 0 and "roscomvpn-geo" in lowered
    ):
        source = "roscomvpn"
    elif marked == "china-cn-direct":
        source = "china"
    elif marked == "iran-ir-direct":
        source = "iran"
    elif rule_count == 0:
        source = "none"
    elif provider_count == 0 and normalized_rules == basic_rules:
        source = "basic"
    else:
        source = "custom"
    current_store = store if store is not None else load_store(paths)
    selected = str(current_store.get("routingPreset", ""))
    effective_preset = selected if selected == "custom" or selected == marked else marked
    return {"mode": mode, "source": source, "preset": effective_preset,
            "configured": bool(selected), "ruleCount": rule_count,
            "providerCount": provider_count,
            "customRuleCount": len(current_store.get("customRules", [])),
            "rulesUpdatedAt": int(current_store.get("rulesUpdatedAt", 0)),
            "ruleUpdateAvailable": bool(
                running and mode == "rule" and provider_count > 0
                and controller_socket(paths).exists()
            )}


def save_custom_rule(paths: Paths, kind: str, action: str, raw_value: str) -> dict[str, Any]:
    if kind not in {"domain", "suffix", "ipcidr"}:
        raise BackendError("Unsupported custom routing match type")
    if action not in {"proxy", "direct", "reject"}:
        raise BackendError("Unsupported custom routing action")
    value = canonical_custom_rule_value(kind, raw_value)
    store = load_store(paths)
    if len(store["customRules"]) >= MAX_CUSTOM_RULE_COUNT:
        raise BackendError(f"At most {MAX_CUSTOM_RULE_COUNT} custom routing rules are supported")
    if any(rule["kind"] == kind and rule["value"] == value for rule in store["customRules"]):
        raise BackendError("That custom routing match already exists")
    previous_store = json.loads(json.dumps(store))
    rule = {"id": str(uuidlib.uuid4()), "kind": kind, "value": value, "action": action}
    store["customRules"].append(rule)
    save_store(paths, store)
    running = service_active(SERVICE)
    active_id = str(store.get("activeId", "")) if running else ""
    if running and not active_id:
        save_store(paths, previous_store)
        raise BackendError("omavless.service is running without an active profile")
    try:
        if active_id:
            connect_profile(paths, active_id)
    except Exception as exc:
        try:
            save_store(paths, previous_store)
        except Exception as rollback_exc:
            raise BackendError(
                f"{exc}; custom rule rollback needs manual recovery ({rollback_exc})", 21
            ) from exc
        raise
    return rule


def delete_custom_rule(paths: Paths, rule_id: str) -> None:
    store = load_store(paths)
    previous_store = json.loads(json.dumps(store))
    remaining = [rule for rule in store["customRules"] if rule["id"] != rule_id]
    if len(remaining) == len(store["customRules"]):
        raise BackendError("No such custom routing rule")
    store["customRules"] = remaining
    save_store(paths, store)
    running = service_active(SERVICE)
    active_id = str(store.get("activeId", "")) if running else ""
    if running and not active_id:
        save_store(paths, previous_store)
        raise BackendError("omavless.service is running without an active profile")
    try:
        if active_id:
            connect_profile(paths, active_id)
    except Exception as exc:
        try:
            save_store(paths, previous_store)
        except Exception as rollback_exc:
            raise BackendError(
                f"{exc}; custom rule rollback needs manual recovery ({rollback_exc})", 21
            ) from exc
        raise


def custom_rules_text(paths: Paths) -> str:
    store = load_store(paths)
    payload = {
        "version": 1,
        "rules": [
            {
                "id": str(rule["id"]), "kind": str(rule["kind"]),
                "value": str(rule["value"]), "action": str(rule["action"]),
            }
            for rule in store["customRules"]
        ],
    }
    return json.dumps(payload, ensure_ascii=False, separators=(",", ":")) + "\n"


def _refresh_rule_provider(socket_path: Path, name: str) -> str:
    endpoint = "/providers/rules/" + urllib.parse.quote(name, safe="")
    status_code, _payload = controller_request(socket_path, "PUT", endpoint, 60)
    if status_code != 204:
        raise BackendError(f"Mihomo refused rule provider update: {name}")
    return name


def refresh_rule_providers(paths: Paths) -> dict[str, int]:
    if not service_active(SERVICE):
        raise BackendError("Connect in Routing mode before refreshing rule data")
    socket_path = controller_socket(paths)
    if not socket_path.exists():
        raise BackendError("Reconnect once to enable private rule updates")
    status_code, payload = controller_json(socket_path, "/providers/rules", 5)
    providers = payload.get("providers") if isinstance(payload, dict) else None
    if status_code != 200 or not isinstance(providers, dict) or len(providers) > 256:
        raise BackendError("Mihomo returned invalid rule provider status")
    names: list[str] = []
    for name, provider in providers.items():
        if not isinstance(name, str) or not 0 < len(name) <= 256 or not isinstance(provider, dict):
            raise BackendError("Mihomo returned invalid rule provider metadata")
        vehicle = str(provider.get("vehicleType", "")).lower()
        if vehicle == "http":
            names.append(name)
    if not names:
        raise BackendError("The active Routing profile has no remote rule providers")
    failures: list[str] = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=min(4, len(names))) as executor:
        futures = {
            executor.submit(_refresh_rule_provider, socket_path, name): name for name in names
        }
        for future, name in futures.items():
            try:
                future.result()
            except Exception:
                failures.append(name)
    if failures:
        preview = ", ".join(failures[:3])
        if len(failures) > 3:
            preview += f" and {len(failures) - 3} more"
        raise BackendError(f"Could not refresh rule providers: {preview}")
    with operation_lock(paths):
        store = load_store(paths)
        store["rulesUpdatedAt"] = time.time_ns() // 1_000_000
        save_store(paths, store)
        updated_at = int(store["rulesUpdatedAt"])
        invalidate_status_cache(paths)
    return {"updated": len(names), "updatedAt": updated_at}


def canonical_route_query(value: str) -> tuple[str, bool]:
    text = value.strip()
    try:
        return str(ipaddress.ip_address(text)), True
    except ValueError:
        return canonical_domain(text), False


def custom_rule_match(
    rules: list[dict[str, Any]], query: str, is_ip: bool
) -> dict[str, Any] | None:
    for rule in rules:
        kind = str(rule["kind"])
        value = str(rule["value"])
        if kind == "ipcidr":
            if is_ip and ipaddress.ip_address(query) in ipaddress.ip_network(value):
                return rule
        elif not is_ip and (
            (kind == "domain" and query == value)
            or (kind == "suffix" and (query == value or query.endswith("." + value)))
        ):
            return rule
    return None


def route_outcome(target: str) -> str:
    value = target.upper()
    if value == "DIRECT":
        return "direct"
    if value.startswith("REJECT"):
        return "block"
    return "vpn"


def public_route_target(target: str) -> str:
    """Expose route semantics without leaking a profile or policy-group name."""
    outcome = route_outcome(target)
    if outcome == "direct":
        return "DIRECT"
    if outcome == "block":
        return "REJECT"
    return "PROXY"


def controller_rules(socket_path: Path, timeout: float) -> list[dict[str, Any]]:
    status_code, payload = controller_json(socket_path, "/rules", timeout)
    rules = payload.get("rules") if isinstance(payload, dict) else None
    if status_code != 200 or not isinstance(rules, list) or len(rules) > 65536:
        raise BackendError("Mihomo returned invalid routing rule status")
    if any(not isinstance(item, dict) for item in rules):
        raise BackendError("Mihomo returned invalid routing rule metadata")
    return rules


def rule_hit_snapshot(rules: list[dict[str, Any]]) -> dict[int, int]:
    snapshot: dict[int, int] = {}
    for position, item in enumerate(rules):
        extra = item.get("extra")
        hit_count = extra.get("hitCount") if isinstance(extra, dict) else None
        index = item.get("index", position)
        if (
            isinstance(index, int) and not isinstance(index, bool)
            and isinstance(hit_count, int) and not isinstance(hit_count, bool)
            and index >= 0 and hit_count >= 0
        ):
            snapshot[index] = hit_count
    return snapshot


def incremented_rule(
    rules: list[dict[str, Any]], before_hits: dict[int, int]
) -> dict[str, Any] | None:
    candidates: list[tuple[int, int, dict[str, Any]]] = []
    for position, item in enumerate(rules):
        extra = item.get("extra")
        hit_count = extra.get("hitCount") if isinstance(extra, dict) else None
        hit_at = extra.get("hitAt", 0) if isinstance(extra, dict) else 0
        index = item.get("index", position)
        if (
            isinstance(index, int) and not isinstance(index, bool)
            and isinstance(hit_count, int) and not isinstance(hit_count, bool)
            and hit_count > before_hits.get(index, hit_count)
        ):
            candidates.append((int(hit_at) if isinstance(hit_at, int) else 0, index, item))
    if not candidates:
        return None
    # Other traffic may hit a rule during the short probe. Mihomo's hitAt value
    # lets the most recent match win; the active-connection match below remains
    # the preferred result when it is observable.
    candidates.sort(key=lambda candidate: (candidate[0], candidate[1]), reverse=True)
    return candidates[0][2]


def rule_target(
    rules: list[dict[str, Any]], rule_type: str, rule_payload: str
) -> str:
    normalized_type = re.sub(r"[^A-Z0-9]", "", rule_type.upper())
    for item in rules:
        item_type = re.sub(r"[^A-Z0-9]", "", str(item.get("type", "")).upper())
        if item_type == normalized_type and str(item.get("payload", "")) == rule_payload:
            return str(item.get("proxy", ""))[:256]
    return ""


def live_route_match(paths: Paths, query: str, is_ip: bool) -> dict[str, str]:
    socket_path = controller_socket(paths)
    if not socket_path.exists():
        raise BackendError("Reconnect once to enable private domain checks")
    before_rules = controller_rules(socket_path, 3)
    before_hits = rule_hit_snapshot(before_rules)
    status_code, before_payload = controller_json(socket_path, "/connections", 3)
    before_connections = before_payload.get("connections") if isinstance(before_payload, dict) else None
    if status_code != 200 or not isinstance(before_connections, list):
        raise BackendError("Mihomo returned invalid connection status")
    before_ids = {
        str(item.get("id", "")) for item in before_connections if isinstance(item, dict)
    }
    config_text = read_text_file(paths.config, MAX_TEMPLATE_BYTES, "generated config", private=True)
    try:
        port = int(yaml_top_level_scalar(config_text, "mixed-port"))
    except ValueError as exc:
        raise BackendError("The active config has no usable local mixed port") from exc
    if not 1 <= port <= 65535:
        raise BackendError("The active config has no usable local mixed port")
    authority = f"[{query}]:443" if is_ip and ":" in query else f"{query}:443"
    probe = socket.create_connection(("127.0.0.1", port), timeout=5)
    try:
        request = f"CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n\r\n"
        probe.sendall(request.encode("ascii"))
        deadline = time.monotonic() + 5
        matched_connection: dict[str, Any] | None = None
        matched_rule: dict[str, Any] | None = None
        current_rules = before_rules
        while time.monotonic() < deadline:
            _code, current = controller_json(socket_path, "/connections", 1)
            connections = current.get("connections") if isinstance(current, dict) else None
            if isinstance(connections, list):
                for item in connections:
                    if not isinstance(item, dict) or str(item.get("id", "")) in before_ids:
                        continue
                    metadata = item.get("metadata")
                    host = str(metadata.get("host", "")).lower() if isinstance(metadata, dict) else ""
                    destination = str(metadata.get("destinationIP", "")) if isinstance(metadata, dict) else ""
                    if host == query or (is_ip and destination == query):
                        matched_connection = item
                        break
            current_rules = controller_rules(socket_path, 1)
            matched_rule = incremented_rule(current_rules, before_hits)
            if matched_connection is not None or matched_rule is not None:
                break
            time.sleep(0.1)
    finally:
        probe.close()
    if matched_connection is None and matched_rule is None:
        raise BackendError("Could not observe a route for that destination; it may be unavailable")
    rule_type = ""
    rule_payload = ""
    target = ""
    if matched_connection is not None:
        rule_type = str(matched_connection.get("rule", ""))[:80]
        rule_payload = str(matched_connection.get("rulePayload", ""))[:256]
        target = rule_target(current_rules, rule_type, rule_payload)
    elif matched_rule is not None:
        rule_type = str(matched_rule.get("type", ""))[:80]
        rule_payload = str(matched_rule.get("payload", ""))[:256]
        target = str(matched_rule.get("proxy", ""))[:256]
    if not target:
        chains = matched_connection.get("chains") if matched_connection is not None else None
        if isinstance(chains, list) and chains:
            upper = [str(item).upper() for item in chains]
            target = "DIRECT" if "DIRECT" in upper else "PROXY"
    return {
        "outcome": route_outcome(target or "PROXY"),
        "ruleType": rule_type or "RULE",
        "rulePayload": rule_payload,
        "target": public_route_target(target or "PROXY"),
        "source": "live",
    }


def route_check(paths: Paths, raw_query: str) -> dict[str, Any]:
    query, is_ip = canonical_route_query(raw_query)
    store = load_store(paths)
    running = service_active(SERVICE)
    routing = routing_status(paths, running, store)
    mode = str(routing["mode"])
    if mode == "global":
        result = {"outcome": "vpn", "ruleType": "MODE", "rulePayload": "global",
                  "target": "PROXY", "source": "mode"}
    elif mode == "direct":
        result = {"outcome": "direct", "ruleType": "MODE", "rulePayload": "direct",
                  "target": "DIRECT", "source": "mode"}
    else:
        matched = custom_rule_match(store["customRules"], query, is_ip)
        if matched is not None:
            target = {"proxy": "PROXY", "direct": "DIRECT", "reject": "REJECT-DROP"}[
                str(matched["action"])
            ]
            result = {
                "outcome": route_outcome(target),
                "ruleType": str(matched["kind"]).upper(),
                "rulePayload": str(matched["value"]),
                "target": target,
                "source": "custom",
            }
        elif running and mode == "rule":
            result = live_route_match(paths, query, is_ip)
        else:
            result = {
                "outcome": "unknown", "ruleType": "RULE-SET", "rulePayload": "",
                "target": "", "source": "disconnected",
            }
    return {"version": 1, "query": query, **result}


def find_core(paths: Paths) -> Path:
    candidates: list[str] = []
    override = os.environ.get("OMAVLESS_MIHOMO", "").strip()
    if override:
        candidates.append(override)
    candidates.append(str(paths.home / ".local" / "bin" / "mihomo"))
    if shutil.which("mihomo"):
        candidates.append(str(shutil.which("mihomo")))
    for candidate in candidates:
        path = Path(candidate).expanduser()
        if path.is_file() and os.access(path, os.X_OK):
            return path.resolve()
    raise BackendError("mihomo is not installed (install mihomo-bin or use ~/.local/bin/mihomo)")


def run(command: list[str], *, check: bool = True, input_text: str | None = None,
        timeout: float | None = COMMAND_TIMEOUT_SECONDS) -> subprocess.CompletedProcess[str]:
    try:
        result = subprocess.run(
            command, input=input_text, text=True, capture_output=True, timeout=timeout
        )
    except subprocess.TimeoutExpired as exc:
        label = Path(command[0]).name if command else "Command"
        duration = f" after {timeout:g} seconds" if timeout is not None else ""
        raise BackendError(f"{label} timed out{duration}") from exc
    if check and result.returncode != 0:
        raise BackendError((result.stderr or result.stdout or "Command failed").strip())
    return result


def systemctl(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return run([os.environ.get("OMAVLESS_SYSTEMCTL", "systemctl"), "--user", *args], check=check)


def service_active(name: str) -> bool:
    return systemctl("is-active", "--quiet", name, check=False).returncode == 0


def service_enabled(name: str) -> bool:
    return systemctl("is-enabled", "--quiet", name, check=False).returncode == 0


def startup_unit(paths: Paths) -> Path:
    return paths.unit.with_name(STARTUP_SERVICE)


def core_setup_status(paths: Paths) -> dict[str, Any]:
    """Return bounded setup facts without executing or exposing VPN credentials."""
    try:
        core = find_core(paths)
    except BackendError:
        return {"installed": False, "tunReady": False, "path": ""}
    getcap = shutil.which("getcap")
    capabilities = ""
    if getcap:
        result = run([getcap, str(core)], check=False, timeout=3)
        if result.returncode == 0:
            capabilities = result.stdout.lower()
    required = ("cap_net_admin", "cap_net_raw", "cap_net_bind_service")
    return {
        "installed": True,
        "tunReady": all(capability in capabilities for capability in required),
        "path": str(core)[:4096],
    }


def effective_startup_enabled(store: dict[str, Any]) -> bool:
    if bool(store.get("startupConfigured", False)):
        return bool(store.get("startup", {}).get("enabled", False))
    # Before the Settings model existed, connect enabled the main unit
    # directly. Preserve that exact state until the user makes a choice.
    return service_enabled(STARTUP_SERVICE) or service_enabled(SERVICE)


def startup_status(store: dict[str, Any], routing_mode: str = "") -> dict[str, Any]:
    startup = store["startup"]
    configured = bool(store.get("startupConfigured", False))
    mode = str(startup["mode"])
    if not configured and routing_mode in {"rule", "global"}:
        mode = routing_mode
    return {
        "enabled": effective_startup_enabled(store),
        "configured": configured,
        "target": str(startup["target"]),
        "profileId": str(startup.get("profileId", "")),
        "mode": mode,
    }


def service_uptime_seconds(name: str, running: bool) -> int:
    """Return bounded active uptime without trusting wall-clock timestamps."""
    if not running:
        return 0
    result = systemctl(
        "show", "--property=ActiveEnterTimestampMonotonic", "--value", name,
        check=False,
    )
    try:
        started_usec = int(result.stdout.strip())
    except (TypeError, ValueError):
        return 0
    if result.returncode != 0 or started_usec <= 0:
        return 0
    now_usec = int(time.clock_gettime(time.CLOCK_BOOTTIME) * 1_000_000)
    return max(0, min((now_usec - started_usec) // 1_000_000, 10 * 365 * 86400))


def external_tun_interfaces(
    own_device: str, running: bool, sys_class_net: Path = Path("/sys/class/net")
) -> list[str]:
    """List other kernel TUN devices. This is evidence, not proof of routing."""
    found: list[str] = []
    try:
        entries = list(sys_class_net.iterdir())[:512]
    except OSError:
        return found
    for entry in entries:
        name = entry.name
        if running and name == own_device:
            continue
        if len(name) > 32 or not re.fullmatch(r"[A-Za-z0-9_.:-]+", name):
            continue
        if (entry / "tun_flags").is_file():
            found.append(name)
    return sorted(found)[:8]


def vpn_process_labels(proc_root: Path = Path("/proc"), own_pid: int = 0) -> list[str]:
    """Recognise common user VPN cores without exposing command lines."""
    labels: list[str] = []
    known = (
        ("v2rayn", "V2RayN"),
        ("mihomo", "Mihomo"),
        ("xray", "Xray"),
        ("sing-box", "sing-box"),
        ("clash", "Clash"),
    )
    try:
        entries = list(proc_root.iterdir())[:32768]
    except OSError:
        return labels
    for entry in entries:
        if not entry.name.isdigit() or int(entry.name) in {own_pid, os.getpid()}:
            continue
        try:
            value = (entry / "comm").read_text(encoding="utf-8", errors="replace")[:128].strip().lower()
        except OSError:
            continue
        for needle, label in known:
            if needle in value and label not in labels:
                labels.append(label)
                break
        if len(labels) >= 8:
            break
    return labels


def routing_conflicts(paths: Paths, running: bool) -> list[str]:
    """Best-effort hints for concurrent full-tunnel software, never a blocker."""
    interfaces = external_tun_interfaces(TUN_DEVICE, running)
    mihoro_active = service_active(CONFLICT_SERVICE)
    if not interfaces and not mihoro_active:
        return []
    own_pid_result = systemctl("show", "--property=MainPID", "--value", SERVICE, check=False)
    try:
        own_pid = int(own_pid_result.stdout.strip()) if own_pid_result.returncode == 0 else 0
    except ValueError:
        own_pid = 0
    labels = vpn_process_labels(own_pid=own_pid)
    if mihoro_active and "Mihomo" not in labels:
        labels.insert(0, "Mihoro")
    result = labels[:4]
    for name in interfaces:
        item = f"TUN {name}"
        if item not in result:
            result.append(item)
    return result[:8]


def systemd_quote(value: str) -> str:
    escaped = (value.replace("\\", "\\\\").replace('"', '\\"').replace("%", "%%")
               .replace("\n", "\\n").replace("\r", "\\r").replace("\t", "\\t"))
    return '"' + escaped + '"'


def ensure_unit(paths: Paths, core: Path) -> None:
    unit = f"""[Unit]
Description=OmaVLESS tunnel (Mihomo core)
Wants=network-online.target
After=network-online.target

[Service]
ExecStart={systemd_quote(str(core))} -d {systemd_quote(str(paths.config_dir))} -f {systemd_quote(str(paths.config))}
Restart=on-failure
RestartSec=2

[Install]
WantedBy=default.target
"""
    if paths.unit.is_symlink():
        raise BackendError(f"Refusing symlinked systemd unit: {paths.unit}")
    current = read_text_file(paths.unit, 64 * 1024, "systemd unit") if paths.unit.exists() else ""
    if current != unit:
        atomic_write(paths.unit, unit, 0o644)
        systemctl("daemon-reload")


def ensure_startup_unit(paths: Paths) -> None:
    helper = startup_unit(paths)
    backend = PLUGIN_DIR / "backend.sh"
    unit = f"""[Unit]
Description=OmaVLESS login autoconnect
Wants=network-online.target
After=network-online.target

[Service]
Type=oneshot
ExecStart={systemd_quote(str(backend))} startup-connect

[Install]
WantedBy=default.target
"""
    if helper.is_symlink():
        raise BackendError(f"Refusing symlinked systemd unit: {helper}")
    current = read_text_file(helper, 64 * 1024, "autostart systemd unit") if helper.exists() else ""
    if current != unit:
        atomic_write(helper, unit, 0o644)
        systemctl("daemon-reload")


def test_config(paths: Paths, core: Path, config: str) -> Path:
    paths.config_dir.mkdir(parents=True, exist_ok=True)
    fd, name = tempfile.mkstemp(prefix=".candidate.", suffix=".yaml", dir=paths.config_dir)
    candidate = Path(name)
    try:
        os.fchmod(fd, 0o600)
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            handle.write(config)
        result = run([str(core), "-t", "-d", str(paths.config_dir), "-f", str(candidate)], check=False)
        if result.returncode != 0:
            raise BackendError((result.stderr or result.stdout or "Mihomo rejected the generated config").strip())
        return candidate
    except Exception:
        with contextlib.suppress(FileNotFoundError):
            candidate.unlink()
        raise


def profile_by_id(store: dict[str, Any], profile_id: str) -> dict[str, Any]:
    for profile in store["profiles"]:
        if str(profile.get("id", "")) == profile_id:
            return profile
    raise BackendError("No such VLESS profile")


def mark_intent(paths: Paths, profile_id: str) -> None:
    ensure_runtime(paths)
    observed_ms = time.time_ns() // 1_000_000
    atomic_write(paths.runtime / f"omavless.{os.getuid()}.intent", f"{profile_id} {observed_ms}\n")


def normalize_epoch_ms(value: int) -> int:
    # Prototype builds wrote whole seconds. Runtime markers are disposable,
    # but accepting both formats avoids a misleading drop after an upgrade.
    return value * 1000 if value < 1_000_000_000_000 else value


def clear_intent(paths: Paths, profile_id: str, observed_ms: int | None = None) -> None:
    intent = paths.runtime / f"omavless.{os.getuid()}.intent"
    if not intent.exists():
        return
    current = read_text_file(intent, 1024, "disconnect intent", private=True).split()
    if len(current) < 2:
        intent.unlink()
        return
    try:
        saved_ms = normalize_epoch_ms(int(current[1]))
    except ValueError:
        intent.unlink()
        return
    if current[0] == profile_id and (observed_ms is None or saved_ms <= observed_ms):
        intent.unlink()


def drop_notice_path(paths: Paths, profile_id: str) -> Path:
    return paths.runtime / f"omavless.{os.getuid()}.drop.{profile_id}"


def clear_drop_notice(paths: Paths, profile_id: str, observed_ms: int | None = None) -> None:
    notice = drop_notice_path(paths, profile_id)
    if not notice.exists():
        return
    try:
        saved_ms = normalize_epoch_ms(int(read_text_file(
            notice, 1024, "drop notice", private=True
        ).strip()))
    except (BackendError, ValueError):
        notice.unlink()
        return
    if observed_ms is None or saved_ms <= observed_ms:
        notice.unlink()


def mark_active(paths: Paths, profile_id: str, observed_ms: int | None = None) -> None:
    clear_intent(paths, profile_id, observed_ms)
    clear_drop_notice(paths, profile_id, observed_ms)


def resolve_startup_profile(store: dict[str, Any], target: str, profile_id: str) -> dict[str, Any]:
    selected_id = profile_id
    if target == "last":
        selected_id = str(store.get("lastId", ""))
        if not selected_id and store["profiles"]:
            selected_id = str(store["profiles"][0]["id"])
    if not selected_id:
        raise BackendError(
            "Connect a profile once before using Last used at login"
            if target == "last" else "Choose a profile for login autoconnect"
        )
    return profile_by_id(store, selected_id)


def restore_unit_enabled(name: str, enabled: bool) -> str:
    if not enabled and not service_enabled(name):
        return ""
    result = systemctl("enable" if enabled else "disable", name, check=False)
    return "" if result.returncode == 0 else (
        result.stderr or result.stdout or f"could not restore {name}"
    ).strip()


def configure_startup(
    paths: Paths, enabled: bool, target: str, profile_id: str, mode: str
) -> None:
    if target not in {"last", "profile"}:
        raise BackendError("Unsupported login autoconnect target")
    if mode not in {"rule", "global"}:
        raise BackendError("Login autoconnect supports only Routing or Full VPN")
    store = load_store(paths)
    previous_store = json.loads(json.dumps(store))
    old_main_enabled = service_enabled(SERVICE)
    old_helper_enabled = service_enabled(STARTUP_SERVICE)
    if enabled:
        if mode == "rule" and not store.get("routingPreset"):
            raise BackendError("Choose a Routing country preset before using it at login")
        profile = resolve_startup_profile(store, target, profile_id)
        setup = core_setup_status(paths)
        if not setup["installed"]:
            raise BackendError("Install Mihomo before enabling login autoconnect")
        if not setup["tunReady"]:
            raise BackendError("Grant Mihomo TUN capabilities before enabling login autoconnect")
        core = find_core(paths)
        ensure_unit(paths, core)
        ensure_startup_unit(paths)
        candidate = test_config(paths, core, render_config_mode(paths, profile, mode, store))
        with contextlib.suppress(FileNotFoundError):
            candidate.unlink()
    else:
        # Write the helper too, so turning the option back on later never
        # depends on an old unit left by a prototype build.
        ensure_startup_unit(paths)

    store["startupConfigured"] = True
    store["startup"] = {
        "enabled": enabled,
        "target": target,
        "profileId": profile_id if target == "profile" else "",
        "mode": mode,
    }
    save_store(paths, store)
    try:
        if enabled:
            result = systemctl("enable", STARTUP_SERVICE, check=False)
            if result.returncode != 0:
                raise BackendError((result.stderr or result.stdout or "Could not enable login autoconnect").strip())
            # The main service remains startable but no longer owns a login
            # symlink. The helper prepares the chosen profile first.
            if old_main_enabled:
                result = systemctl("disable", SERVICE, check=False)
                if result.returncode != 0:
                    raise BackendError((result.stderr or result.stdout or "Could not migrate login autoconnect").strip())
        else:
            for name, was_enabled in (
                (STARTUP_SERVICE, old_helper_enabled), (SERVICE, old_main_enabled)
            ):
                if not was_enabled:
                    continue
                result = systemctl("disable", name, check=False)
                if result.returncode != 0:
                    reason = (
                        result.stderr or result.stdout
                        or "Could not disable login autoconnect"
                    ).strip()
                    raise BackendError(reason)
    except Exception as exc:
        rollback_errors: list[str] = []
        try:
            save_store(paths, previous_store)
        except Exception as rollback_exc:
            rollback_errors.append(f"preference restore: {rollback_exc}")
        for name, was_enabled in (
            (SERVICE, old_main_enabled), (STARTUP_SERVICE, old_helper_enabled)
        ):
            reason = restore_unit_enabled(name, was_enabled)
            if reason:
                rollback_errors.append(reason)
        if rollback_errors:
            raise BackendError(
                f"{exc}; autoconnect rollback needs manual recovery ({'; '.join(rollback_errors)})",
                21,
            ) from exc
        raise


def startup_connect(paths: Paths) -> None:
    """Prepare and start the configured profile from the login helper unit."""
    store = load_store(paths)
    startup = store["startup"]
    if not store.get("startupConfigured") or not startup.get("enabled"):
        raise BackendError("Login autoconnect is disabled")
    if service_active(CONFLICT_SERVICE):
        raise BackendError("Mihoro is running. Stop mihomo.service before starting OmaVLESS")
    profile = resolve_startup_profile(
        store, str(startup["target"]), str(startup.get("profileId", ""))
    )
    profile_id = str(profile["id"])
    core = find_core(paths)
    ensure_unit(paths, core)
    previous_store = json.loads(json.dumps(store))
    old_config = (
        read_text_file(paths.config, MAX_TEMPLATE_BYTES, "generated config", private=True).encode()
        if paths.config.exists() else None
    )
    was_active = service_active(SERVICE)
    candidate = test_config(
        paths, core, render_config_mode(paths, profile, str(startup["mode"]), store)
    )
    try:
        prepare_controller_socket(paths)
        os.replace(candidate, paths.config)
        os.chmod(paths.config, 0o600)
        systemctl("restart" if was_active else "start", SERVICE)
        if not service_active(SERVICE):
            raise BackendError("OmaVLESS service did not remain active after login autoconnect")
        store["activeId"] = profile_id
        store["lastId"] = profile_id
        save_store(paths, store)
        mark_active(paths, profile_id)
    except Exception as exc:
        rollback_errors: list[str] = []
        try:
            if old_config is None:
                with contextlib.suppress(FileNotFoundError):
                    paths.config.unlink()
            else:
                atomic_write(paths.config, old_config)
        except Exception as rollback_exc:
            rollback_errors.append(f"config restore: {rollback_exc}")
        try:
            save_store(paths, previous_store)
        except Exception as rollback_exc:
            rollback_errors.append(f"profile state restore: {rollback_exc}")
        result = systemctl("restart" if was_active else "stop", SERVICE, check=False)
        restored = service_active(SERVICE) if was_active else not service_active(SERVICE)
        if result.returncode != 0 or not restored:
            reason = (result.stderr or result.stdout or "unexpected service state").strip()
            rollback_errors.append(f"service restore: {reason}")
        if rollback_errors:
            raise BackendError(
                f"{exc}; login autoconnect rollback needs manual recovery ({'; '.join(rollback_errors)})",
                21,
            ) from exc
        raise
    finally:
        with contextlib.suppress(FileNotFoundError):
            candidate.unlink()


def connect_profile(paths: Paths, profile_id: str) -> None:
    store = load_store(paths)
    profile = profile_by_id(store, profile_id)
    if service_active(CONFLICT_SERVICE):
        raise BackendError("Mihoro is running. Stop mihomo.service before starting OmaVLESS")
    core = find_core(paths)
    ensure_unit(paths, core)
    candidate = test_config(paths, core, render_config(paths, profile, store))
    old_config = (read_text_file(paths.config, MAX_TEMPLATE_BYTES, "generated config", private=True).encode()
                  if paths.config.exists() else None)
    was_active = service_active(SERVICE)
    try:
        prepare_controller_socket(paths)
        os.replace(candidate, paths.config)
        os.chmod(paths.config, 0o600)
        configured_startup = bool(store.get("startupConfigured", False))
        if was_active:
            systemctl("restart", SERVICE)
        elif configured_startup:
            systemctl("start", SERVICE)
        else:
            systemctl("enable", "--now", SERVICE)
        if not service_active(SERVICE):
            raise BackendError("OmaVLESS service did not remain active after startup")
        store["activeId"] = profile_id
        store["lastId"] = profile_id
        save_store(paths, store)
        mark_active(paths, profile_id)
    except Exception as exc:
        rollback_errors: list[str] = []
        try:
            if old_config is None:
                with contextlib.suppress(FileNotFoundError):
                    paths.config.unlink()
            else:
                atomic_write(paths.config, old_config)
        except Exception as rollback_exc:
            rollback_errors.append(f"config restore: {rollback_exc}")
        if was_active:
            result = systemctl("restart", SERVICE, check=False)
        elif configured_startup:
            result = systemctl("stop", SERVICE, check=False)
        else:
            result = systemctl("disable", "--now", SERVICE, check=False)
        restored = service_active(SERVICE) if was_active else not service_active(SERVICE)
        if result.returncode != 0 or not restored:
            reason = (result.stderr or result.stdout or "unexpected service state").strip()
            rollback_errors.append(f"service restore: {reason}")
        if rollback_errors:
            raise BackendError(
                f"{exc}; rollback needs manual recovery ({'; '.join(rollback_errors)})", 21
            ) from exc
        raise BackendError(f"{exc}; previous OmaVLESS state restored", 20) from exc
    finally:
        with contextlib.suppress(FileNotFoundError):
            candidate.unlink()


def stop_service(paths: Paths, expected_profile_id: str = "") -> None:
    store = load_store(paths)
    active_id = str(store.get("activeId", ""))
    if expected_profile_id and active_id != expected_profile_id:
        raise BackendError("That VLESS profile is no longer active")
    if active_id:
        mark_intent(paths, active_id)
    configured_startup = bool(store.get("startupConfigured", False))
    if configured_startup:
        result = systemctl("stop", SERVICE, check=False)
    else:
        result = systemctl("disable", "--now", SERVICE, check=False)
    if result.returncode != 0 and service_active(SERVICE):
        if active_id:
            clear_intent(paths, active_id)
        reason = (result.stderr or result.stdout or "systemd refused to stop OmaVLESS").strip()
        raise BackendError(reason)
    with contextlib.suppress(FileNotFoundError):
        controller_socket(paths).unlink()
    store["activeId"] = ""
    removed_ids = {
        str(profile["id"]) for profile in store["profiles"]
        if profile.get("missing", False)
    }
    if removed_ids:
        store["profiles"] = [
            profile for profile in store["profiles"] if profile.get("id") not in removed_ids
        ]
        if store.get("lastId") in removed_ids:
            store["lastId"] = store["profiles"][0]["id"] if store["profiles"] else ""
    save_store(paths, store)


def import_profile(paths: Paths, name: str, old_id: str, text: str) -> None:
    uri = extract_vless_uri(text)
    parsed = parse_vless(uri)
    store = load_store(paths)
    name = clean_name(name or parsed["suggested_name"] or parsed["server"])
    if any(p.get("name") == name and p.get("id") != old_id for p in store["profiles"]):
        raise BackendError(f"A profile named {name} already exists")
    old_store = json.loads(json.dumps(store))
    old_config = (read_text_file(paths.config, MAX_TEMPLATE_BYTES, "generated config", private=True).encode()
                  if paths.config.exists() else None)
    was_active = bool(service_active(SERVICE) and old_id and store.get("activeId") == old_id)
    if old_id:
        profile = profile_by_id(store, old_id)
        if profile.get("subscriptionId"):
            raise BackendError("Subscribed profiles are updated from their subscription")
        profile.update({"name": name, "uri": uri})
        profile_id = old_id
    else:
        profile_id = str(uuidlib.uuid4())
        store["profiles"].append({"id": profile_id, "name": name, "uri": uri})
    save_store(paths, store)
    if was_active:
        try:
            connect_profile(paths, profile_id)
        except Exception as exc:
            rollback_errors: list[str] = []
            try:
                save_store(paths, old_store)
            except Exception as rollback_exc:
                rollback_errors.append(f"profile restore: {rollback_exc}")
            if old_config is not None:
                try:
                    atomic_write(paths.config, old_config)
                    result = systemctl("restart", SERVICE, check=False)
                    if result.returncode != 0 or not service_active(SERVICE):
                        reason = (result.stderr or result.stdout or "service is inactive").strip()
                        rollback_errors.append(f"service restore: {reason}")
                except Exception as rollback_exc:
                    rollback_errors.append(f"config restore: {rollback_exc}")
            if rollback_errors:
                raise BackendError(
                    f"{exc}; edit rollback needs manual recovery ({'; '.join(rollback_errors)})", 6
                ) from exc
            raise


def rename_profile(paths: Paths, profile_id: str, new_name: str) -> None:
    store = load_store(paths)
    profile = profile_by_id(store, profile_id)
    if profile.get("subscriptionId"):
        raise BackendError("Subscribed profile names are managed by their subscription")
    new_name = clean_name(new_name)
    if any(p.get("name") == new_name and p.get("id") != profile_id for p in store["profiles"]):
        raise BackendError(f"A profile named {new_name} already exists")
    old_name = str(profile["name"])
    profile["name"] = new_name
    save_store(paths, store)
    if service_active(SERVICE) and store.get("activeId") == profile_id:
        try:
            connect_profile(paths, profile_id)
        except Exception:
            profile["name"] = old_name
            save_store(paths, store)
            raise


def set_profile_favorite(paths: Paths, profile_id: str, enabled: bool) -> None:
    store = load_store(paths)
    profile = profile_by_id(store, profile_id)
    profile["favorite"] = enabled
    save_store(paths, store)


def delete_profile(paths: Paths, profile_id: str) -> None:
    store = load_store(paths)
    profile = profile_by_id(store, profile_id)
    if profile.get("subscriptionId"):
        raise BackendError("Remove this profile from its subscription provider")
    if service_active(SERVICE) and store.get("activeId") == profile_id:
        stop_service(paths)
        store = load_store(paths)
    store["profiles"] = [p for p in store["profiles"] if p.get("id") != profile_id]
    if store.get("lastId") == profile_id:
        store["lastId"] = store["profiles"][0]["id"] if store["profiles"] else ""
    save_store(paths, store)


def status_text(paths: Paths) -> str:
    store = load_store(paths)
    running = service_active(SERVICE)
    stored_active_id = str(store.get("activeId", ""))
    if running and not stored_active_id:
        raise BackendError("omavless.service is running without an active profile")
    active_id = stored_active_id if running else ""
    profiles = sorted(store["profiles"], key=lambda p: (
        p.get("id") != active_id,
        not bool(p.get("favorite", False)),
        str(p.get("name", "")).lower(),
    ))
    subscriptions = {str(item["id"]): item for item in store["subscriptions"]}
    endpoints: dict[str, str] = {}
    for profile in profiles:
        try:
            endpoints[str(profile["id"])] = str(parse_vless(str(profile["uri"]))["server"])
        except (BackendError, KeyError, TypeError):
            endpoints[str(profile.get("id", ""))] = ""
    routing = routing_status(paths, running, store)
    payload = {
        "version": 1,
        "capabilities": PUBLIC_CAPABILITIES,
        "profiles": [
            {
                "id": str(profile["id"]),
                "name": str(profile["name"]),
                "device": TUN_DEVICE if profile["id"] == active_id else "",
                "active": profile["id"] == active_id,
                "subscriptionId": str(profile.get("subscriptionId", "")),
                "sourceName": str(subscriptions.get(
                    str(profile.get("subscriptionId", "")), {}
                ).get("name", "")),
                "missing": bool(profile.get("missing", False)),
                "favorite": bool(profile.get("favorite", False)),
                "server": endpoints.get(str(profile["id"]), ""),
            }
            for profile in profiles
        ],
        "activeId": active_id,
        "lastId": str(store.get("lastId", "")),
        "subscriptions": [
            {
                "id": str(subscription["id"]),
                "name": str(subscription["name"]),
                "updatedAt": int(subscription.get("updatedAt", 0)),
                "profileCount": sum(
                    1 for profile in store["profiles"]
                    if profile.get("subscriptionId") == subscription["id"]
                ),
                "staleCount": sum(
                    1 for profile in store["profiles"]
                    if profile.get("subscriptionId") == subscription["id"]
                    and profile.get("missing", False)
                ),
            }
            for subscription in sorted(
                store["subscriptions"], key=lambda item: str(item.get("name", "")).lower()
            )
        ],
        "routing": routing,
        "coreSetup": core_setup_status(paths),
        "startup": startup_status(store, str(routing["mode"])),
        "onboardingComplete": bool(store.get("onboardingComplete", False)),
        "uptimeSeconds": service_uptime_seconds(SERVICE, running),
        "conflicts": routing_conflicts(paths, running),
    }
    return json.dumps(payload, ensure_ascii=False, separators=(",", ":")) + "\n"


def status(paths: Paths) -> None:
    # Every monitor owns a widget instance and may poll at the same moment.
    # Serialize those polls across processes and share the one-second result,
    # so only one systemctl probe is paid for and mutations cannot be observed
    # half-way through their transaction.
    ensure_runtime(paths)
    with operation_lock(paths):
        cache = status_cache(paths)
        try:
            age = time.time() - cache.stat().st_mtime
            fresh = 0 <= age <= STATUS_CACHE_SECONDS
            text = cache.read_text(encoding="utf-8") if fresh else ""
        except (FileNotFoundError, OSError, UnicodeError):
            text = ""
        if not text:
            text = status_text(paths)
            atomic_write(cache, text)
    print(text, end="")


def diagnostics_payload(paths: Paths) -> dict[str, Any]:
    """Build a support snapshot from counts and booleans, never raw profiles."""
    store = load_store(paths)
    running = service_active(SERVICE)
    routing = routing_status(paths, running, store)
    setup = core_setup_status(paths)
    startup = startup_status(store, str(routing["mode"]))
    latest_subscription_update = max(
        (int(item.get("updatedAt", 0)) for item in store["subscriptions"]),
        default=0,
    )
    return {
        "schemaVersion": 1,
        "generatedAt": time.time_ns() // 1_000_000,
        "plugin": {"id": "kdk.omavless", "version": PLUGIN_VERSION},
        "environment": {
            "platform": sys.platform,
            "python": ".".join(str(part) for part in sys.version_info[:3]),
        },
        "core": {
            "installed": bool(setup["installed"]),
            "tunReady": bool(setup["tunReady"]),
            "controllerReady": controller_socket(paths).exists(),
        },
        "service": {
            "active": running,
            "enabled": service_enabled(SERVICE),
            "loginAutoconnectEnabled": service_enabled(STARTUP_SERVICE),
            "stateConsistent": not running or bool(store.get("activeId", "")),
        },
        "inventory": {
            "profiles": len(store["profiles"]),
            "favorites": sum(bool(item.get("favorite", False)) for item in store["profiles"]),
            "subscriptions": len(store["subscriptions"]),
            "customRules": len(store["customRules"]),
        },
        "routing": {
            "mode": str(routing["mode"]),
            "source": str(routing["source"]),
            "preset": str(routing["preset"]),
            "configured": bool(routing["configured"]),
            "rules": int(routing["ruleCount"]),
            "providers": int(routing["providerCount"]),
            "lastManualRuleUpdate": int(routing["rulesUpdatedAt"]),
        },
        "startup": {
            "configured": bool(startup["configured"]),
            "enabled": bool(startup["enabled"]),
            "target": str(startup["target"]),
            "mode": str(startup["mode"]),
        },
        "updates": {"latestSubscription": latest_subscription_update},
        "files": {
            "store": paths.store.is_file() and not paths.store.is_symlink(),
            "template": paths.template.is_file() and not paths.template.is_symlink(),
            "generatedConfig": paths.config.is_file() and not paths.config.is_symlink(),
            "serviceUnit": paths.unit.is_file() and not paths.unit.is_symlink(),
        },
        "conflictCount": len(routing_conflicts(paths, running)),
    }


def diagnostics_text(paths: Paths) -> str:
    text = json.dumps(
        diagnostics_payload(paths), ensure_ascii=False, indent=2, sort_keys=True
    ) + "\n"
    # Defense in depth: future fields must not accidentally turn this support
    # file into a credential/profile export.
    forbidden = re.compile(
        r"(?i)vless://|https?://|[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}"
    )
    if forbidden.search(text):
        raise BackendError("Refusing diagnostics containing a credential or private address")
    return text


def export_diagnostics(paths: Paths, destination: str) -> Path:
    target = Path(destination).expanduser()
    if not target.name:
        raise BackendError("Choose a diagnostics file destination")
    atomic_write(target, diagnostics_text(paths), 0o600)
    return target


def interface_addresses(device: str) -> list[str]:
    if not device:
        return []
    ip = shutil.which("ip")
    if not ip:
        return []
    result = run([ip, "-j", "address", "show", "dev", device], check=False)
    if result.returncode != 0:
        return []
    try:
        rows = json.loads(result.stdout)
    except json.JSONDecodeError:
        return []
    addresses: list[str] = []
    for row in rows if isinstance(rows, list) else []:
        for item in row.get("addr_info", []) if isinstance(row, dict) else []:
            if not isinstance(item, dict) or item.get("scope") != "global":
                continue
            local, prefix = item.get("local"), item.get("prefixlen")
            if isinstance(local, str) and isinstance(prefix, int):
                addresses.append(f"{local}/{prefix}")
    return addresses


def details(paths: Paths, profile_id: str, device: str = "") -> None:
    node = parse_vless(profile_by_id(load_store(paths), profile_id)["uri"])
    payload = {
        "version": 1,
        "address": ", ".join(interface_addresses(device)),
        "server": f"{node['server']}:{node['port']}",
        "transport": f"{node['network']} / {node['security']}",
        "sni": node["servername"] or "--",
    }
    print(json.dumps(payload, ensure_ascii=False, separators=(",", ":")))


def export_file(paths: Paths, profile_id: str, destination: str) -> None:
    profile = profile_by_id(load_store(paths), profile_id)
    atomic_write(Path(destination).expanduser(), str(profile["uri"]) + "\n")


def qr_png(paths: Paths, profile_id: str) -> None:
    ensure_runtime(paths)
    qrencode = shutil.which("qrencode")
    if not qrencode:
        fail("qrencode is not installed", 2)
    profile = profile_by_id(load_store(paths), profile_id)
    fd, name = tempfile.mkstemp(prefix=f"vless-qr.{os.getppid()}.", suffix=".png", dir=paths.runtime)
    os.close(fd)
    target = Path(name)
    try:
        run([qrencode, "-o", str(target), "-s", "8", "-m", "2"], input_text=str(profile["uri"]))
        os.chmod(target, 0o600)
        print(target)
    except Exception:
        with contextlib.suppress(FileNotFoundError):
            target.unlink()
        raise


def edit_profile(paths: Paths, profile_id: str, name: str, seed: str) -> int:
    zenity = shutil.which("zenity")
    if not zenity:
        return 2
    profile = profile_by_id(load_store(paths), profile_id)
    source = str(profile["uri"]) + "\n"
    ensure_runtime(paths)
    with tempfile.NamedTemporaryFile("w", encoding="utf-8", delete=False, dir=paths.runtime,
                                     prefix=f"vless-edit.{os.getppid()}.") as handle:
        handle.write(seed or source)
        filename = handle.name
    os.chmod(filename, 0o600)
    try:
        result = run([zenity, "--text-info", "--editable", f"--filename={filename}",
                      f"--title=Edit {name}", "--width=700", "--height=300"],
                     check=False, timeout=None)
        if result.returncode != 0:
            return 3
        if len(result.stdout.encode("utf-8")) > MAX_IMPORT_BYTES:
            raise BackendError(
                f"Edited VLESS input is too large (maximum {MAX_IMPORT_BYTES // 1024} KiB)"
            )
        if result.stdout == source:
            return 4
        print(result.stdout, end="")
        return 0
    finally:
        with contextlib.suppress(FileNotFoundError):
            Path(filename).unlink()


def cleanup_runtime(paths: Paths) -> None:
    ensure_runtime(paths)
    now = time.time()
    for pattern in ("vless-qr.*.png", "vless-edit.*"):
        for path in paths.runtime.glob(pattern):
            with contextlib.suppress(OSError):
                match = re.match(r"^vless-(?:qr|edit)\.(\d+)\.", path.name)
                owner_pid = int(match.group(1)) if match else -1
                # The current process is necessarily alive even in restricted
                # containers where its own /proc entry can be transiently
                # hidden from a filesystem lookup.
                owner_alive = bool(
                    match and (
                        owner_pid == os.getpid()
                        or Path("/proc", str(owner_pid)).exists()
                    )
                )
                # A dead Quickshell cannot clean its credential-bearing temp
                # files. Reap those immediately; only malformed legacy names
                # need the conservative age fallback.
                if (match and not owner_alive) or (not match and now - path.stat().st_mtime > 86400):
                    path.unlink()


def notify_drop(paths: Paths, profile_id: str, name: str) -> int:
    intent = paths.runtime / f"omavless.{os.getuid()}.intent"
    if intent.exists():
        with contextlib.suppress(BackendError, OSError, ValueError):
            saved_id, saved_at = read_text_file(
                intent, 1024, "disconnect intent", private=True
            ).split()[:2]
            age_ms = time.time_ns() // 1_000_000 - normalize_epoch_ms(int(saved_at))
            if saved_id == profile_id and 0 <= age_ms < 7_200_000:
                return 1
    # Every monitor sees the same transition and asks independently. The
    # runtime marker makes the first observer own both the desktop notice and
    # the urgent bar state; a later active observation re-arms the profile.
    notice = drop_notice_path(paths, profile_id)
    now_ms = time.time_ns() // 1_000_000
    if notice.exists():
        with contextlib.suppress(BackendError, OSError, ValueError):
            saved_ms = normalize_epoch_ms(int(read_text_file(
                notice, 1024, "drop notice", private=True
            ).strip()))
            if 0 <= now_ms - saved_ms < 30_000:
                return 2
    atomic_write(notice, f"{now_ms}\n")
    if shutil.which("notify-send"):
        run([str(shutil.which("notify-send")), "-a", "OmaVLESS", "OmaVLESS",
             f"Profile {name} was deactivated"], check=False)
    return 0


def build_parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(prog="backend.sh")
    sub = result.add_subparsers(dest="command", required=True)
    sub.add_parser("status")
    p = sub.add_parser("details"); p.add_argument("id"); p.add_argument("device", nargs="?")
    p = sub.add_parser("connect"); p.add_argument("id")
    p = sub.add_parser("down"); p.add_argument("id", nargs="?")
    sub.add_parser("down-all")
    p = sub.add_parser("delete"); p.add_argument("id")
    p = sub.add_parser("rename"); p.add_argument("id"); p.add_argument("name")
    p = sub.add_parser("favorite"); p.add_argument("id"); p.add_argument("enabled", choices=("on", "off"))
    p = sub.add_parser("import"); p.add_argument("name"); p.add_argument("old_id", nargs="?", default=""); p.add_argument("file", nargs="?")
    p = sub.add_parser("preview"); p.add_argument("file", nargs="?")
    p = sub.add_parser("export-file"); p.add_argument("id"); p.add_argument("path")
    sub.add_parser("diagnostics")
    p = sub.add_parser("diagnostics-export"); p.add_argument("path")
    p = sub.add_parser("qr-png"); p.add_argument("id")
    p = sub.add_parser("edit"); p.add_argument("id"); p.add_argument("name")
    p = sub.add_parser("notify-drop"); p.add_argument("id"); p.add_argument("name")
    p = sub.add_parser("mark-active"); p.add_argument("id"); p.add_argument("observed", nargs="?")
    sub.add_parser("cleanup-runtime"); sub.add_parser("cleanup-qr")
    p = sub.add_parser("adopt-template"); p.add_argument("path")
    p = sub.add_parser("use-routing")
    p.add_argument("profile", choices=tuple(ROUTING_PRESETS))
    p.add_argument("--keep-mode", action="store_true")
    p = sub.add_parser("set-mode"); p.add_argument("mode", choices=("rule", "global", "direct"))
    sub.add_parser("custom-rules")
    p = sub.add_parser("custom-rule-add")
    p.add_argument("kind", choices=("domain", "suffix", "ipcidr"))
    p.add_argument("action", choices=("proxy", "direct", "reject"))
    p = sub.add_parser("custom-rule-delete"); p.add_argument("id")
    sub.add_parser("route-check")
    sub.add_parser("rule-providers-refresh")
    p = sub.add_parser("startup-configure")
    p.add_argument("enabled", choices=("on", "off"))
    p.add_argument("target", choices=("last", "profile"))
    p.add_argument("profile_id")
    p.add_argument("mode", choices=("rule", "global"))
    sub.add_parser("startup-connect")
    sub.add_parser("onboarding-complete")
    p = sub.add_parser("subscription-save"); p.add_argument("name"); p.add_argument("id", nargs="?", default="")
    p = sub.add_parser("subscription-refresh"); p.add_argument("id")
    sub.add_parser("subscription-refresh-all")
    p = sub.add_parser("subscription-delete"); p.add_argument("id")
    p = sub.add_parser("subscription-url"); p.add_argument("id")
    p = sub.add_parser("subscription-probe"); p.add_argument("id")
    p = sub.add_parser("subscription-probe-stream"); p.add_argument("id")
    p = sub.add_parser("render-uri"); p.add_argument("name", nargs="?", default="VLESS")
    return result


def main() -> int:
    args = build_parser().parse_args()
    paths = Paths.current()
    ensure_private_dir(paths.config_dir)
    with operation_lock(paths):
        migrate_legacy_data(paths)
    if args.command == "status":
        status(paths)
    elif args.command == "preview":
        text = read_text_file(
            Path(args.file).expanduser(), MAX_IMPORT_BYTES, "VLESS preview file"
        ) if args.file else read_stdin_text(MAX_IMPORT_BYTES, "VLESS preview")
        print(json.dumps(preview_vless(text), ensure_ascii=False, separators=(",", ":")))
    elif args.command == "diagnostics":
        with operation_lock(paths):
            print(diagnostics_text(paths), end="")
    elif args.command == "diagnostics-export":
        with operation_lock(paths):
            print(export_diagnostics(paths, args.path))
    elif args.command == "details":
        details(paths, args.id, args.device)
    elif args.command == "export-file":
        export_file(paths, args.id, args.path)
    elif args.command == "qr-png":
        qr_png(paths, args.id)
    elif args.command in {"cleanup-runtime", "cleanup-qr"}:
        cleanup_runtime(paths)
    elif args.command == "edit":
        return edit_profile(paths, args.id, args.name, read_stdin_text(MAX_IMPORT_BYTES, "editor input"))
    elif args.command == "notify-drop":
        with operation_lock(paths):
            return notify_drop(paths, args.id, args.name)
    elif args.command == "mark-active":
        with operation_lock(paths):
            try:
                observed_ms = normalize_epoch_ms(int(args.observed)) if args.observed else None
            except ValueError as exc:
                raise BackendError("mark-active observation must be an epoch timestamp") from exc
            mark_active(paths, args.id, observed_ms)
    elif args.command == "render-uri":
        text = read_stdin_text(MAX_IMPORT_BYTES, "VLESS input")
        print(proxy_yaml({"name": clean_name(args.name), "uri": extract_vless_uri(text)}))
    elif args.command == "adopt-template":
        with operation_lock(paths):
            source = Path(args.path).expanduser()
            candidate = template_from_config(
                read_text_file(source, MAX_TEMPLATE_BYTES, "route template source")
            )
            if candidate.count(PROFILE_MARKER) != 1:
                raise BackendError(f"Route template must contain exactly one {PROFILE_MARKER}")
            atomic_write(paths.template, candidate)
            store = load_store(paths)
            store["routingPreset"] = "custom"
            save_store(paths, store)
    elif args.command == "use-routing":
        with operation_lock(paths):
            use_bundled_template(paths, args.profile, keep_mode=args.keep_mode)
            invalidate_status_cache(paths)
    elif args.command == "set-mode":
        with operation_lock(paths):
            set_routing_mode(paths, args.mode)
            invalidate_status_cache(paths)
    elif args.command == "custom-rules":
        with operation_lock(paths):
            print(custom_rules_text(paths), end="")
    elif args.command == "custom-rule-add":
        value = read_stdin_text(MAX_CUSTOM_RULE_VALUE_BYTES, "custom routing value")
        with operation_lock(paths):
            print(json.dumps(
                save_custom_rule(paths, args.kind, args.action, value),
                ensure_ascii=False, separators=(",", ":"),
            ))
            invalidate_status_cache(paths)
    elif args.command == "custom-rule-delete":
        with operation_lock(paths):
            delete_custom_rule(paths, args.id)
            invalidate_status_cache(paths)
    elif args.command == "route-check":
        value = read_stdin_text(MAX_CUSTOM_RULE_VALUE_BYTES, "routing query")
        print(json.dumps(route_check(paths, value), ensure_ascii=False, separators=(",", ":")))
    elif args.command == "rule-providers-refresh":
        print(json.dumps(refresh_rule_providers(paths), separators=(",", ":")))
    elif args.command == "startup-configure":
        with operation_lock(paths):
            configure_startup(
                paths, args.enabled == "on", args.target, args.profile_id, args.mode
            )
            invalidate_status_cache(paths)
    elif args.command == "startup-connect":
        with operation_lock(paths):
            startup_connect(paths)
            invalidate_status_cache(paths)
    elif args.command == "onboarding-complete":
        with operation_lock(paths):
            store = load_store(paths)
            store["onboardingComplete"] = True
            save_store(paths, store)
    elif args.command == "subscription-save":
        url = read_stdin_text(MAX_SUBSCRIPTION_URL_BYTES, "subscription URL")
        # Network I/O never holds the global mutation lock. A slow provider
        # must not prevent an urgent disconnect or make status monitors time
        # out; only the final validated store update is serialized.
        canonical_url = validate_subscription_url(url)
        entries, skipped = parse_subscription(fetch_subscription(canonical_url))
        with operation_lock(paths):
            result = commit_subscription(
                paths, args.name, args.id, canonical_url, entries, skipped
            )
        print(json.dumps(result, separators=(",", ":")))
    elif args.command == "subscription-refresh":
        with operation_lock(paths):
            original_url = str(subscription_by_id(load_store(paths), args.id)["url"])
        entries, skipped = parse_subscription(fetch_subscription(original_url))
        with operation_lock(paths):
            store = load_store(paths)
            subscription = subscription_by_id(store, args.id)
            if subscription["url"] != original_url:
                raise BackendError("Subscription changed while it was being updated; try again")
            result = sync_subscription_store(
                store, subscription, entries, time.time_ns() // 1_000_000
            )
            result["skipped"] = skipped
            save_store(paths, store)
        print(json.dumps(result, separators=(",", ":")))
    elif args.command == "subscription-refresh-all":
        with operation_lock(paths):
            snapshot = [
                (str(item["id"]), str(item["url"])) for item in load_store(paths)["subscriptions"]
            ]
        downloaded = [
            (subscription_id, url, *parse_subscription(fetch_subscription(url)))
            for subscription_id, url in snapshot
        ]
        with operation_lock(paths):
            store = load_store(paths)
            current = [(str(item["id"]), str(item["url"])) for item in store["subscriptions"]]
            if current != snapshot:
                raise BackendError("Subscriptions changed while they were being updated; try again")
            result = {"added": 0, "removed": 0, "stale": 0, "total": 0, "skipped": 0}
            updated_at = time.time_ns() // 1_000_000
            for subscription_id, _url, entries, skipped in downloaded:
                partial = sync_subscription_store(
                    store, subscription_by_id(store, subscription_id), entries, updated_at
                )
                for key in ("added", "removed", "stale", "total"):
                    result[key] += partial[key]
                result["skipped"] += skipped
            save_store(paths, store)
        print(json.dumps(result, separators=(",", ":")))
    elif args.command == "subscription-delete":
        with operation_lock(paths):
            delete_subscription(paths, args.id)
    elif args.command == "subscription-url":
        # Deliberately separate from status: the bearer URL is released only
        # for an explicit editor request, over a private process pipe.
        with operation_lock(paths):
            subscription = subscription_by_id(load_store(paths), args.id)
            print(str(subscription["url"]))
    elif args.command == "subscription-probe":
        # A probe is an explicit, read-only network action. It never holds the
        # mutation lock while endpoints are contacted and returns no secrets.
        print(json.dumps(probe_subscription(paths, args.id), separators=(",", ":")))
    elif args.command == "subscription-probe-stream":
        # NDJSON lets the panel reveal each server as soon as its DNS lookup
        # or first successful end-to-end request finishes.
        probe_subscription_stream(paths, args.id)
    else:
        with operation_lock(paths):
            if args.command == "connect":
                connect_profile(paths, args.id)
            elif args.command == "down":
                stop_service(paths, args.id or "")
            elif args.command == "down-all":
                stop_service(paths)
            elif args.command == "delete":
                delete_profile(paths, args.id)
            elif args.command == "rename":
                rename_profile(paths, args.id, args.name)
            elif args.command == "favorite":
                set_profile_favorite(paths, args.id, args.enabled == "on")
            elif args.command == "import":
                text = read_text_file(
                    Path(args.file).expanduser(), MAX_IMPORT_BYTES, "VLESS import file"
                ) if args.file else read_stdin_text(MAX_IMPORT_BYTES, "VLESS import")
                import_profile(paths, args.name, args.old_id, text)
            invalidate_status_cache(paths)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except BackendError as exc:
        fail(str(exc), exc.exit_code)
    except OSError as exc:
        fail(str(exc))
