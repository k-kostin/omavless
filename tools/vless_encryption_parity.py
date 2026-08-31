#!/usr/bin/env python3
"""Credential-safe Python oracle adapter for VLESS Encryption parity."""

from __future__ import annotations

import base64
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import backend  # noqa: E402
from tools.vless_reality_parity import error_code as inherited_error_code  # noqa: E402


def emit(
    *,
    accepted: bool,
    classification: str,
    enabled: bool = False,
    mode: str = "none",
    rtt: str = "none",
    key_count: int = 0,
    large_key_present: bool = False,
    padding_count: int = 0,
    total_padding: int = 0,
) -> None:
    print(json.dumps({
        "accepted": accepted,
        "classification": classification,
        "enabled": enabled,
        "mode": mode,
        "rtt": rtt,
        "key_count": key_count,
        "large_key_present": large_key_present,
        "padding_count": padding_count,
        "total_padding": total_padding,
    }, sort_keys=True, separators=(",", ":")))


def error_code(message: str) -> str:
    inherited = inherited_error_code(message)
    if inherited != "outside_slice":
        return inherited
    exact = {
        "VLESS Encryption value is too large": "encryption_too_large",
        "VLESS Encryption value has an invalid format": "invalid_encryption_format",
        "VLESS Encryption value is not supported by Mihomo": "unsupported_encryption",
        "VLESS Encryption padding has an invalid format": "invalid_encryption_padding",
        "VLESS Encryption padding is outside the supported range":
            "encryption_padding_range",
        "VLESS Encryption first padding range is too small":
            "encryption_first_padding_too_small",
        "VLESS Encryption key has an invalid format": "invalid_encryption_key",
        "VLESS Encryption requires at least one client key": "encryption_key_required",
        "VLESS Encryption total padding is too large":
            "encryption_total_padding_too_large",
    }
    return exact.get(message, "outside_slice")


def summarize(value: str) -> dict[str, object]:
    if value == "none":
        return {}
    parts = value.split(".")
    key_count = 0
    large_key_present = False
    padding_count = 0
    total_padding = 0
    for token in parts[3:]:
        if len(token) < 20:
            _probability, _minimum, maximum = map(int, token.split("-"))
            if padding_count % 2 == 0:
                total_padding += maximum
            padding_count += 1
        else:
            decoded = base64.b64decode(
                token + "=" * (-len(token) % 4), altchars=b"-_", validate=True
            )
            key_count += 1
            large_key_present = large_key_present or len(decoded) == 1184
    return {
        "enabled": True,
        "mode": parts[1],
        "rtt": parts[2],
        "key_count": key_count,
        "large_key_present": large_key_present,
        "padding_count": padding_count,
        "total_padding": total_padding,
    }


def main(argv: list[str]) -> int:
    if argv:
        print("Usage: vless_encryption_parity.py", file=sys.stderr)
        return 2
    try:
        raw = sys.stdin.buffer.read(backend.MAX_IMPORT_BYTES + 1)
        if len(raw) > backend.MAX_IMPORT_BYTES:
            emit(accepted=False, classification="invalid_input")
            return 0
        text = raw.decode("utf-8", errors="strict")
        node = backend.parse_vless(text)
        emit(
            accepted=True,
            classification="accepted",
            **summarize(str(node["encryption"])),
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
