#!/usr/bin/env python3
"""Credential-safe Python oracle adapter for VLESS XHTTP downloadSettings."""

from __future__ import annotations

import ipaddress
import json
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import backend  # noqa: E402

MAX_INPUT_BYTES = backend.MAX_XHTTP_EXTRA_BYTES + 2048


def empty_outcome(classification: str, *, accepted: bool = False) -> dict[str, Any]:
    return {
        "accepted": accepted,
        "classification": classification,
        "normalized_field_count": 0,
        "server_kind": "none",
        "port_present": False,
        "port": 0,
        "tls_present": False,
        "tls": False,
        "servername_kind": "none",
        "alpn_count": 0,
        "alpn_h2": False,
        "alpn_h3": False,
        "alpn_http_1_1": False,
        "fingerprint_present": False,
        "skip_cert_verify_present": False,
        "skip_cert_verify": False,
        "reality_present": False,
        "reality_short_id_present": False,
        "reality_pq_enabled": False,
        "reality_spider_compatibility": False,
        "reality_pq_compatibility": False,
        "path_present": False,
        "host_kind": "none",
        "header_count": 0,
        "reuse_field_count": 0,
        "max_concurrency": "none",
        "max_connections": "none",
        "c_max_reuse_times": "none",
        "h_max_request_times": "none",
        "h_max_reusable_secs": "none",
        "h_keep_alive_present": False,
        "h_keep_alive_period": 0,
    }


def endpoint_kind(value: Any) -> str:
    if not isinstance(value, str) or not value:
        return "none"
    try:
        address = ipaddress.ip_address(value)
    except ValueError:
        return "dns"
    return "ipv4" if address.version == 4 else "ipv6"


def accepted_outcome(result: dict[str, Any], compatibility_note: str) -> dict[str, Any]:
    outcome = empty_outcome("accepted", accepted=True)
    outcome["normalized_field_count"] = len(result)
    outcome["server_kind"] = endpoint_kind(result.get("server"))
    outcome["port_present"] = "port" in result
    outcome["port"] = result.get("port", 0)
    outcome["tls_present"] = "tls" in result
    outcome["tls"] = result.get("tls", False)
    outcome["servername_kind"] = endpoint_kind(result.get("servername"))
    alpn = result.get("alpn", [])
    outcome["alpn_count"] = len(alpn)
    outcome["alpn_h2"] = "h2" in alpn
    outcome["alpn_h3"] = "h3" in alpn
    outcome["alpn_http_1_1"] = "http/1.1" in alpn
    outcome["fingerprint_present"] = "client-fingerprint" in result
    outcome["skip_cert_verify_present"] = "skip-cert-verify" in result
    outcome["skip_cert_verify"] = result.get("skip-cert-verify", False)
    reality = result.get("reality-opts")
    outcome["reality_present"] = isinstance(reality, dict)
    if isinstance(reality, dict):
        outcome["reality_short_id_present"] = bool(reality.get("short-id"))
        outcome["reality_pq_enabled"] = bool(reality.get("support-x25519mlkem768"))
    outcome["reality_spider_compatibility"] = (
        backend.REALITY_SPX_COMPATIBILITY_NOTE in compatibility_note
    )
    outcome["reality_pq_compatibility"] = (
        backend.REALITY_PQ_COMPATIBILITY_NOTE in compatibility_note
    )
    outcome["path_present"] = "path" in result
    outcome["host_kind"] = endpoint_kind(result.get("host"))
    headers = result.get("headers", {})
    outcome["header_count"] = len(headers)
    reuse = result.get("reuse-settings", {})
    outcome["reuse_field_count"] = len(reuse)
    for source, target in (
        ("max-concurrency", "max_concurrency"),
        ("max-connections", "max_connections"),
        ("c-max-reuse-times", "c_max_reuse_times"),
        ("h-max-request-times", "h_max_request_times"),
        ("h-max-reusable-secs", "h_max_reusable_secs"),
    ):
        outcome[target] = reuse.get(source, "none")
    outcome["h_keep_alive_present"] = "h-keep-alive-period" in reuse
    outcome["h_keep_alive_period"] = reuse.get("h-keep-alive-period", 0)
    return outcome


