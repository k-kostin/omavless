import json
import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ADAPTER = ROOT / "tools" / "vless_reality_parity.py"
UUID = "11111111-1111-4111-8111-111111111111"
PUBLIC_KEY = "A" * 43


class VlessRealityParityAdapterTests(unittest.TestCase):
    def run_adapter(self, value, *arguments):
        return subprocess.run(
            [sys.executable, str(ADAPTER), *arguments],
            input=value,
            capture_output=True,
            check=False,
            timeout=5,
        )

    def test_success_returns_only_public_reality_facts(self):
        private = (
            f"vless://{UUID}@private-marker.invalid:443?security=reality"
            f"&sni=private-sni.invalid&pbk={PUBLIC_KEY}&sid=0123456789abcdef"
            "&spx=%2Fprivate&supportX25519MLKEM768=true#Private"
        ).encode()
        result = self.run_adapter(private)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stderr, b"")
        self.assertEqual(json.loads(result.stdout), {
            "accepted": True,
            "classification": "accepted",
            "reality": True,
            "pq": True,
            "pq_present": True,
            "short_id_present": True,
            "spider_x_present": True,
        })
        for marker in (
            b"private-marker", b"private-sni", b"private", b"Private",
            UUID.encode(), PUBLIC_KEY.encode(), b"0123456789abcdef",
        ):
            self.assertNotIn(marker, result.stdout)

    def test_dynamic_errors_are_mapped_without_echoing_private_input(self):
        for query, classification in (
            ("security=reality&sni=example.invalid&pbk=private-key",
             "invalid_reality_public_key"),
            (f"security=reality&sni=example.invalid&pbk={PUBLIC_KEY}"
             "&mldsa65Verify=private-verifier", "reality_mldsa_unsupported"),
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
