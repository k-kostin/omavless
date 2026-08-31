#!/usr/bin/env python3
"""Credential-safe Python oracle adapter for VLESS transport-option parity."""

from __future__ import annotations

import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import backend  # noqa: E402
from tools.vless_encryption_parity import error_code as inherited_error_code  # noqa: E402


def emit(
    *,
    accepted: bool,
    classification: str,
    path_default: bool = False,
    path_starts_with_slash: bool = False,
    path_non_ascii: bool = False,
    host_present: bool = False,
    host_non_ascii: bool = False,
    service_name_present: bool = False,
    service_name_non_ascii: bool = False,
    fingerprint_present: bool = False,
    fingerprint_non_ascii: bool = False,
    alpn_count: int = 0,
    alpn_non_ascii: bool = False,
) -> None:
    print(json.dumps({
        "accepted": accepted,
        "classification": classification,
        "path_default": path_default,
        "path_starts_with_slash": path_starts_with_slash,
        "path_non_ascii": path_non_ascii,
        "host_present": host_present,
        "host_non_ascii": host_non_ascii,
        "service_name_present": service_name_present,
        "service_name_non_ascii": service_name_non_ascii,
        "fingerprint_present": fingerprint_present,
        "fingerprint_non_ascii": fingerprint_non_ascii,
        "alpn_count": alpn_count,
        "alpn_non_ascii": alpn_non_ascii,
    }, sort_keys=True, separators=(",", ":")))


def error_code(message: str) -> str:
    inherited = inherited_error_code(message)
    if inherited != "outside_slice":
        return inherited
    if message.startswith("Unsupported VLESS TCP header type:"):
        return "unsupported_tcp_header"
    return "outside_slice"


def summarize(node: dict[str, object]) -> dict[str, object]:
    path = str(node["path"])
    host = str(node["host"])
    service_name = str(node["service_name"])
    fingerprint = str(node["fingerprint"])
    alpn = [str(value) for value in node["alpn"]]  # type: ignore[union-attr]
    return {
        "path_default": path == "/",
        "path_starts_with_slash": path.startswith("/"),
        "path_non_ascii": not path.isascii(),
        "host_present": bool(host),
        "host_non_ascii": not host.isascii(),
        "service_name_present": bool(service_name),
        "service_name_non_ascii": not service_name.isascii(),
        "fingerprint_present": bool(fingerprint),
        "fingerprint_non_ascii": not fingerprint.isascii(),
        "alpn_count": len(alpn),
        "alpn_non_ascii": any(not value.isascii() for value in alpn),
    }


def main(argv: list[str]) -> int:
    if argv:
        print("Usage: vless_transport_parity.py", file=sys.stderr)
        return 2
    try:
        raw = sys.stdin.buffer.read(backend.MAX_IMPORT_BYTES + 1)
        if len(raw) > backend.MAX_IMPORT_BYTES:
            emit(accepted=False, classification="invalid_input")
            return 0
        text = raw.decode("utf-8", errors="strict")
        node = backend.parse_vless(text)
        emit(accepted=True, classification="accepted", **summarize(node))
        return 0
    except UnicodeDecodeError:
        emit(accepted=False, classification="invalid_input")
        return 0
    except backend.BackendError as error:
        emit(accepted=False, classification=error_code(str(error)))
        return 0
    except OSError:
        emit(accepted=False, classification="internal_error")
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
