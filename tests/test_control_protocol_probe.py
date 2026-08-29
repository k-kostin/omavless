import ast
import json
import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PROBE = ROOT / "tools" / "control_protocol_probe.py"


class ControlProtocolProbeTests(unittest.TestCase):
    def run_probe(self, *args, frame=b""):
        return subprocess.run(
            [sys.executable, str(PROBE), *args],
            input=frame,
            capture_output=True,
            check=False,
            timeout=5,
        )

    def test_hello_emits_only_the_fixed_v1_request(self):
        result = self.run_probe("hello")
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stderr, b"")
        value = json.loads(result.stdout)
        self.assertEqual(
            value,
            {
                "api": "omavless.control",
                "version": 1,
                "id": "hello-1",
                "method": "system.hello",
                "params": {"versions": [1]},
            },
        )
        self.assertTrue(result.stdout.endswith(b"\n"))

    def test_request_and_response_validation_are_end_to_end(self):
        request = (
            b'{"api":"omavless.control","version":1,"id":"status-1",'
            b'"method":"status.get","params":{"query":"Unicode-\xd0\x9c\xd0\xbe\xd1\x81\xd0\xba\xd0\xb2\xd0\xb0"}}\n'
        )
        response = (
            b'{"api":"omavless.control","version":1,"id":"status-1",'
            b'"ok":true,"revision":4,"result":{"actual":"disconnected"}}\n'
        )
        valid_request = self.run_probe("request", frame=request)
        valid_response = self.run_probe("response", frame=response)
        self.assertEqual(
            (valid_request.returncode, valid_request.stdout),
            (0, b"VALID request\n"),
        )
        self.assertEqual(
            (valid_response.returncode, valid_response.stdout),
            (0, b"VALID response\n"),
        )
        self.assertEqual(valid_request.stderr + valid_response.stderr, b"")

    def test_invalid_private_input_is_never_echoed(self):
        marker = b"vless://private.example.invalid?password=secret"
        result = self.run_probe("request", frame=b"{broken:" + marker + b"}\n")
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, b"")
        self.assertIn(b"INVALID request", result.stderr)
        self.assertNotIn(marker, result.stderr)
        self.assertNotIn(b"password", result.stderr)

        valid_private = (
            b'{"api":"omavless.control","version":1,"id":"private-1",'
            b'"method":"imports.classify","params":{"content":"'
            + marker
            + b'"}}\n'
        )
        valid = self.run_probe("request", frame=valid_private)
        self.assertEqual((valid.returncode, valid.stdout), (0, b"VALID request\n"))
        self.assertEqual(valid.stderr, b"")
        self.assertNotIn(marker, valid.stdout)

    def test_unary_and_byte_bounds_are_enforced(self):
        hello = self.run_probe("hello").stdout
        for frame in (hello + hello, b" " * (64 * 1024) + b"\n"):
            with self.subTest(size=len(frame)):
                result = self.run_probe("request", frame=frame)
                self.assertEqual(result.returncode, 2)
                self.assertEqual(result.stdout, b"")
                self.assertLessEqual(len(result.stderr), 300)

    def test_command_line_cannot_select_methods_or_echo_arguments(self):
        marker = "vless://private.example.invalid"
        result = self.run_probe("hello", marker)
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, b"")
        self.assertNotIn(marker.encode(), result.stderr)

        tree = ast.parse(PROBE.read_text(encoding="utf-8"))
        imported = set()
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                imported.update(alias.name.split(".", 1)[0] for alias in node.names)
            elif isinstance(node, ast.ImportFrom) and node.module:
                imported.add(node.module.split(".", 1)[0])
        self.assertTrue({"subprocess", "socket", "backend"}.isdisjoint(imported))


if __name__ == "__main__":
    unittest.main()
