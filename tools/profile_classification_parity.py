#!/usr/bin/env python3
"""Sanitized Python oracle adapter for R2 profile classification parity."""

from __future__ import annotations

import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import backend  # noqa: E402


ERROR_CODES = {
    "That profile protocol is not supported": "unsupported_protocol",
    "Input does not contain a supported profile link": "missing_profile_link",
}


def emit(*, accepted: bool, classification: str, protocol: str = "none") -> None:
    print(json.dumps({
        "accepted": accepted,
        "classification": classification,
        "protocol": protocol,
    }, sort_keys=True, separators=(",", ":")))


def main(argv: list[str]) -> int:
    if argv:
        print("Usage: profile_classification_parity.py", file=sys.stderr)
        return 2
    try:
        raw = sys.stdin.buffer.read(backend.MAX_IMPORT_BYTES + 1)
        if len(raw) > backend.MAX_IMPORT_BYTES:
            emit(accepted=False, classification="invalid_input")
            return 0
        text = raw.decode("utf-8", errors="strict")
        emit(
            accepted=True,
            classification="accepted",
            protocol=backend.profile_protocol(text),
        )
        return 0
    except UnicodeDecodeError:
        emit(accepted=False, classification="invalid_input")
        return 0
    except backend.BackendError as error:
        emit(
            accepted=False,
            classification=ERROR_CODES.get(str(error), "invalid_input"),
        )
        return 0
    except OSError:
        emit(accepted=False, classification="internal_error")
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
