#!/usr/bin/env python3
"""Credential-safe Python oracle adapter for R2 VLESS query metadata parity."""

from __future__ import annotations

import json
import sys
import urllib.parse
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import backend  # noqa: E402


EXACT_ERROR_CODES = {
    "Input does not contain a VLESS link": "missing_vless_link",
    "Invalid VLESS link": "invalid_link",
    "VLESS link must not contain a password field": "password_not_allowed",
    "VLESS user id is not a valid UUID": "invalid_user_id",
    "VLESS server and port are required": "missing_server_port",
    "Invalid VLESS query": "invalid_query",
    "VLESS link contains duplicate fields": "duplicate_fields",
    "VLESS link contains unsupported fields": "unsupported_fields",
    "VLESS provider metadata has an invalid format": "invalid_provider_metadata",
    "VLESS link contains conflicting field aliases": "conflicting_aliases",
    "VLESS boolean query field must be true or false": "invalid_boolean",
}


def emit(
    *,
    accepted: bool,
    classification: str,
    transport: str = "none",
    security: str = "none",
    allow_insecure: bool = False,
    xhttp_mode: str = "none",
    provider_metadata_present: bool = False,
    non_xhttp_mode_metadata: bool = False,
) -> None:
    print(json.dumps({
        "accepted": accepted,
        "classification": classification,
        "transport": transport,
        "security": security,
        "allow_insecure": allow_insecure,
        "xhttp_mode": xhttp_mode,
        "provider_metadata_present": provider_metadata_present,
        "non_xhttp_mode_metadata": non_xhttp_mode_metadata,
    }, sort_keys=True, separators=(",", ":")))


def error_code(message: str) -> str:
    if message in EXACT_ERROR_CODES:
        return EXACT_ERROR_CODES[message]
    if message.startswith("VLESS link is too large"):
        return "invalid_input"
    if message.startswith("Unsupported VLESS transport:"):
        return "unsupported_transport"
    if message.startswith("Unsupported VLESS security:"):
        return "unsupported_security"
    if message.startswith("Unsupported VLESS XHTTP mode:"):
        return "unsupported_xhttp_mode"
    return "outside_slice"


def main(argv: list[str]) -> int:
    if argv:
        print("Usage: vless_query_metadata_parity.py", file=sys.stderr)
        return 2
    try:
        raw = sys.stdin.buffer.read(backend.MAX_IMPORT_BYTES + 1)
        if len(raw) > backend.MAX_IMPORT_BYTES:
            emit(accepted=False, classification="invalid_input")
            return 0
        text = raw.decode("utf-8", errors="strict")
        uri = backend.extract_vless_uri(text)
        parsed = urllib.parse.urlsplit(uri)
        query = backend.parse_vless_query(parsed.query)
        node = backend.parse_vless(text)
        transport = str(node["network"])
        raw_mode = backend.first_query(query, "mode")
        emit(
            accepted=True,
            classification="accepted",
            transport=transport,
            security=str(node["security"]),
            allow_insecure=bool(node["allow_insecure"]),
            xhttp_mode=(
                str(node["mode"]) or "default"
                if transport == "xhttp" else "none"
            ),
            provider_metadata_present=bool(
                backend.VLESS_PROVIDER_METADATA_FIELDS & set(query)
            ),
            non_xhttp_mode_metadata=bool(raw_mode and transport != "xhttp"),
        )
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
