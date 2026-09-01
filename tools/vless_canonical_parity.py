#!/usr/bin/env python3
"""Credential-safe Python oracle adapter for canonical VLESS R2i parity."""

from __future__ import annotations

import hashlib
import ipaddress
import json
import re
import sys
import urllib.parse
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import backend  # noqa: E402
from tools.vless_transport_parity import error_code as inherited_error_code  # noqa: E402
from tools.vless_xhttp_download_parity import error_code as download_error_code  # noqa: E402
from tools.vless_xhttp_options_parity import error_code as options_error_code  # noqa: E402

MAX_INPUT_BYTES = 1024 * 1024
MAX_CASES = 512
SAFE_ID = re.compile(r"[A-Za-z0-9_.-]{1,96}\Z")


def canonical_fingerprint(value: Any) -> str:
    payload = json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


def empty_outcome(classification: str, *, accepted: bool = False) -> dict[str, Any]:
    return {
        "accepted": accepted,
        "classification": classification,
        "host_kind": "none",
        "port": 0,
        "standard_https_port": False,
        "transport": "none",
        "security": "none",
        "flow": "none",
        "packet_encoding": "none",
        "xhttp_mode": "none",
        "allow_insecure": False,
        "encryption_enabled": False,
        "reality_pq": False,
        "alpn_count": 0,
        "advanced_xhttp": False,
        "xhttp_field_count": 0,
        "experimental_feature_count": 0,
        "compatibility_note_present": False,
        "compatibility_spider": False,
        "compatibility_pq": False,
        "compatibility_provider_metadata": False,
        "compatibility_transport_metadata": False,
        "identity_fingerprint": "",
        "preview_fingerprint": "",
        "mihomo_fingerprint": "",
    }


def has_download_reality(uri: str) -> bool:
    try:
        link = backend.extract_vless_uri(uri)
        query = backend.parse_vless_query(urllib.parse.urlsplit(link).query)
        raw = backend.first_query(query, "extra")
        data = backend.decode_xhttp_extra(raw)
        download = data.get("downloadSettings")
        return isinstance(download, dict) and download.get("realitySettings") is not None
    except (backend.BackendError, UnicodeError, ValueError):
        return False


def classify_error(message: str, uri: str) -> str:
    # The production oracle reuses the top-level Reality validators inside
    # downloadSettings. Resolve those two text-identical failures by the
    # validated location rather than changing either accepted error catalog.
    if has_download_reality(uri):
        if message == "VLESS Reality public key has an invalid format":
            return "reality_public_key"
        if message == "VLESS Reality short ID has an invalid format":
            return "reality_short_id"
    inherited = inherited_error_code(message)
    if inherited != "outside_slice":
        return inherited
    if message == "VLESS XHTTP extra requires the XHTTP transport":
        return "xhttp_extra_requires_transport"
    options = options_error_code(message)
    if options not in {"internal_error", "download_settings_outside_slice"}:
        return options
    download = download_error_code(message)
    if download != "internal_error":
        return download
    return "internal_error"


def host_kind(value: str) -> str:
    try:
        parsed = ipaddress.ip_address(value)
    except ValueError:
        return "dns"
    return "ipv4" if parsed.version == 4 else "ipv6"


def mihomo_semantics(
    node: dict[str, Any], name: str, server_override: str | None
) -> dict[str, Any]:
    """Return the schema-validated proxy mapping independent of YAML spelling."""
    values: dict[str, Any] = {
        "name": name,
        "type": "vless",
        "server": server_override or node["server"],
        "port": int(node["port"]),
        "uuid": str(node["uuid"]),
        "udp": True,
        "network": str(node["network"]),
        "encryption": str(node["encryption"]),
    }
    if node["mihomo_flow"]:
        values["flow"] = str(node["mihomo_flow"])
    if node["packet_encoding"]:
        values["packet-encoding"] = str(node["packet_encoding"])
    if node["security"] in {"tls", "reality"}:
        values["tls"] = True
        if node["servername"]:
            values["servername"] = str(node["servername"])
        if node["fingerprint"]:
            values["client-fingerprint"] = str(node["fingerprint"])
        if node["alpn"]:
            values["alpn"] = list(node["alpn"])
        if node["allow_insecure"]:
            values["skip-cert-verify"] = True
    if node["security"] == "reality":
        reality: dict[str, Any] = {"public-key": str(node["public_key"])}
        if node["short_id"]:
            reality["short-id"] = str(node["short_id"])
        if node["support_x25519mlkem768"]:
            reality["support-x25519mlkem768"] = True
        values["reality-opts"] = reality
    if node["network"] == "ws":
        options: dict[str, Any] = {"path": str(node["path"])}
        if node["host"]:
            options["headers"] = {"Host": str(node["host"])}
        values["ws-opts"] = options
    elif node["network"] == "grpc":
        values["grpc-opts"] = {
            "grpc-service-name": str(node["service_name"])
        }
    elif node["network"] == "h2":
        options = {}
        if node["host"]:
            options["host"] = [str(node["host"])]
        options["path"] = str(node["path"])
        values["h2-opts"] = options
    elif node["network"] == "http":
        options = {"path": [str(node["path"])]}
        if node["host"]:
            options["headers"] = {"Host": [str(node["host"])]}
        values["http-opts"] = options
    elif node["network"] == "xhttp":
        options = {"path": str(node["path"])}
        if node["host"]:
            options["host"] = str(node["host"])
        if node["mode"]:
            options["mode"] = str(node["mode"])
        options.update(node["xhttp_extra"])
        values["xhttp-opts"] = options
    return values


