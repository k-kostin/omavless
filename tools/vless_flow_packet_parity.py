#!/usr/bin/env python3
"""Credential-safe Python oracle adapter for VLESS flow/packet parity."""

from __future__ import annotations

import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import backend  # noqa: E402
from tools.vless_query_metadata_parity import error_code as query_error_code  # noqa: E402


def emit(
    *,
    accepted: bool,
    classification: str,
    flow: str = "none",
    mihomo_flow: str = "none",
    packet_encoding: str = "none",
) -> None:
    print(json.dumps({
        "accepted": accepted,
        "classification": classification,
        "flow": flow,
        "mihomo_flow": mihomo_flow,
        "packet_encoding": packet_encoding,
    }, sort_keys=True, separators=(",", ":")))


def error_code(message: str) -> str:
    inherited = query_error_code(message)
    if inherited != "outside_slice":
        return inherited
    if message.startswith("Unsupported VLESS flow:"):
        return "unsupported_flow"
    if message == "VLESS Vision flow requires the TCP transport":
        return "vision_requires_tcp"
    if message == "VLESS Vision flow requires TLS or Reality security":
        return "vision_requires_security"
    if message.startswith("Unsupported VLESS packet encoding:"):
        return "unsupported_packet_encoding"
    return "outside_slice"


def main(argv: list[str]) -> int:
    if argv:
        print("Usage: vless_flow_packet_parity.py", file=sys.stderr)
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
            flow=str(node["flow"]) or "none",
            mihomo_flow=str(node["mihomo_flow"]) or "none",
            packet_encoding=str(node["packet_encoding"]) or "none",
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
