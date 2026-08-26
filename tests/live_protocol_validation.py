#!/usr/bin/env python3
"""Opt-in, privacy-safe live validation runner for OmaVLESS profiles."""

from __future__ import annotations

import argparse
import contextlib
import datetime as dt
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any, Callable


MAX_CASES_BYTES = 64 * 1024
MAX_CASES = 32
MAX_BACKEND_OUTPUT = 512 * 1024
MAX_PROBE_BODY = 64 * 1024
DEFAULT_TIMEOUT = 30
SLUG_RE = re.compile(r"[a-z0-9][a-z0-9._-]{0,63}\Z")
PROFILE_ID_RE = re.compile(
    r"[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\Z",
    re.IGNORECASE,
)
PROTOCOLS = {"vless", "trojan", "hysteria2", "tuic"}
NETWORKS = {"normal", "udp-friendly", "udp-restricted"}
SOURCE_CLASSES = {"self-hosted", "independent-server", "provider"}
STAGES = {"preflight", "connect", "probe", "disconnect", "restore"}


class ValidationError(RuntimeError):
    """Expected, credential-free validation failure."""

    def __init__(self, code: str, stage: str = "preflight") -> None:
        super().__init__(code)
        self.code = code
        self.stage = stage


def _private_regular_file(path: Path, label: str) -> os.stat_result:
    try:
        info = path.lstat()
    except OSError as exc:
        raise ValidationError(f"{label}-unavailable") from exc
    if stat.S_ISLNK(info.st_mode) or not stat.S_ISREG(info.st_mode):
        raise ValidationError(f"{label}-not-private-regular-file")
    if info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) & 0o077:
        raise ValidationError(f"{label}-not-private-regular-file")
    return info


def _slug(value: Any, label: str) -> str:
    text = str(value)
    if not SLUG_RE.fullmatch(text):
        raise ValidationError(f"invalid-{label}")
    return text


def _https_probe_url(value: Any) -> str:
    parsed = urllib.parse.urlsplit(str(value))
    if (
        parsed.scheme != "https"
        or not parsed.hostname
        or parsed.username is not None
        or parsed.password is not None
        or parsed.port not in (None, 443)
        or parsed.query
        or parsed.fragment
    ):
        raise ValidationError("invalid-probe-url")
    return urllib.parse.urlunsplit(("https", parsed.netloc, parsed.path or "/", "", ""))


def load_cases(path: Path) -> dict[str, Any]:
    info = _private_regular_file(path, "cases-file")
    if info.st_size > MAX_CASES_BYTES:
        raise ValidationError("cases-file-too-large")
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise ValidationError("invalid-cases-json") from exc
    if not isinstance(payload, dict) or payload.get("schemaVersion") != 1:
        raise ValidationError("invalid-cases-schema")
    cases = payload.get("cases")
    if not isinstance(cases, list) or not cases or len(cases) > MAX_CASES:
        raise ValidationError("invalid-case-count")
    probe_url = _https_probe_url(payload.get("probeUrl", "https://example.com/"))
    seen: set[str] = set()
    normalized: list[dict[str, Any]] = []
    for raw in cases:
        if not isinstance(raw, dict):
            raise ValidationError("invalid-case")
        allowed = {
            "id", "profileId", "protocol", "features", "network", "sourceClass",
            "pairId", "expectSuccess", "expectedFailureStages",
        }
        if set(raw) - allowed:
            raise ValidationError("unknown-case-field")
        case_id = _slug(raw.get("id", ""), "case-id")
        if case_id in seen:
            raise ValidationError("duplicate-case-id")
        seen.add(case_id)
        profile_id = str(raw.get("profileId", ""))
        if not PROFILE_ID_RE.fullmatch(profile_id):
            raise ValidationError("invalid-profile-id")
        protocol = str(raw.get("protocol", ""))
        network = str(raw.get("network", ""))
        source_class = str(raw.get("sourceClass", ""))
        if protocol not in PROTOCOLS or network not in NETWORKS or source_class not in SOURCE_CLASSES:
            raise ValidationError("invalid-case-classification")
        features = raw.get("features", [])
        if (
            not isinstance(features, list)
            or len(features) > 16
            or any(not isinstance(item, str) for item in features)
        ):
            raise ValidationError("invalid-features")
        clean_features = [_slug(item, "feature") for item in features]
        expect_success = raw.get("expectSuccess", True)
        if not isinstance(expect_success, bool):
            raise ValidationError("invalid-expect-success")
        failure_stages = raw.get("expectedFailureStages", ["connect", "probe"])
        if (
            not isinstance(failure_stages, list)
            or not failure_stages
            or any(stage not in {"connect", "probe"} for stage in failure_stages)
        ):
            raise ValidationError("invalid-expected-failure-stages")
        pair_id = raw.get("pairId", "")
        if pair_id:
            pair_id = _slug(pair_id, "pair-id")
        normalized.append({
            "id": case_id,
            "profileId": profile_id,
            "protocol": protocol,
            "features": clean_features,
            "network": network,
            "sourceClass": source_class,
            "pairId": pair_id,
            "expectSuccess": expect_success,
            "expectedFailureStages": list(failure_stages),
        })
    return {"probeUrl": probe_url, "cases": normalized}


