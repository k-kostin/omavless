#!/usr/bin/env python3
"""Test-only sanitized classifier for Python/Rust protocol parity.

The adapter accepts exactly one bounded request or response frame on stdin and
returns only an acceptance boolean plus a stable public classification. It
never returns the frame, decoded value, parser fragment, or caller-controlled
error text and does not touch the VPN runtime or private store.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import omavless_control_protocol as protocol  # noqa: E402


USAGE = "Usage: control_protocol_parity.py request|response"


def _emit(*, accepted: bool, classification: str) -> None:
    output = {"accepted": accepted, "classification": classification}
    print(json.dumps(output, sort_keys=True, separators=(",", ":")))


def main(argv: list[str]) -> int:
    if len(argv) != 1 or argv[0] not in {"request", "response"}:
        print(USAGE, file=sys.stderr)
        return 2
    response = argv[0] == "response"
    limit = (
        protocol.MAX_RESPONSE_FRAME_BYTES
        if response
        else protocol.MAX_REQUEST_FRAME_BYTES
    )
    try:
        frame = sys.stdin.buffer.read(limit + 1)
        if response:
            protocol.decode_response(frame)
        else:
            protocol.decode_request(frame)
        _emit(accepted=True, classification="accepted")
        return 0
    except protocol.ProtocolError as error:
        _emit(accepted=False, classification=error.code)
        return 0
    except OSError:
        _emit(accepted=False, classification="internal_error")
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
