#!/usr/bin/env python3
"""Credential-safe Python oracle adapter for VLESS REALITY/PQ parity."""

from __future__ import annotations

import json
import sys
import urllib.parse
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import backend  # noqa: E402
from tools.vless_flow_packet_parity import error_code as inherited_error_code  # noqa: E402


PQ_FIELDS = {"supportx25519mlkem768", "support-x25519mlkem768"}


def emit(
    *,
    accepted: bool,
    classification: str,
    reality: bool = False,
    pq: bool = False,
    pq_present: bool = False,
    short_id_present: bool = False,
    spider_x_present: bool = False,
) -> None:
    print(json.dumps({
        "accepted": accepted,
        "classification": classification,
        "reality": reality,
        "pq": pq,
        "pq_present": pq_present,
        "short_id_present": short_id_present,
        "spider_x_present": spider_x_present,
    }, sort_keys=True, separators=(",", ":")))


def error_code(message: str) -> str:
    inherited = inherited_error_code(message)
    if inherited != "outside_slice":
        return inherited
    exact = {
        "Reality link requires both pbk/public-key and sni/servername":
            "reality_fields_required",
        "VLESS Reality post-quantum flag must be true or false":
            "invalid_reality_pq_boolean",
        "VLESS Reality post-quantum flag requires Reality security":
            "reality_pq_requires_reality",
        "VLESS Reality ML-DSA verification is not supported by Mihomo":
            "reality_mldsa_unsupported",
        "VLESS Reality public key has an invalid format":
            "invalid_reality_public_key",
        "VLESS Reality short ID has an invalid format":
            "invalid_reality_short_id",
    }
    return exact.get(message, "outside_slice")


def main(argv: list[str]) -> int:
    if argv:
        print("Usage: vless_reality_parity.py", file=sys.stderr)
        return 2
    try:
        raw = sys.stdin.buffer.read(backend.MAX_IMPORT_BYTES + 1)
        if len(raw) > backend.MAX_IMPORT_BYTES:
            emit(accepted=False, classification="invalid_input")
            return 0
        text = raw.decode("utf-8", errors="strict")
        uri = backend.extract_vless_uri(text)
        query = backend.parse_vless_query(urllib.parse.urlsplit(uri).query)
        node = backend.parse_vless(text)
        emit(
            accepted=True,
            classification="accepted",
            reality=node["security"] == "reality",
            pq=bool(node["support_x25519mlkem768"]),
            pq_present=bool(PQ_FIELDS & set(query)),
            short_id_present=bool(node["short_id"]),
            spider_x_present=bool(node["spider_x"]),
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
