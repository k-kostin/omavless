"""Bounded, credential-safe mechanics for the OmaVLESS control protocol.

This module implements the accepted v1 wire contract only.  It deliberately
does not open a socket, dispatch a method, access private state, or manage a VPN
process.  Future daemon, CLI, TUI, and QML bridge code should reuse these
parsers and encoders instead of growing subtly different JSON boundaries.
"""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from typing import Any, BinaryIO, Mapping, NoReturn


API_NAME = "omavless.control"
API_VERSION = 1
SUPPORTED_VERSIONS = (API_VERSION,)

MAX_REQUEST_FRAME_BYTES = 64 * 1024
MAX_RESPONSE_FRAME_BYTES = 256 * 1024
MAX_NESTING_DEPTH = 16
MAX_STRING_BYTES = 32 * 1024
MAX_ID_LENGTH = 64
MAX_METHOD_LENGTH = 128
MAX_REVISION = (1 << 63) - 1
MIN_JSON_INTEGER = -(1 << 63)
MAX_JSON_INTEGER = (1 << 64) - 1

REQUEST_FIELDS = frozenset({"api", "version", "id", "method", "params"})
SUCCESS_RESPONSE_FIELDS = frozenset(
    {"api", "version", "id", "ok", "revision", "result"}
)
ERROR_RESPONSE_FIELDS = frozenset(
    {"api", "version", "id", "ok", "revision", "error"}
)
ERROR_FIELDS = frozenset({"code", "message", "retryable", "details"})

ERROR_MESSAGES = {
    "invalid_request": "The control request is invalid",
    "unsupported_version": "The control protocol version is unsupported",
    "unknown_method": "The control method is not supported",
    "invalid_argument": "A control request argument is invalid",
    "not_found": "The requested item was not found",
    "conflict": "The request conflicts with current state",
    "busy": "Another operation is in progress",
    "capability_unavailable": "The requested capability is unavailable",
    "permission_denied": "The request is not permitted",
    "core_rejected": "The proxy core rejected the operation",
    "transition_failed_restored": "The transition failed and prior state was restored",
    "manual_recovery_required": "Manual recovery is required",
    "daemon_restarting": "The OmaVLESS runtime is restarting",
    "internal_error": "The OmaVLESS runtime encountered an internal error",
}
STABLE_ERROR_CODES = frozenset(ERROR_MESSAGES)


class _DuplicateKey(ValueError):
    pass


@dataclass(frozen=True)
class MachineError:
    """Stable public error data suitable for a v1 response."""

    code: str
    message: str
    retryable: bool = False
    details: Mapping[str, Any] | None = None

    def as_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "code": self.code,
            "message": self.message,
            "retryable": self.retryable,
        }
        if self.details is not None:
            result["details"] = dict(self.details)
        return result


class ProtocolError(ValueError):
    """A bounded protocol failure which never includes raw input."""

    def __init__(
        self,
        code: str,
        message: str | None = None,
        *,
        retryable: bool = False,
        details: Mapping[str, Any] | None = None,
    ) -> None:
        if code not in STABLE_ERROR_CODES:
            code = "internal_error"
        # Parser/validation failures intentionally use only protocol-owned
        # fallback text.  Keeping the optional argument internal-call-site
        # compatible makes it possible to retain precise source comments
        # without ever turning raw caller input into a public error message.
        safe_message = ERROR_MESSAGES[code]
        super().__init__(safe_message)
        safe_details = None
        if code == "unsupported_version" and details == {
            "supported": list(SUPPORTED_VERSIONS)
        }:
            safe_details = {"supported": list(SUPPORTED_VERSIONS)}
        self.machine = MachineError(code, safe_message, retryable, safe_details)

    @property
    def code(self) -> str:
        return self.machine.code


def _is_safe_public_message(message: object) -> bool:
    if not isinstance(message, str) or not message or len(message) > 256:
        return False
    try:
        encoded = message.encode("ascii")
    except UnicodeEncodeError:
        return False
    return all(byte in (9, 32) or 33 <= byte <= 126 for byte in encoded)


def _reject_constant(_value: str) -> NoReturn:
    raise ValueError("non-finite JSON number")


def _object_without_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise _DuplicateKey
        result[key] = value
    return result


def _validate_structure(value: Any, *, depth: int = 0) -> None:
    if isinstance(value, str):
        try:
            encoded = value.encode("utf-8", errors="strict")
        except UnicodeEncodeError as exc:
            raise ProtocolError("invalid_request", "A JSON string is not valid UTF-8") from exc
        if len(encoded) > MAX_STRING_BYTES:
            raise ProtocolError("invalid_request", "A JSON string exceeds the size limit")
        return
    if value is None or isinstance(value, bool):
        return
    if isinstance(value, int):
        if not MIN_JSON_INTEGER <= value <= MAX_JSON_INTEGER:
            raise ProtocolError("invalid_request", "A JSON integer exceeds the range limit")
        return
    if isinstance(value, float):
        if not math.isfinite(value):
            raise ProtocolError("invalid_request", "A JSON number is not finite")
        return
    if isinstance(value, dict):
        next_depth = depth + 1
        if next_depth > MAX_NESTING_DEPTH:
            raise ProtocolError("invalid_request", "JSON nesting exceeds the depth limit")
        for key, item in value.items():
            if not isinstance(key, str):
                raise ProtocolError("invalid_request", "JSON object keys must be strings")
            _validate_structure(key, depth=next_depth)
            _validate_structure(item, depth=next_depth)
        return
    if isinstance(value, list):
        next_depth = depth + 1
        if next_depth > MAX_NESTING_DEPTH:
            raise ProtocolError("invalid_request", "JSON nesting exceeds the depth limit")
        for item in value:
            _validate_structure(item, depth=next_depth)
        return
    raise ProtocolError("invalid_request", "The value is not valid JSON data")


