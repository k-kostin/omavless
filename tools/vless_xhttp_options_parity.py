#!/usr/bin/env python3
"""Credential-safe Python oracle adapter for normalized VLESS XHTTP options."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import backend  # noqa: E402

MAX_INPUT_BYTES = backend.MAX_XHTTP_EXTRA_BYTES + 1024


def empty_outcome(classification: str, *, accepted: bool = False) -> dict[str, Any]:
    return {
        "accepted": accepted,
        "classification": classification,
        "normalized_field_count": 0,
        "header_count": 0,
        "x_padding_bytes": "none",
        "uplink_chunk_size": "none",
        "sc_max_each_post_bytes": "none",
        "sc_min_posts_interval_ms": "none",
        "x_padding_obfs_mode": False,
        "no_grpc_header": False,
        "x_padding_placement": "none",
        "x_padding_method": "none",
        "uplink_http_method": "none",
        "seq_placement": "none",
        "uplink_data_placement": "none",
        "x_padding_key_present": False,
        "x_padding_header_present": False,
        "seq_key_present": False,
        "uplink_data_key_present": False,
        "session_placement": "none",
        "session_key_present": False,
        "session_table_present": False,
        "session_length": "none",
        "reuse_field_count": 0,
        "max_concurrency": "none",
        "max_connections": "none",
        "c_max_reuse_times": "none",
        "h_max_request_times": "none",
        "h_max_reusable_secs": "none",
        "h_keep_alive_present": False,
        "h_keep_alive_period": 0,
    }


def emit(outcome: dict[str, Any]) -> None:
    print(json.dumps(outcome, sort_keys=True, separators=(",", ":")))


def error_code(message: str) -> str:
    exact = {
        "VLESS XHTTP extra contains duplicate fields": "duplicate_fields",
        "VLESS XHTTP extra is not valid JSON": "invalid_json",
        "VLESS XHTTP extra is nested too deeply": "too_deep",
        "VLESS XHTTP extra contains too many values": "too_many_values",
        "VLESS XHTTP extra contains an oversized field name": "oversized_field_name",
        "VLESS XHTTP extra contains an oversized string": "oversized_string",
        "VLESS XHTTP extra must be a JSON object": "non_object_root",
        "VLESS XHTTP extra contains unsupported fields": "unsupported_fields",
        "VLESS XHTTP extra mode is invalid": "compatibility_mode",
        "VLESS XHTTP recursive extra objects are not supported": "recursive_extra",
        "VLESS XHTTP headers have an invalid format": "headers_format",
        "VLESS XHTTP contains an unsupported header name": "header_name",
        "VLESS XHTTP contains an invalid header value": "header_value",
        "VLESS XHTTP extra contains conflicting field aliases": "alias_conflict",
        "VLESS XHTTP session placement has an unsupported value": "session_placement",
        "VLESS XHTTP seq placement must be path when session placement is path": (
            "session_sequence_conflict"
        ),
        "VLESS XHTTP xmux must be an object": "xmux_object",
        "VLESS XHTTP xmux contains unsupported fields": "xmux_fields",
        "VLESS XHTTP xmux maxConcurrency and maxConnections are mutually exclusive": (
            "xmux_exclusive"
        ),
        "VLESS XHTTP xmux hKeepAlivePeriod is invalid": "xmux_keep_alive",
    }
    if message == "VLESS XHTTP downloadSettings is outside the R2h2 layer":
        return "download_settings_outside_slice"
    if message in exact:
        return exact[message]
    if message.startswith("VLESS XHTTP extra is too large"):
        return "too_large"
    if message in {
        "VLESS XHTTP extra host has an invalid format",
        "VLESS XHTTP extra path has an invalid format",
    }:
        return "compatibility_field_format"
    if message.startswith("VLESS XHTTP ") and message.endswith(
        " is server-only and cannot be imported"
    ):
        return "server_only_field"
    if "must be an integer or range" in message:
        return "range_type"
    if message.endswith("is outside the supported range"):
        return "range_bounds"
    if message.endswith("must be true or false"):
        return "boolean_type"
    if message.endswith("has an invalid format"):
        return "token_format"
    if message.endswith("has an unsupported value"):
        return "enum_value"
    return exact.get(message, "internal_error")


def range_code(value: Any) -> str:
    if value is None:
        return "none"
    if not isinstance(value, str):
        raise TypeError("normalized range is not a string")
    return value


def accepted_outcome(result: dict[str, Any]) -> dict[str, Any]:
    outcome = empty_outcome("accepted", accepted=True)
    reuse = result.get("reuse-settings", {})
    headers = result.get("headers", {})
    if not isinstance(reuse, dict) or not isinstance(headers, dict):
        raise TypeError("normalized mapping has an invalid shape")
    outcome.update({
        "normalized_field_count": len(result),
        "header_count": len(headers),
        "x_padding_bytes": range_code(result.get("x-padding-bytes")),
        "uplink_chunk_size": range_code(result.get("uplink-chunk-size")),
        "sc_max_each_post_bytes": range_code(result.get("sc-max-each-post-bytes")),
        "sc_min_posts_interval_ms": range_code(result.get("sc-min-posts-interval-ms")),
        "x_padding_obfs_mode": result.get("x-padding-obfs-mode") is True,
        "no_grpc_header": result.get("no-grpc-header") is True,
        "x_padding_placement": result.get("x-padding-placement", "none"),
        "x_padding_method": result.get("x-padding-method", "none"),
        "uplink_http_method": result.get("uplink-http-method", "none"),
        "seq_placement": result.get("seq-placement", "none"),
        "uplink_data_placement": result.get("uplink-data-placement", "none"),
        "x_padding_key_present": "x-padding-key" in result,
        "x_padding_header_present": "x-padding-header" in result,
        "seq_key_present": "seq-key" in result,
        "uplink_data_key_present": "uplink-data-key" in result,
        "session_placement": result.get("session-placement", "none"),
        "session_key_present": "session-key" in result,
        "session_table_present": "session-table" in result,
        "session_length": range_code(result.get("session-length")),
        "reuse_field_count": len(reuse),
        "max_concurrency": range_code(reuse.get("max-concurrency")),
        "max_connections": range_code(reuse.get("max-connections")),
        "c_max_reuse_times": range_code(reuse.get("c-max-reuse-times")),
        "h_max_request_times": range_code(reuse.get("h-max-request-times")),
        "h_max_reusable_secs": range_code(reuse.get("h-max-reusable-secs")),
        "h_keep_alive_present": "h-keep-alive-period" in reuse,
        "h_keep_alive_period": reuse.get("h-keep-alive-period", 0),
    })
    return outcome


def main(argv: list[str]) -> int:
    if argv:
        print("Usage: vless_xhttp_options_parity.py", file=sys.stderr)
        return 2
    try:
        raw_input = sys.stdin.buffer.read(MAX_INPUT_BYTES + 1)
        if len(raw_input) > MAX_INPUT_BYTES:
            emit(empty_outcome("internal_error"))
            return 1
        request = json.loads(raw_input.decode("utf-8", errors="strict"))
        if not isinstance(request, dict) or set(request) != {"raw"}:
            emit(empty_outcome("internal_error"))
            return 1
        raw = request["raw"]
        if not isinstance(raw, str):
            emit(empty_outcome("internal_error"))
            return 1

        original_parse_download = backend.parse_xhttp_download

        def outside_download(*_args: Any, **_kwargs: Any) -> tuple[dict[str, Any], str]:
            raise backend.BackendError(
                "VLESS XHTTP downloadSettings is outside the R2h2 layer"
            )

        backend.parse_xhttp_download = outside_download
        try:
            result = backend.parse_xhttp_extra(raw, "auto", "none")
        finally:
            backend.parse_xhttp_download = original_parse_download
        emit(accepted_outcome(result))
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
