#!/usr/bin/env python3
"""Credential-safe Python oracle adapter for R2 VLESS authority parity."""

from __future__ import annotations

import ipaddress
import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import backend  # noqa: E402


ERROR_CODES = {
    "Input does not contain a VLESS link": "missing_vless_link",
    "Invalid VLESS link": "invalid_link",
    "VLESS link must not contain a password field": "password_not_allowed",
    "VLESS user id is not a valid UUID": "invalid_user_id",
    "VLESS server and port are required": "missing_server_port",
}


def emit(
    *,
    accepted: bool,
    classification: str,
    host_kind: str = "none",
    standard_https_port: bool = False,
    label_kind: str = "none",
    label_sanitized: bool = False,
    label_truncated: bool = False,
) -> None:
    print(json.dumps({
        "accepted": accepted,
        "classification": classification,
        "host_kind": host_kind,
        "standard_https_port": standard_https_port,
        "label_kind": label_kind,
        "label_sanitized": label_sanitized,
        "label_truncated": label_truncated,
    }, sort_keys=True, separators=(",", ":")))


def host_kind(value: str) -> str:
    try:
        parsed = ipaddress.ip_address(value)
    except ValueError:
        return "dns"
    return "ipv4" if parsed.version == 4 else "ipv6"


def main(argv: list[str]) -> int:
    if argv:
        print("Usage: vless_authority_parity.py", file=sys.stderr)
        return 2
    try:
        raw = sys.stdin.buffer.read(backend.MAX_IMPORT_BYTES + 1)
        if len(raw) > backend.MAX_IMPORT_BYTES:
            emit(accepted=False, classification="invalid_input")
            return 0
        text = raw.decode("utf-8", errors="strict")
        node = backend.parse_vless(text)
        raw_label = str(node["suggested_name"])
        sanitized = re.sub(r"[\x00-\x1f\x7f]", "", raw_label).strip()
        visible = sanitized[:80]
        emit(
            accepted=True,
            classification="accepted",
            host_kind=host_kind(str(node["server"])),
            standard_https_port=int(node["port"]) == 443,
            label_kind=(
                "none" if not visible else "ascii" if visible.isascii() else "unicode"
            ),
            label_sanitized=sanitized != raw_label,
            label_truncated=len(sanitized) > 80,
        )
        return 0
    except UnicodeDecodeError:
        emit(accepted=False, classification="invalid_input")
        return 0
    except backend.BackendError as error:
        message = str(error)
        classification = ERROR_CODES.get(message)
        if classification is None and message.startswith("VLESS link is too large"):
            classification = "invalid_input"
        emit(accepted=False, classification=classification or "invalid_link")
        return 0
    except OSError:
        emit(accepted=False, classification="internal_error")
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
