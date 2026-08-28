import importlib.util
import io
import json
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "omavless_control_protocol", ROOT / "omavless_control_protocol.py"
)
protocol = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = protocol
SPEC.loader.exec_module(protocol)

CORPUS = json.loads(
    (ROOT / "tests" / "control_protocol_cases" / "cases.json").read_text(
        encoding="utf-8"
    )
)


class ControlProtocolTests(unittest.TestCase):
    def assert_protocol_error(self, frame, *, code="invalid_request", response=False):
        decoder = protocol.decode_response if response else protocol.decode_request
        with self.assertRaises(protocol.ProtocolError) as caught:
            decoder(frame)
        self.assertEqual(caught.exception.code, code)
        self.assertLessEqual(len(str(caught.exception)), 256)
        return str(caught.exception)

    def base_request(self, **changes):
        value = protocol.make_request("request-1", "status.get")
        value.update(changes)
        return value

    def encoded(self, value):
        return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode() + b"\n"

    def test_checked_in_request_corpus_round_trips(self):
        for case in CORPUS["requests"]:
            with self.subTest(case=case["name"]):
                frame = protocol.encode_request(case["value"])
                self.assertEqual(protocol.decode_request(frame), case["value"])

    def test_checked_in_response_corpus_round_trips(self):
        for case in CORPUS["responses"]:
            with self.subTest(case=case["name"]):
                frame = protocol.encode_response(case["value"])
                self.assertEqual(protocol.decode_response(frame), case["value"])

    def test_hello_negotiates_v1(self):
        request = protocol.decode_request(
            protocol.encode_request(CORPUS["requests"][0]["value"])
        )
        self.assertEqual(protocol.negotiate_version(request["params"]["versions"]), 1)

    def test_ordinary_request_and_unicode_are_valid(self):
        request = protocol.make_request(
            "ordinary-1", "profiles.list", {"query": "Москва"}
        )
        self.assertEqual(protocol.decode_request(protocol.encode_request(request)), request)

    def test_success_response_helper(self):
        response = protocol.success_response(
            "status-1", revision=7, result={"actual": "disconnected"}
        )
        self.assertTrue(protocol.decode_response(protocol.encode_response(response))["ok"])

    def test_error_response_helper_uses_stable_fallback(self):
        response = protocol.error_response(
            "connect-1", revision=7, code="conflict", retryable=True
        )
        parsed = protocol.decode_response(protocol.encode_response(response))
        self.assertEqual(parsed["error"]["message"], protocol.ERROR_MESSAGES["conflict"])
        self.assertTrue(parsed["error"]["retryable"])

    def test_every_reserved_error_code_has_a_safe_encodable_fallback(self):
        for code in sorted(protocol.STABLE_ERROR_CODES):
            with self.subTest(code=code):
                response = protocol.error_response("error-1", revision=0, code=code)
                parsed = protocol.decode_response(protocol.encode_response(response))
                self.assertEqual(parsed["error"]["code"], code)
                self.assertEqual(parsed["error"]["message"], protocol.ERROR_MESSAGES[code])

    def test_max_legal_id(self):
        request = protocol.make_request("x" * 64, "status.get")
        self.assertEqual(len(protocol.decode_request(protocol.encode_request(request))["id"]), 64)

    def test_max_legal_string(self):
        request = protocol.make_request(
            "bounded-1", "imports.classify", {"content": "x" * (32 * 1024)}
        )
        self.assertEqual(
            len(protocol.decode_request(protocol.encode_request(request))["params"]["content"]),
            32 * 1024,
        )

    def test_max_legal_nesting(self):
        nested = "leaf"
        # Request object + params object + fourteen arrays = depth sixteen.
        for _ in range(14):
            nested = [nested]
        request = protocol.make_request("depth-1", "status.get", {"value": nested})
        self.assertEqual(protocol.decode_request(protocol.encode_request(request)), request)

    def test_empty_frame(self):
        self.assert_protocol_error(b"")
        self.assert_protocol_error(b"\n")

    def test_missing_newline(self):
        self.assert_protocol_error(self.encoded(self.base_request())[:-1])

    def test_invalid_utf8(self):
        self.assert_protocol_error(b'{"api":"\xff"}\n')

    def test_escaped_unpaired_surrogate_is_rejected(self):
        self.assert_protocol_error(
            b'{"api":"omavless.control","version":1,"id":"x",'
            b'"method":"status.get","params":{"value":"\\ud800"}}\n'
        )

    def test_duplicate_top_level_key(self):
        self.assert_protocol_error(
            b'{"api":"omavless.control","api":"omavless.control","version":1,'
            b'"id":"x","method":"status.get","params":{}}\n'
        )

    def test_duplicate_nested_key(self):
        self.assert_protocol_error(
            b'{"api":"omavless.control","version":1,"id":"x",'
            b'"method":"status.get","params":{"value":1,"value":2}}\n'
        )

    def test_non_object_json(self):
        self.assert_protocol_error(b"[]\n")

    def test_trailing_second_json_value(self):
        self.assert_protocol_error(b"{}{}\n")
        self.assert_protocol_error(b"{}\n{}\n")

    def test_unknown_top_level_field(self):
        request = self.base_request(extra=True)
        self.assert_protocol_error(self.encoded(request))

    def test_wrong_api(self):
        self.assert_protocol_error(self.encoded(self.base_request(api="other.control")))

    def test_unsupported_version(self):
        self.assert_protocol_error(
            self.encoded(self.base_request(version=2)), code="unsupported_version"
        )
        with self.assertRaises(protocol.ProtocolError) as caught:
            protocol.negotiate_version([2, 3])
        self.assertEqual(caught.exception.code, "unsupported_version")

    def test_missing_required_field(self):
        request = self.base_request()
        request.pop("params")
        self.assert_protocol_error(self.encoded(request))

    def test_bad_id_charset(self):
        for request_id in ("contains space", "строка", "line\nbreak"):
            with self.subTest(request_id=repr(request_id)):
                self.assert_protocol_error(self.encoded(self.base_request(id=request_id)))

    def test_zero_length_id(self):
        self.assert_protocol_error(self.encoded(self.base_request(id="")))

    def test_overlong_id(self):
        self.assert_protocol_error(self.encoded(self.base_request(id="x" * 65)))

    def test_excessive_nesting(self):
        nested = "leaf"
        for _ in range(15):
            nested = [nested]
        self.assert_protocol_error(
            self.encoded(self.base_request(params={"value": nested}))
        )

    def test_oversized_request(self):
        # Two individually legal strings make the complete request exceed 64 KiB.
        request = self.base_request(
            params={"first": "a" * (32 * 1024), "second": "b" * (32 * 1024)}
        )
        with self.assertRaises(protocol.ProtocolError):
            protocol.encode_request(request)

    def test_oversized_raw_request_is_rejected_before_json(self):
        self.assert_protocol_error(b" " * protocol.MAX_REQUEST_FRAME_BYTES + b"\n")

    def test_oversized_response_is_rejected(self):
        response = protocol.success_response(
            "large-1",
            revision=0,
            result={str(index): "x" * (32 * 1024) for index in range(8)},
        )
        with self.assertRaises(protocol.ProtocolError):
            protocol.encode_response(response)

    def test_oversized_string(self):
        request = self.base_request(params={"value": "x" * (32 * 1024 + 1)})
        self.assert_protocol_error(self.encoded(request))

    def test_invalid_params_shape(self):
        self.assert_protocol_error(
            self.encoded(self.base_request(params=[])), code="invalid_argument"
        )

    def test_malformed_operation_id(self):
        for value in ("", "x" * 65, "has space", 7):
            with self.subTest(value=repr(value)):
                self.assert_protocol_error(
                    self.encoded(self.base_request(params={"operationId": value}))
                )

    def test_malformed_expected_revision(self):
        for value in (-1, True, "1", protocol.MAX_REVISION + 1):
            with self.subTest(value=repr(value)):
                self.assert_protocol_error(
                    self.encoded(self.base_request(params={"expectedRevision": value}))
                )

    def test_non_finite_number_is_rejected(self):
        self.assert_protocol_error(
            b'{"api":"omavless.control","version":1,"id":"x",'
            b'"method":"status.get","params":{"value":NaN}}\n'
        )

    def test_unary_read_write_helpers(self):
        frame = protocol.encode_request(self.base_request())
        self.assertEqual(protocol.read_unary_frame(io.BytesIO(frame)), frame)
        output = io.BytesIO()
        protocol.write_unary_frame(output, frame)
        self.assertEqual(output.getvalue(), frame)

    def test_unary_reader_rejects_two_frames(self):
        frame = protocol.encode_request(self.base_request())
        with self.assertRaises(protocol.ProtocolError):
            protocol.read_unary_frame(io.BytesIO(frame + frame))

    def test_parser_errors_do_not_echo_private_input(self):
        private_markers = (
            "vless://private-value",
            "password=private-value",
            "private-key-value",
            "private.example.invalid",
        )
        for marker in private_markers:
            with self.subTest(marker=marker):
                message = self.assert_protocol_error(
                    ("{malformed:" + marker + "}\n").encode()
                )
                self.assertNotIn(marker, message)
                self.assertNotIn("malformed", message)

    def test_protocol_error_constructor_cannot_publish_caller_text(self):
        error = protocol.ProtocolError(
            "invalid_request", "vless://private.example.invalid?password=secret"
        )
        self.assertEqual(str(error), protocol.ERROR_MESSAGES["invalid_request"])

    def test_unknown_error_code_is_not_encoded(self):
        with self.assertRaises(protocol.ProtocolError):
            protocol.error_response(
                "error-1", revision=0, code="future_unregistered_error"
            )

    def test_unknown_response_top_level_field_is_rejected(self):
        response = protocol.success_response("status-1", revision=0, result={})
        response["extra"] = True
        self.assert_protocol_error(self.encoded(response), response=True)

    def test_error_details_cannot_carry_arbitrary_private_data(self):
        with self.assertRaises(protocol.ProtocolError):
            protocol.error_response(
                "error-1",
                revision=0,
                code="invalid_argument",
                details={"raw": "vless://private.example.invalid"},
            )
        response = protocol.error_response(
            "error-1",
            revision=0,
            code="unsupported_version",
            details={"supported": [1]},
        )
        self.assertEqual(response["error"]["details"], {"supported": [1]})


if __name__ == "__main__":
    unittest.main()
