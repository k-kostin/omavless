import json
import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ADAPTER = ROOT / "tools" / "vless_query_metadata_parity.py"
UUID = "11111111-1111-4111-8111-111111111111"


class VlessQueryMetadataParityAdapterTests(unittest.TestCase):
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
            f"vless://{UUID}@private-marker.invalid:443"
            "?type=ws&security=tls&host=private-host.invalid#Private"
        ).encode()
        result = self.run_adapter(private)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stderr, b"")
        self.assertEqual(json.loads(result.stdout), {
            "accepted": True,
            "classification": "accepted",
            "transport": "ws",
            "security": "tls",
            "allow_insecure": False,
            "xhttp_mode": "none",
            "provider_metadata_present": False,
            "non_xhttp_mode_metadata": False,
        })
        for marker in (b"private-marker", b"private-host", b"Private", UUID.encode()):
            self.assertNotIn(marker, result.stdout)

    def test_dynamic_errors_are_mapped_without_echoing_private_input(self):
        for query, classification in (
            ("type=private-transport", "unsupported_transport"),
            ("security=private-security", "unsupported_security"),
            ("type=xhttp&mode=private-mode", "unsupported_xhttp_mode"),
        ):
            with self.subTest(classification=classification):
                private = (
                    f"vless://{UUID}@private-marker.invalid:443?{query}"
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