def _decode_frame(frame: bytes, *, limit: int, kind: str) -> Any:
    if not isinstance(frame, bytes):
        raise ProtocolError("invalid_request", f"{kind} frame must be bytes")
    if len(frame) > limit:
        raise ProtocolError("invalid_request", f"{kind} frame exceeds its byte limit")
    if not frame:
        raise ProtocolError("invalid_request", f"{kind} frame is empty")
    if not frame.endswith(b"\n"):
        raise ProtocolError("invalid_request", f"{kind} frame must end with a newline")
    if frame.count(b"\n") != 1:
        raise ProtocolError("invalid_request", f"{kind} frame contains trailing data")
    payload = frame[:-1]
    if not payload:
        raise ProtocolError("invalid_request", f"{kind} frame is empty")
    try:
        text = payload.decode("utf-8", errors="strict")
    except UnicodeDecodeError as exc:
        raise ProtocolError("invalid_request", f"{kind} frame is not valid UTF-8") from exc
    try:
        value = json.loads(
            text,
            object_pairs_hook=_object_without_duplicates,
            parse_constant=_reject_constant,
        )
    except _DuplicateKey as exc:
        raise ProtocolError("invalid_request", "JSON contains a duplicate object key") from exc
    except (json.JSONDecodeError, ValueError) as exc:
        raise ProtocolError("invalid_request", f"{kind} frame is not valid JSON") from exc
    _validate_structure(value)
    return value


def _encode_frame(value: Any, *, limit: int, kind: str) -> bytes:
    _validate_structure(value)
    try:
        encoded = json.dumps(
            value,
            ensure_ascii=False,
            allow_nan=False,
            separators=(",", ":"),
        ).encode("utf-8") + b"\n"
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise ProtocolError("invalid_request", f"{kind} cannot be encoded") from exc
    if len(encoded) > limit:
        raise ProtocolError("invalid_request", f"{kind} frame exceeds its byte limit")
    return encoded


def _validate_ascii_id(value: Any, *, field: str) -> str:
    if not isinstance(value, str) or not 1 <= len(value) <= MAX_ID_LENGTH:
        raise ProtocolError("invalid_request", f"{field} must contain 1 to 64 ASCII characters")
    if any(ord(char) < 0x21 or ord(char) > 0x7E for char in value):
        raise ProtocolError("invalid_request", f"{field} must use visible ASCII characters")
    return value


