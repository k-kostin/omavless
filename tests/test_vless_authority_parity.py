import json
import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ADAPTER = ROOT / "tools" / "vless_authority_parity.py"
UUID = "11111111-1111-4111-8111-111111111111"


class VlessAuthorityParityAdapterTests(unittest.TestCase):
    def run_adapter(self, value, *arguments):
        return subprocess.run(
            [sys.executable, str(ADAPTER), *arguments],
            input=value,
            capture_output=True,
            check=False,
            timeout=5,
        )

    def test_success_returns_only_coarse_public_facts(self):
        private = (
            f"vless://{UUID}@private-marker.invalid:443#Private%20Name"
        ).encode()
        result = self.run_adapter(private)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stderr, b"")
        self.assertEqual(json.loads(result.stdout), {
            "accepted": True,
            "classification": "accepted",
            "host_kind": "dns",
            "standard_https_port": True,
            "label_kind": "ascii",
            "label_sanitized": False,
            "label_truncated": False,
        })
        for marker in (b"private-marker", b"Private", UUID.encode()):
            self.assertNotIn(marker, result.stdout)

    def test_errors_never_echo_private_input(self):
        private = b"vless://not-a-uuid@private-marker.invalid:private-port"
        result = self.run_adapter(private)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stderr, b"")
        self.assertEqual(json.loads(result.stdout)["classification"], "invalid_link")
        self.assertNotIn(b"private", result.stdout)

    def test_arguments_invalid_utf8_and_oversize_fail_safely(self):
        argument = self.run_adapter(b"", "private-argument")
        self.assertEqual(argument.returncode, 2)
        self.assertNotIn(b"private-argument", argument.stderr)
        for value in (b"\xff", b"x" * (64 * 1024 + 1)):
            with self.subTest(size=len(value)):
                result = self.run_adapter(value)
                self.assertEqual(result.returncode, 0)
                self.assertEqual(
                    json.loads(result.stdout)["classification"], "invalid_input"
                )


if __name__ == "__main__":
    unittest.main()