def accepted_outcome(
    uri: str, node: dict[str, Any], name: str, server_override: str | None
) -> dict[str, Any]:
    outcome = empty_outcome("accepted", accepted=True)
    outcome.update(
        {
            "host_kind": host_kind(str(node["server"])),
            "port": int(node["port"]),
            "standard_https_port": int(node["port"]) == 443,
            "transport": str(node["network"]),
            "security": str(node["security"]),
            "flow": str(node["flow"]) or "none",
            "packet_encoding": str(node["packet_encoding"]) or "none",
            "xhttp_mode": (
                str(node["mode"]) or "default"
                if node["network"] == "xhttp"
                else "none"
            ),
            "allow_insecure": bool(node["allow_insecure"]),
            "encryption_enabled": str(node["encryption"]) != "none",
            "reality_pq": bool(node["support_x25519mlkem768"]),
            "alpn_count": len(node["alpn"]),
            "advanced_xhttp": bool(node["xhttp_extra"]),
            "xhttp_field_count": len(node["xhttp_extra"]),
            "experimental_feature_count": len(node["experimental_features"]),
            "compatibility_note_present": bool(node["compatibility_note"]),
            "compatibility_spider": (
                backend.REALITY_SPX_COMPATIBILITY_NOTE
                in str(node["compatibility_note"])
            ),
            "compatibility_pq": (
                backend.REALITY_PQ_COMPATIBILITY_NOTE
                in str(node["compatibility_note"])
            ),
            "compatibility_provider_metadata": (
                backend.VLESS_PROVIDER_METADATA_COMPATIBILITY_NOTE
                in str(node["compatibility_note"])
            ),
            "compatibility_transport_metadata": (
                backend.VLESS_TRANSPORT_METADATA_COMPATIBILITY_NOTE
                in str(node["compatibility_note"])
            ),
            "identity_fingerprint": backend._vless_subscription_key(uri),
            "preview_fingerprint": canonical_fingerprint(backend.preview_vless(uri)),
            "mihomo_fingerprint": canonical_fingerprint(
                mihomo_semantics(node, name, server_override)
            ),
        }
    )
    return outcome


def validate_case(value: Any) -> tuple[str, str, str, str | None]:
    if not isinstance(value, dict) or set(value) != {
        "id",
        "uri",
        "name",
        "server_override",
    }:
        raise ValueError("invalid case envelope")
    case_id = value["id"]
    uri = value["uri"]
    name = value["name"]
    server_override = value["server_override"]
    if not isinstance(case_id, str) or not SAFE_ID.fullmatch(case_id):
        raise ValueError("invalid case id")
    if not isinstance(uri, str) or not isinstance(name, str):
        raise ValueError("invalid case value")
    if server_override is not None and not isinstance(server_override, str):
        raise ValueError("invalid server override")
    if len(uri.encode("utf-8")) > backend.MAX_IMPORT_BYTES:
        raise ValueError("oversized case")
    if len(name.encode("utf-8")) > 256:
        raise ValueError("oversized name")
    if server_override is not None and len(server_override.encode("utf-8")) > 253:
        raise ValueError("oversized server override")
    return case_id, uri, name, server_override


def main(argv: list[str]) -> int:
    if argv:
        return 2
    raw = sys.stdin.buffer.read(MAX_INPUT_BYTES + 1)
    if len(raw) > MAX_INPUT_BYTES:
        return 1
    try:
        envelope = json.loads(raw.decode("utf-8", errors="strict"))
        if not isinstance(envelope, dict) or set(envelope) != {"cases"}:
            raise ValueError("invalid envelope")
        cases = envelope["cases"]
        if not isinstance(cases, list) or len(cases) > MAX_CASES:
            raise ValueError("invalid cases")
        results: list[dict[str, Any]] = []
        for value in cases:
            case_id, uri, name, server_override = validate_case(value)
            try:
                node = backend.parse_vless(uri)
                outcome = accepted_outcome(uri, node, name, server_override)
            except backend.BackendError as error:
                outcome = empty_outcome(classify_error(str(error), uri))
            results.append({"id": case_id, **outcome})
        sys.stdout.write(
            json.dumps({"cases": results}, sort_keys=True, separators=(",", ":"))
        )
        return 0
    except (UnicodeError, RecursionError, TypeError, ValueError, json.JSONDecodeError):
        return 1
    except (MemoryError, OSError):
        return 1
    except Exception:  # noqa: BLE001 - unexpected details must never escape
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
