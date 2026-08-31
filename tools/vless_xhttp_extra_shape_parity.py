#!/usr/bin/env python3
"""Credential-safe Python oracle adapter for VLESS XHTTP-extra shape parity."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import backend  # noqa: E402


def emit(
    *,
    accepted: bool,
    classification: str,
    source_empty: bool = False,
    root_field_count: int = 0,
    value_count: int = 0,
    object_count: int = 0,
    array_count: int = 0,
    string_count: int = 0,
    integer_count: int = 0,
    float_count: int = 0,
    boolean_count: int = 0,
    true_count: int = 0,
    false_count: int = 0,
    null_count: int = 0,
    maximum_depth: int = 0,
    non_ascii_present: bool = False,
) -> None:
    print(json.dumps({
        "accepted": accepted,
        "classification": classification,
        "source_empty": source_empty,
        "root_field_count": root_field_count,
        "value_count": value_count,
        "object_count": object_count,
        "array_count": array_count,
        "string_count": string_count,
        "integer_count": integer_count,
        "float_count": float_count,
        "boolean_count": boolean_count,
        "true_count": true_count,
        "false_count": false_count,
        "null_count": null_count,
        "maximum_depth": maximum_depth,
        "non_ascii_present": non_ascii_present,
    }, sort_keys=True, separators=(",", ":")))


def error_code(message: str) -> str:
    exact = {
        "VLESS XHTTP extra contains duplicate fields": "duplicate_fields",
        "VLESS XHTTP extra is not valid JSON": "invalid_json",
        "VLESS XHTTP extra is nested too deeply": "too_deep",
        "VLESS XHTTP extra contains too many values": "too_many_values",
        "VLESS XHTTP extra contains an oversized field name": "oversized_field_name",
        "VLESS XHTTP extra contains an oversized string": "oversized_string",
        "VLESS XHTTP extra must be a JSON object": "non_object_root",
    }
    if message.startswith("VLESS XHTTP extra is too large"):
        return "too_large"
    return exact.get(message, "internal_error")


def summarize(data: dict[str, Any], *, source_empty: bool) -> dict[str, int | bool]:
    facts: dict[str, int | bool] = {
        "source_empty": source_empty,
        "root_field_count": len(data),
        "value_count": 0,
        "object_count": 0,
        "array_count": 0,
        "string_count": 0,
        "integer_count": 0,
        "float_count": 0,
        "boolean_count": 0,
        "true_count": 0,
        "false_count": 0,
        "null_count": 0,
        "maximum_depth": 0,
        "non_ascii_present": False,
    }

    def visit(value: Any, depth: int) -> None:
        facts["value_count"] = int(facts["value_count"]) + 1
        facts["maximum_depth"] = max(int(facts["maximum_depth"]), depth)
        if isinstance(value, dict):
            facts["object_count"] = int(facts["object_count"]) + 1
            for key, child in value.items():
                if not key.isascii():
                    facts["non_ascii_present"] = True
                visit(child, depth + 1)
        elif isinstance(value, list):
            facts["array_count"] = int(facts["array_count"]) + 1
            for child in value:
                visit(child, depth + 1)
        elif isinstance(value, str):
            facts["string_count"] = int(facts["string_count"]) + 1
            if not value.isascii():
                facts["non_ascii_present"] = True
        elif isinstance(value, bool):
            facts["boolean_count"] = int(facts["boolean_count"]) + 1
            key = "true_count" if value else "false_count"
            facts[key] = int(facts[key]) + 1
        elif isinstance(value, int):
            facts["integer_count"] = int(facts["integer_count"]) + 1
        elif isinstance(value, float):
            facts["float_count"] = int(facts["float_count"]) + 1
        elif value is None:
            facts["null_count"] = int(facts["null_count"]) + 1
        else:
            raise TypeError("unsupported accepted JSON value")

    visit(data, 0)
    return facts


def main(argv: list[str]) -> int:
    if argv:
        print("Usage: vless_xhttp_extra_shape_parity.py", file=sys.stderr)
        return 2

    try:
        raw = sys.stdin.buffer.read(backend.MAX_XHTTP_EXTRA_BYTES + 1)
        if len(raw) > backend.MAX_XHTTP_EXTRA_BYTES:
            emit(accepted=False, classification="too_large")
            return 0
        try:
            text = raw.decode("utf-8", errors="strict")
        except UnicodeDecodeError:
            emit(accepted=False, classification="invalid_utf8")
            return 0

        data = backend.decode_xhttp_extra(text)
        emit(
            accepted=True,
            classification="accepted",
            **summarize(data, source_empty=not raw),
        )
        return 0
    except backend.BackendError as error:
        emit(accepted=False, classification=error_code(str(error)))
        return 0
    except (UnicodeError, RecursionError, TypeError, ValueError):
        # Python's decoder can materialize lone UTF-16 surrogates and fail only
        # during the accepted shape's UTF-8 byte accounting. Treat that as the
        # fixed fail-closed invalid-JSON class rather than exposing the fragment.
        emit(accepted=False, classification="invalid_json")
        return 0
    except (MemoryError, OSError):
        emit(accepted=False, classification="internal_error")
        return 1
    except Exception:  # noqa: BLE001 - never expose unexpected oracle details
        emit(accepted=False, classification="internal_error")
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