def run_process(argv: list[str], *, timeout: int = DEFAULT_TIMEOUT) -> subprocess.CompletedProcess[str]:
    try:
        result = subprocess.run(argv, text=True, capture_output=True, timeout=timeout, check=False)
    except subprocess.TimeoutExpired as exc:
        raise ValidationError("command-timeout") from exc
    if len(result.stdout.encode()) > MAX_BACKEND_OUTPUT or len(result.stderr.encode()) > MAX_BACKEND_OUTPUT:
        raise ValidationError("command-output-too-large")
    return result


class Backend:
    def __init__(self, path: Path, timeout: int) -> None:
        self.path = path
        self.timeout = timeout

    def call(self, *args: str, stage: str = "preflight") -> str:
        result = run_process([str(self.path), *args], timeout=self.timeout)
        if result.returncode != 0:
            raise ValidationError(f"backend-exit-{result.returncode}", stage)
        return result.stdout

    def status(self) -> dict[str, Any]:
        try:
            value = json.loads(self.call("status"))
        except json.JSONDecodeError as exc:
            raise ValidationError("invalid-public-status") from exc
        if not isinstance(value, dict):
            raise ValidationError("invalid-public-status")
        return value


def probe_https(url: str, timeout: int) -> None:
    request = urllib.request.Request(url, headers={"User-Agent": "OmaVLESS-V0/1"})
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            status_code = int(getattr(response, "status", 0))
            if not 200 <= status_code < 400:
                raise ValidationError("probe-http-status", "probe")
            response.read(MAX_PROBE_BODY + 1)
    except ValidationError:
        raise
    except Exception as exc:
        raise ValidationError("probe-failed", "probe") from exc


def _public_case(case: dict[str, Any]) -> dict[str, Any]:
    return {
        key: case[key]
        for key in ("id", "protocol", "features", "network", "sourceClass", "pairId")
    }


def evaluate_case(
    backend: Backend,
    case: dict[str, Any],
    probe_url: str,
    timeout: int,
    probe: Callable[[str, int], None] = probe_https,
) -> dict[str, Any]:
    started = time.monotonic()
    public = _public_case(case)
    stage = "preflight"
    try:
        status = backend.status()
        matches = [item for item in status.get("profiles", []) if item.get("id") == case["profileId"]]
        if len(matches) != 1 or matches[0].get("protocol") != case["protocol"]:
            raise ValidationError("profile-not-found-or-mismatched")
        backend.call("down-all", stage="preflight")
        backend.call("set-mode", "global", stage="preflight")
        stage = "connect"
        backend.call("connect", case["profileId"], stage=stage)
        connected = backend.status()
        if connected.get("activeId") != case["profileId"]:
            raise ValidationError("active-profile-mismatch", stage)
        stage = "probe"
        probe(probe_url, timeout)
        observed_success = True
        error_code = ""
        failure_stage = ""
    except ValidationError as exc:
        observed_success = False
        error_code = exc.code
        failure_stage = exc.stage or stage
    finally:
        with contextlib.suppress(Exception):
            backend.call("down-all", stage="disconnect")
    expected = (
        observed_success
        if case["expectSuccess"]
        else (not observed_success and failure_stage in case["expectedFailureStages"])
    )
    public.update({
        "expectedSuccess": case["expectSuccess"],
        "observedSuccess": observed_success,
        "accepted": expected,
        "failureStage": failure_stage,
        "errorCode": error_code,
        "durationMs": max(0, int((time.monotonic() - started) * 1000)),
    })
    return public


