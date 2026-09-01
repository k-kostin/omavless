#!/usr/bin/env python3
"""Credential-safe Python oracle for canonical non-VLESS R2 parity."""

from __future__ import annotations

import hashlib
import ipaddress
import json
import re
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import backend  # noqa: E402

MAX_INPUT_BYTES = 1024 * 1024
MAX_CASES = 256
SAFE_ID = re.compile(r"[A-Za-z0-9_.-]{1,96}\Z")


def fingerprint(value: Any) -> str:
    return hashlib.sha256(json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode()).hexdigest()


def empty(classification: str) -> dict[str, Any]:
    return {
        "accepted": False, "classification": classification,
        "protocol": "none", "host_kind": "none", "port": 0,
        "transport": "none", "security": "none", "allow_insecure": False,
        "udp": False, "alpn_count": 0, "port_hopping": False,
        "obfuscation": False, "fingerprint_present": False,
        "ech_present": False, "reality_pq": False, "disable_sni": False,
        "congestion": "none", "udp_relay": "none",
        "compatibility_note_present": False, "identity_fingerprint": "",
        "preview_fingerprint": "", "mihomo_fingerprint": "",
    }


def host_kind(value: str) -> str:
    try:
        address = ipaddress.ip_address(value)
    except ValueError:
        return "dns"
    return "ipv4" if address.version == 4 else "ipv6"


def mihomo(node: dict[str, Any], protocol: str, name: str,
           server_override: str | None) -> dict[str, Any]:
    result: dict[str, Any] = {
        "name": name, "type": protocol,
        "server": server_override or node["server"], "port": node["port"],
    }
    if protocol == "trojan":
        result.update(password=node["password"], udp=node["udp"], network=node["network"])
        if node["servername"]: result["sni"] = node["servername"]
        if node["alpn"]: result["alpn"] = node["alpn"]
        if node["fingerprint"]: result["client-fingerprint"] = node["fingerprint"]
        if node["allow_insecure"]: result["skip-cert-verify"] = True
        if node["security"] == "reality":
            reality = {"public-key": node["public_key"]}
            if node["short_id"]: reality["short-id"] = node["short_id"]
            if node["support_x25519mlkem768"]: reality["support-x25519mlkem768"] = True
            result["reality-opts"] = reality
        if node["network"] == "ws":
            options: dict[str, Any] = {"path": node["path"]}
            if node["host"]: options["headers"] = {"Host": node["host"]}
            result["ws-opts"] = options
        elif node["network"] == "grpc":
            result["grpc-opts"] = {"grpc-service-name": node["service_name"]}
    elif protocol == "hysteria2":
        result["password"] = node["password"]
        if node["port_hopping"]: result["ports"] = node["ports"]
        if node["obfs"]:
            result["obfs"] = node["obfs"]
            result["obfs-password"] = node["obfs_password"]
        if node["servername"]: result["sni"] = node["servername"]
        if node["allow_insecure"]: result["skip-cert-verify"] = True
        if node["fingerprint"]: result["fingerprint"] = node["fingerprint"]
        if node["ech"]: result["ech-opts"] = {"enable": True, "config": node["ech"]}
    else:
        result.update(
            uuid=node["uuid"], password=node["password"],
            **{"udp-relay-mode": node["udp_relay_mode"],
               "congestion-controller": node["congestion_controller"]},
        )
        if node["alpn"]: result["alpn"] = node["alpn"]
        if node["servername"]: result["sni"] = node["servername"]
        if node["allow_insecure"]: result["skip-cert-verify"] = True
        if node["disable_sni"]: result["disable-sni"] = True
    return result


def accepted(protocol: str, uri: str, node: dict[str, Any], name: str,
             server_override: str | None) -> dict[str, Any]:
    result = empty("accepted")
    result.update(
        accepted=True, protocol=protocol, host_kind=host_kind(node["server"]),
        port=node["port"], transport=node.get("network", "quic"),
        security=node.get("security", "tls"),
        allow_insecure=bool(node["allow_insecure"]),
        udp=bool(node.get("udp", False)), alpn_count=len(node.get("alpn", [])),
        port_hopping=bool(node.get("port_hopping", False)),
        obfuscation=bool(node.get("obfs", "")),
        fingerprint_present=bool(node.get("fingerprint", "")),
        ech_present=bool(node.get("ech", "")),
        reality_pq=bool(node.get("support_x25519mlkem768", False)),
        disable_sni=bool(node.get("disable_sni", False)),
        congestion=str(node.get("congestion_controller", "none")),
        udp_relay=str(node.get("udp_relay_mode", "none")),
        compatibility_note_present=bool(node["compatibility_note"]),
        identity_fingerprint=backend.profile_subscription_key(uri),
        preview_fingerprint=fingerprint(backend.preview_profile(uri)),
        mihomo_fingerprint=fingerprint(mihomo(node, protocol, name, server_override)),
    )
    return result


