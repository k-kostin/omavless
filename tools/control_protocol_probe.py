#!/usr/bin/env python3
"""Credential-safe developer probe for OmaVLESS control protocol v1.

This tool only generates the fixed hello example or validates one bounded
request/response frame from standard input. It never connects to a socket,
dispatches a method, opens the private store, or touches the VPN runtime.
"""

from __future__ import annotations

import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import omavless_control_protocol as protocol  # noqa: E402


USAGE = "Usage: control_protocol_probe.py hello|request|response"
COMMANDS = frozenset({"hello", "request", "response"})


def _write_stdout(value: bytes) -> None:
    sys.stdout.buffer.write(value)
    sys.stdout.buffer.flush()


def main(argv: list[str]) -> int:
    if argv in (["-h"], ["--help"]):
        print(USAGE)
        return 0
    if len(argv) != 1 or argv[0] not in COMMANDS:
        print("Invalid probe command. Use --help.", file=sys.stderr)
        return 2

    command = argv[0]
    try:
        if command == "hello":
            request = protocol.make_request(
                "hello-1", "system.hello", {"versions": [protocol.API_VERSION]}
            )
            _write_stdout(protocol.encode_request(request))
            return 0

        response = command == "response"
        frame = protocol.read_unary_frame(sys.stdin.buffer, response=response)
        if response:
            protocol.decode_response(frame)
        else:
            protocol.decode_request(frame)
        _write_stdout(("VALID " + command + "\n").encode("ascii"))
        return 0
    except protocol.ProtocolError as error:
        # ProtocolError text comes exclusively from the stable safe catalog;
        # raw stdin is never included in the public result.
        print("INVALID " + command + ": " + str(error), file=sys.stderr)
        return 2
    except OSError:
        print("Probe I/O failed", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
