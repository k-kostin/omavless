import json
import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ADAPTER = ROOT / "tools" / "control_protocol_parity.py"


class ControlProtocolParityAdapterTests(unittest.TestCase):
    def run_adapter(self, kind, frame):
        return subprocess.run(
            [sys.executable, str(ADAPTER), kind],
            input=frame,
            capture_output=True,
            check=False,
            timeout=5,
        )

    def test_valid_frame_has_only_sanitized_acceptance(self):
        marker = b"vless://private.example.invalid?password=secret"
        frame = (
            b'{"api":"omavless.control","version":1,"id":"private-1",'
            b'"method":"imports.classify","params":{"content":"'
            + marker
            + b'"}}\n'
        )
        result = self.run_adapter("request", frame)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stderr, b"")
        self.assertEqual(
            json.loads(result.stdout),
            {"accepted": True, "classification": "accepted"},
        )
        self.assertNotIn(marker, result.stdout)

    def test_invalid_frame_never_echoes_private_input(self):
        marker = b"password=private-marker"
        result = self.run_adapter("request", b"{broken:" + marker + b"}\n")
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stderr, b"")
        self.assertEqual(
            json.loads(result.stdout),
            {"accepted": False, "classification": "invalid_request"},
        )
        self.assertNotIn(marker, result.stdout)

    def test_arbitrary_commands_and_arguments_are_rejected_without_echo(self):
        marker = "vless://private.invalid"
        result = subprocess.run(
            [sys.executable, str(ADAPTER), "request", marker],
            capture_output=True,
            check=False,
            timeout=5,
        )
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, b"")
        self.assertNotIn(marker.encode(), result.stderr)


if __name__ == "__main__":
    unittest.main()