def error_code(message: str) -> str:
    exact = {
        "VLESS XHTTP extra contains duplicate fields": "duplicate_fields",
        "VLESS XHTTP extra is not valid JSON": "invalid_json",
        "VLESS XHTTP extra is nested too deeply": "too_deep",
        "VLESS XHTTP extra contains too many values": "too_many_values",
        "VLESS XHTTP extra contains an oversized field name": "oversized_field_name",
        "VLESS XHTTP extra contains an oversized string": "oversized_string",
        "VLESS XHTTP extra must be a JSON object": "non_object_root",
        "VLESS XHTTP stream-one cannot use downloadSettings": "stream_one",
        "VLESS XHTTP downloadSettings must be an object": "non_object_root",
        "VLESS XHTTP downloadSettings contains unsupported fields": "unsupported_fields",
        "VLESS XHTTP download sockopt is not imported": "sockopt",
        "VLESS XHTTP download port is invalid": "port",
        "VLESS XHTTP download network must be xhttp": "network",
        "VLESS XHTTP download security is unsupported": "security",
        "VLESS XHTTP download tlsSettings must be an object": "tls_object",
        "VLESS XHTTP download tlsSettings contains unsupported fields": "tls_fields",
        "VLESS XHTTP download TLS settings conflict with security none": "tls_security_conflict",
        "VLESS XHTTP download tlsSettings show is unsupported": "tls_show",
        "VLESS XHTTP download ALPN has an invalid format": "alpn_format",
        "VLESS XHTTP download ALPN has an unsupported value": "alpn_value",
        "VLESS XHTTP download Reality conflicts with its security": "reality_security_conflict",
        "VLESS XHTTP download realitySettings must be an object": "reality_object",
        "VLESS XHTTP download realitySettings contains unsupported fields": "reality_fields",
        "VLESS XHTTP download Reality show is unsupported": "reality_show",
        "VLESS XHTTP download Reality ML-DSA verification is not supported by Mihomo": "reality_mldsa",
        "VLESS XHTTP download Reality requires a public key": "reality_public_key_required",
        "VLESS Reality public key has an invalid format": "reality_public_key",
        "VLESS XHTTP download Reality short ID has an invalid format": "reality_short_id",
        "VLESS Reality short ID has an invalid format": "reality_short_id",
        "VLESS XHTTP extra contains conflicting field aliases": "alias_conflict",
        "VLESS XHTTP download Reality requires realitySettings when upload is not Reality": "reality_settings_required",
        "VLESS XHTTP download xhttpSettings must be an object": "transport_object",
        "VLESS XHTTP download xhttpSettings contains unsupported fields": "transport_fields",
        "VLESS XHTTP download path has an invalid format": "path_format",
        "VLESS XHTTP download mode is unsupported": "mode",
        "VLESS XHTTP upload and download modes must match in Mihomo": "mode_mismatch",
        "VLESS XHTTP download transport extra must be an object": "transport_extra_object",
        "VLESS XHTTP download transport extra contains unsupported fields": "transport_extra_fields",
        "VLESS XHTTP recursive download settings are not supported": "recursive_download",
        "VLESS XHTTP download transport mode is invalid": "transport_mode",
        "VLESS XHTTP download headers conflict": "headers_conflict",
        "VLESS XHTTP headers have an invalid format": "headers_format",
        "VLESS XHTTP contains an unsupported header name": "header_name",
        "VLESS XHTTP contains an invalid header value": "header_value",
        "VLESS XHTTP xmux must be an object": "xmux_object",
        "VLESS XHTTP xmux contains unsupported fields": "xmux_fields",
        "VLESS XHTTP xmux maxConcurrency and maxConnections are mutually exclusive": "xmux_exclusive",
        "VLESS XHTTP xmux hKeepAlivePeriod is invalid": "xmux_keep_alive",
    }
    if message in exact:
        return exact[message]
    if message.startswith("VLESS XHTTP extra is too large"):
        return "too_large"
    if message.startswith("VLESS XHTTP download ") and message.endswith(" has an invalid format"):
        if "fingerprint" in message:
            return "token_format"
        if "transport host" in message or "transport path" in message:
            return "transport_compatibility_format"
        return "endpoint_format"
    if "must be an integer or range" in message:
        return "range_type"
    if "is outside the supported range" in message:
        return "range_bounds"
    if "must be true or false" in message:
        return "boolean_type"
    if "cannot be overridden independently in Mihomo" in message:
        return "independent_override"
    return "internal_error"


def emit(outcome: dict[str, Any]) -> None:
    print(json.dumps(outcome, sort_keys=True, separators=(",", ":")))


def main(argv: list[str]) -> int:
    if argv:
        emit(empty_outcome("internal_error"))
        return 1
    raw_input = sys.stdin.buffer.read(MAX_INPUT_BYTES + 1)
    if len(raw_input) > MAX_INPUT_BYTES:
        emit(empty_outcome("internal_error"))
        return 1
    try:
        envelope = json.loads(raw_input)
        if not isinstance(envelope, dict) or set(envelope) != {
            "raw", "main_mode", "main_security"
        }:
            raise ValueError("invalid adapter envelope")
        raw = envelope["raw"]
        main_mode = envelope["main_mode"]
        main_security = envelope["main_security"]
        if not all(isinstance(value, str) for value in (raw, main_mode, main_security)):
            raise ValueError("invalid adapter envelope")
        document = backend.decode_xhttp_extra(raw)
        result, compatibility_note = backend.parse_xhttp_download(
            document, main_mode, main_security
        )
        emit(accepted_outcome(result, compatibility_note))
        return 0
    except backend.BackendError as error:
        emit(empty_outcome(error_code(str(error))))
        return 0
    except (UnicodeError, RecursionError, TypeError, ValueError, json.JSONDecodeError):
        emit(empty_outcome("internal_error"))
        return 1
    except (MemoryError, OSError):
        emit(empty_outcome("internal_error"))
        return 1
    except Exception:  # noqa: BLE001 - never expose unexpected oracle details
        emit(empty_outcome("internal_error"))
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