def core_version(path: str | None, timeout: int) -> str:
    selected = Path(path).expanduser() if path else None
    if selected is None:
        found = shutil.which("mihomo")
        selected = Path(found) if found else None
    if selected is None:
        return "unavailable"
    try:
        result = run_process([str(selected), "-v"], timeout=timeout)
    except (OSError, ValidationError):
        return "unavailable"
    if result.returncode != 0:
        return "unavailable"
    line = (result.stdout or result.stderr).splitlines()[0][:120] if (result.stdout or result.stderr) else ""
    return line if re.fullmatch(r"[A-Za-z0-9 ._+()/:-]{1,120}", line) else "available-redacted"


def atomic_private_json(path: Path, payload: dict[str, Any]) -> None:
    path = path.expanduser()
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    if path.exists() and (path.is_symlink() or not path.is_file() or path.stat().st_uid != os.getuid()):
        raise ValidationError("unsafe-output-path")
    fd, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        os.fchmod(fd, 0o600)
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            json.dump(payload, handle, ensure_ascii=False, indent=2)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        os.chmod(path, 0o600)
    finally:
        with contextlib.suppress(FileNotFoundError):
            os.unlink(temporary)


def restore_state(backend: Backend, previous: dict[str, Any]) -> str:
    try:
        backend.call("down-all", stage="restore")
        mode = str(previous.get("routing", {}).get("mode", "rule"))
        if mode not in {"rule", "global", "direct"}:
            raise ValidationError("invalid-previous-mode", "restore")
        backend.call("set-mode", mode, stage="restore")
        active_id = str(previous.get("activeId", ""))
        if active_id:
            backend.call("connect", active_id, stage="restore")
        return "restored"
    except ValidationError:
        return "manual-recovery-required"


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cases", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--backend", type=Path, default=Path(__file__).resolve().parents[1] / "backend.sh")
    parser.add_argument("--mihomo")
    parser.add_argument("--timeout", type=int, default=DEFAULT_TIMEOUT)
    parser.add_argument("--confirm-live", action="store_true")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if not args.confirm_live:
        raise ValidationError("confirm-live-required")
    if not 5 <= args.timeout <= 120:
        raise ValidationError("invalid-timeout")
    cases = load_cases(args.cases.expanduser())
    backend_path = args.backend.expanduser().resolve()
    if not backend_path.is_file() or not os.access(backend_path, os.X_OK):
        raise ValidationError("backend-unavailable")
    backend = Backend(backend_path, args.timeout)
    previous = backend.status()
    results: list[dict[str, Any]] = []
    restore = "not-attempted"
    try:
        for case in cases["cases"]:
            results.append(evaluate_case(backend, case, cases["probeUrl"], args.timeout))
    finally:
        restore = restore_state(backend, previous)
    payload = {
        "schemaVersion": 1,
        "generatedAt": dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat(),
        "coreVersion": core_version(args.mihomo, args.timeout),
        "probeHost": urllib.parse.urlsplit(cases["probeUrl"]).hostname,
        "restoreState": restore,
        "summary": {
            "cases": len(results),
            "accepted": sum(bool(item["accepted"]) for item in results),
            "unexpected": sum(not bool(item["accepted"]) for item in results),
        },
        "results": results,
    }
    atomic_private_json(args.output, payload)
    return 0 if payload["summary"]["unexpected"] == 0 and restore == "restored" else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ValidationError as exc:
        print(json.dumps({"ok": False, "errorCode": exc.code}, separators=(",", ":")))
        raise SystemExit(2)