def _validate_revision(value: Any, *, field: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= MAX_REVISION:
        raise ProtocolError("invalid_request", f"{field} must be a non-negative integer")
    return value


def validate_request(value: Any) -> dict[str, Any]:
    _validate_structure(value)
    if not isinstance(value, dict):
        raise ProtocolError("invalid_request", "A control request must be a JSON object")
    fields = frozenset(value)
    if fields != REQUEST_FIELDS:
        raise ProtocolError("invalid_request", "The control request fields are invalid")
    if value["api"] != API_NAME:
        raise ProtocolError("invalid_request", "The control API name is invalid")
    if (
        isinstance(value["version"], bool)
        or not isinstance(value["version"], int)
        or value["version"] != API_VERSION
    ):
        raise ProtocolError(
            "unsupported_version",
            details={"supported": list(SUPPORTED_VERSIONS)},
        )
    _validate_ascii_id(value["id"], field="id")
    method = value["method"]
    if not isinstance(method, str) or not 1 <= len(method) <= MAX_METHOD_LENGTH:
        raise ProtocolError("invalid_request", "method must be a bounded ASCII string")
    if any(ord(char) < 0x21 or ord(char) > 0x7E for char in method):
        raise ProtocolError("invalid_request", "method must use visible ASCII characters")
    params = value["params"]
    if not isinstance(params, dict):
        raise ProtocolError("invalid_argument", "params must be a JSON object")
    if "operationId" in params:
        _validate_ascii_id(params["operationId"], field="operationId")
    if "expectedRevision" in params:
        _validate_revision(params["expectedRevision"], field="expectedRevision")
    return value


def decode_request(frame: bytes) -> dict[str, Any]:
    return validate_request(
        _decode_frame(frame, limit=MAX_REQUEST_FRAME_BYTES, kind="Request")
    )


def encode_request(value: Mapping[str, Any]) -> bytes:
    request = validate_request(dict(value))
    return _encode_frame(request, limit=MAX_REQUEST_FRAME_BYTES, kind="Request")


def make_request(
    request_id: str,
    method: str,
    params: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    request = {
        "api": API_NAME,
        "version": API_VERSION,
        "id": request_id,
        "method": method,
        "params": dict(params or {}),
    }
    return validate_request(request)


def negotiate_version(versions: Any) -> int:
    if not isinstance(versions, list) or not versions:
        raise ProtocolError("invalid_argument", "versions must be a non-empty array")
    if any(isinstance(item, bool) or not isinstance(item, int) for item in versions):
        raise ProtocolError("invalid_argument", "versions must contain integers")
    common = sorted(set(versions).intersection(SUPPORTED_VERSIONS), reverse=True)
    if not common:
        raise ProtocolError(
            "unsupported_version",
            details={"supported": list(SUPPORTED_VERSIONS)},
        )
    return common[0]


def _validate_response(value: Any) -> dict[str, Any]:
    _validate_structure(value)
    if not isinstance(value, dict):
        raise ProtocolError("invalid_request", "A control response must be a JSON object")
    if value.get("api") != API_NAME:
        raise ProtocolError("invalid_request", "The control API name is invalid")
    if (
        isinstance(value.get("version"), bool)
        or not isinstance(value.get("version"), int)
        or value.get("version") != API_VERSION
    ):
        raise ProtocolError(
            "unsupported_version",
            details={"supported": list(SUPPORTED_VERSIONS)},
        )
    _validate_ascii_id(value.get("id"), field="id")
    _validate_revision(value.get("revision"), field="revision")
    ok = value.get("ok")
    if not isinstance(ok, bool):
        raise ProtocolError("invalid_request", "ok must be a boolean")
    expected_fields = SUCCESS_RESPONSE_FIELDS if ok else ERROR_RESPONSE_FIELDS
    if frozenset(value) != expected_fields:
        raise ProtocolError("invalid_request", "The control response fields are invalid")
    if ok:
        return value
    error = value["error"]
    if not isinstance(error, dict) or not {"code", "message", "retryable"}.issubset(error):
        raise ProtocolError("invalid_request", "The control error is invalid")
    if not frozenset(error).issubset(ERROR_FIELDS):
        raise ProtocolError("invalid_request", "The control error fields are invalid")
    if error["code"] not in STABLE_ERROR_CODES:
        raise ProtocolError("invalid_request", "The control error code is invalid")
    if not _is_safe_public_message(error["message"]):
        raise ProtocolError("invalid_request", "The control error message is invalid")
    if not isinstance(error["retryable"], bool):
        raise ProtocolError("invalid_request", "The control error retry flag is invalid")
    if "details" in error and not isinstance(error["details"], dict):
        raise ProtocolError("invalid_request", "The control error details are invalid")
    return value


def decode_response(frame: bytes) -> dict[str, Any]:
    return _validate_response(
        _decode_frame(frame, limit=MAX_RESPONSE_FRAME_BYTES, kind="Response")
    )


def encode_response(value: Mapping[str, Any]) -> bytes:
    response = _validate_response(dict(value))
    return _encode_frame(response, limit=MAX_RESPONSE_FRAME_BYTES, kind="Response")


def success_response(
    request_id: str,
    *,
    revision: int,
    result: Any,
) -> dict[str, Any]:
    response = {
        "api": API_NAME,
        "version": API_VERSION,
        "id": request_id,
        "ok": True,
        "revision": revision,
        "result": result,
    }
    return _validate_response(response)


def error_response(
    request_id: str,
    *,
    revision: int,
    code: str,
    retryable: bool = False,
    details: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    if code not in STABLE_ERROR_CODES:
        raise ProtocolError("invalid_argument", "The error code is not part of protocol v1")
    if details is not None and not (
        code == "unsupported_version"
        and details == {"supported": list(SUPPORTED_VERSIONS)}
    ):
        raise ProtocolError("invalid_argument", "The error details are not safe for protocol v1")
    machine = MachineError(code, ERROR_MESSAGES[code], retryable, details)
    response = {
        "api": API_NAME,
        "version": API_VERSION,
        "id": request_id,
        "ok": False,
        "revision": revision,
        "error": machine.as_dict(),
    }
    return _validate_response(response)


def read_unary_frame(stream: BinaryIO, *, response: bool = False) -> bytes:
    """Read exactly one bounded frame from a binary stream.

    The helper intentionally consumes at most limit + 1 bytes.  Socket
    deadlines remain the responsibility of the future transport owner.
    """

    limit = MAX_RESPONSE_FRAME_BYTES if response else MAX_REQUEST_FRAME_BYTES
    frame = stream.read(limit + 1)
    _decode_frame(frame, limit=limit, kind="Response" if response else "Request")
    return frame


def write_unary_frame(stream: BinaryIO, frame: bytes, *, response: bool = False) -> None:
    limit = MAX_RESPONSE_FRAME_BYTES if response else MAX_REQUEST_FRAME_BYTES
    _decode_frame(frame, limit=limit, kind="Response" if response else "Request")
    stream.write(frame)
