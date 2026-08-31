import json
import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ADAPTER = ROOT / "tools" / "vless_encryption_parity.py"
UUID = "11111111-1111-4111-8111-111111111111"
KEY = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8"


class VlessEncryptionParityAdapterTests(unittest.TestCase):
    def run_adapter(self, value, *arguments):
        return subprocess.run(
            [sys.executable, str(ADAPTER), *arguments],
            input=value,
            capture_output=True,
            check=False,
            timeout=5,
        )

    def test_success_returns_only_bounded_public_encryption_facts(self):
        encryption = (
            "mlkem768x25519plus.random.1rtt."
            f"100-35-100.75-0-50.50-0-200.{KEY}"
        )
        private = (
            f"vless://{UUID}@private-marker.invalid:443"
            f"?encryption={encryption}#Private"
        ).encode()
        result = self.run_adapter(private)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stderr, b"")
        self.assertEqual(json.loads(result.stdout), {
            "accepted": True,
            "classification": "accepted",
            "enabled": True,
            "mode": "random",
            "rtt": "1rtt",
            "key_count": 1,
            "large_key_present": False,
            "padding_count": 3,
            "total_padding": 300,
        })
        for marker in (b"private", b"Private", UUID.encode(), KEY.encode(), b"mlkem"):
            self.assertNotIn(marker, result.stdout)

    def test_dynamic_errors_are_mapped_without_echoing_private_input(self):
        for encryption, classification in (
            ("private-secret", "unsupported_encryption"),
            ("mlkem768x25519plus.native.1rtt.private-secret",
             "invalid_encryption_padding"),
        ):
            with self.subTest(classification=classification):
                private = (
                    f"vless://{UUID}@private-marker.invalid:443"
                    f"?encryption={encryption}"
                ).encode()
                result = self.run_adapter(private)
                self.assertEqual(result.returncode, 0)
                self.assertEqual(json.loads(result.stdout)["classification"], classification)
                self.assertNotIn(b"private", result.stdout)
                self.assertEqual(result.stderr, b"")

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
