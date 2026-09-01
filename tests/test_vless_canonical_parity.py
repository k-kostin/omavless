from __future__ import annotations

import json
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ADAPTER = ROOT / "tools" / "vless_canonical_parity.py"
UUID = "11111111-1111-4111-8111-111111111111"


class VlessCanonicalParityAdapterTests(unittest.TestCase):
    def run_adapter(self, payload: bytes, *args: str) -> subprocess.CompletedProcess[bytes]:
        return subprocess.run(
            [sys.executable, str(ADAPTER), *args],
            input=payload,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            timeout=30,
        )

    def test_batch_output_is_bounded_and_contains_only_safe_facts(self) -> None:
        uri = (
            f"vless://{UUID}@private.invalid:443?type=ws&security=tls"
            "&sni=sni.private.invalid&host=host.private.invalid&path=%2Fprivate#Node"
        )
        payload = json.dumps({
            "cases": [{
                "id": "private-case",
                "uri": uri,
                "name": "Private Node",
                "server_override": None,
            }]
        }).encode()
        result = self.run_adapter(payload)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stderr, b"")
        self.assertLess(len(result.stdout), 16 * 1024)
        outcome = json.loads(result.stdout)["cases"][0]
        self.assertTrue(outcome["accepted"])
        self.assertEqual(outcome["classification"], "accepted")
        self.assertRegex(outcome["identity_fingerprint"], r"^[0-9a-f]{64}$")
        self.assertRegex(outcome["preview_fingerprint"], r"^[0-9a-f]{64}$")
        self.assertRegex(outcome["mihomo_fingerprint"], r"^[0-9a-f]{64}$")
        public = (result.stdout + result.stderr).decode()
        for marker in ("vless://", UUID, "private.invalid", "/private"):
            self.assertNotIn(marker, public)

    def test_private_error_input_is_not_echoed(self) -> None:
        uri = f"vless://{UUID}@private.invalid:443?type=ws&extra=%7B%7D"
        payload = json.dumps({
            "cases": [{
                "id": "rejected-case",
                "uri": uri,
                "name": "Private Node",
                "server_override": None,
            }]
        }).encode()
        result = self.run_adapter(payload)
        self.assertEqual(result.returncode, 0)
        outcome = json.loads(result.stdout)["cases"][0]
        self.assertFalse(outcome["accepted"])
        self.assertEqual(outcome["classification"], "xhttp_extra_requires_transport")
        public = (result.stdout + result.stderr).decode()
        for marker in ("vless://", UUID, "private.invalid"):
            self.assertNotIn(marker, public)

    def test_arguments_and_malformed_envelopes_fail_without_output(self) -> None:
        with_arg = self.run_adapter(b"{}", "unexpected")
        self.assertNotEqual(with_arg.returncode, 0)
        self.assertEqual(with_arg.stdout, b"")
        self.assertEqual(with_arg.stderr, b"")

        malformed = self.run_adapter(b'{"cases":"not-a-list"}')
        self.assertNotEqual(malformed.returncode, 0)
        self.assertEqual(malformed.stdout, b"")
        self.assertEqual(malformed.stderr, b"")


if __name__ == "__main__":
    unittest.main()