def classify(protocol: str, message: str) -> str:
    if message == "Input does not contain a supported profile link":
        return {
            "trojan": "missing_trojan_link",
            "hysteria2": "missing_hysteria2_link",
            "tuic": "missing_tuic_link",
        }[protocol]
    exact = {
        "Input does not contain a Trojan link": "missing_trojan_link",
        "Input does not contain a Hysteria2 link": "missing_hysteria2_link",
        "Input does not contain a TUIC link": "missing_tuic_link",
    }
    if message in exact: return exact[message]
    if "link is too large" in message: return "invalid_input"
    tables = {
        "trojan": [
            ("invalid authority", "invalid_authority"), ("password", "invalid_password"),
            ("server and port", "missing_server_port"), ("duplicate", "duplicate_field"),
            ("unsupported fields", "unsupported_field"), ("conflicting field aliases", "alias_conflict"),
            ("transport is not supported", "unsupported_transport"), ("security must", "unsupported_security"),
            ("ALPN", "invalid_alpn"), ("fingerprint", "unsupported_fingerprint"),
            ("true or false", "invalid_boolean"), ("encryption metadata", "unsupported_encryption"),
            ("flow metadata", "unsupported_flow"), ("TCP header", "unsupported_header"),
            ("transport-only", "transport_field_conflict"), ("gRPC-only", "transport_field_conflict"),
            ("unsupported transport fields", "transport_field_conflict"),
            ("not supported with WebSocket", "reality_websocket"),
            ("requires both", "reality_required"), ("fields require Reality", "reality_fields_require_reality"),
            ("public key", "reality_public_key"), ("short ID", "reality_short_id"),
            ("ML-DSA", "reality_mldsa"), ("Invalid Trojan query", "invalid_query"),
            ("Invalid Trojan link", "invalid_link"), ("invalid format", "invalid_text"),
        ],
        "hysteria2": [
            ("path is not supported", "unsupported_path"), ("invalid authority", "invalid_authority"),
            ("authentication", "invalid_authentication"), ("server", "invalid_host"),
            ("overlapping", "overlapping_ports"), ("invalid range", "invalid_port_range"),
            ("port list", "invalid_port_list"), ("duplicate", "duplicate_field"),
            ("local setting", "local_bandwidth"), ("unsupported fields", "unsupported_field"),
            ("obfuscation type", "unsupported_obfuscation"), ("requires a password", "missing_obfuscation_password"),
            ("requires an obfs", "missing_obfuscation_type"), ("insecure", "invalid_insecure"),
            ("SHA-256", "invalid_fingerprint"), ("ECH config", "invalid_ech"),
            ("Invalid Hysteria2 query", "invalid_query"), ("Invalid Hysteria2 link", "invalid_link"),
        ],
        "tuic": [
            ("invalid authority", "invalid_authority"), ("credential", "invalid_credential"),
            ("requires a UUID and password", "missing_credential"), ("valid UUID", "invalid_uuid"),
            ("password", "invalid_password"), ("server and port", "missing_server_port"),
            ("duplicate", "duplicate_field"), ("unsupported fields", "unsupported_field"),
            ("congestion controller", "unsupported_congestion"), ("UDP relay mode", "unsupported_udp_relay"),
            ("ALPN", "invalid_alpn"), ("must be 0 or 1", "invalid_boolean"),
            ("cannot set both", "sni_conflict"), ("Invalid TUIC query", "invalid_query"),
            ("Invalid TUIC link", "invalid_link"),
        ],
    }
    for marker, code in tables[protocol]:
        if marker in message: return code
    return "internal_error"


def main(argv: list[str]) -> int:
    if argv: return 2
    raw = sys.stdin.buffer.read(MAX_INPUT_BYTES + 1)
    if len(raw) > MAX_INPUT_BYTES: return 1
    try:
        envelope = json.loads(raw.decode("utf-8", errors="strict"))
        if not isinstance(envelope, dict) or set(envelope) != {"cases"}: return 1
        cases = envelope["cases"]
        if not isinstance(cases, list) or len(cases) > MAX_CASES: return 1
        results = []
        for case in cases:
            if not isinstance(case, dict) or set(case) != {"id", "protocol", "uri", "name", "server_override"}: return 1
            case_id, protocol, uri, name, override = (case[key] for key in ("id", "protocol", "uri", "name", "server_override"))
            if not isinstance(case_id, str) or not SAFE_ID.fullmatch(case_id): return 1
            if protocol not in {"trojan", "hysteria2", "tuic"} or not isinstance(uri, str) or not isinstance(name, str): return 1
            if override is not None and not isinstance(override, str): return 1
            try:
                node = backend.parse_profile(uri)
                outcome = accepted(protocol, uri, node, name, override)
            except backend.BackendError as error:
                outcome = empty(classify(protocol, str(error)))
            results.append({"id": case_id, **outcome})
        sys.stdout.write(json.dumps({"cases": results}, sort_keys=True, separators=(",", ":")))
        return 0
    except (MemoryError, RecursionError, UnicodeError, ValueError, TypeError, json.JSONDecodeError):
        return 1
    except Exception:
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
