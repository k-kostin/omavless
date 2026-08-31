import json
import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ADAPTER = ROOT / "tools" / "vless_transport_parity.py"
UUID = "11111111-1111-4111-8111-111111111111"


class VlessTransportParityAdapterTests(unittest.TestCase):
    def run_adapter(self, payload: bytes, *arguments: str) -> subprocess.CompletedProcess[bytes]:
        return subprocess.run(
            [sys.executable, str(ADAPTER), *arguments],
            input=payload,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def test_success_returns_only_bounded_public_transport_facts(self):
        private = "private-marker"
        uri = (
            f"vless://{UUID}@example.invalid:443?type=ws"
            f"&path=%252F&host={private}&serviceName={private}"
            f"&fp={private}&alpn=h2%2C%D1%82%D0%B5%D1%81%D1%82"
        ).encode()
        result = self.run_adapter(uri)
        self.assertEqual(result.returncode, 0)
        payload = json.loads(result.stdout)
        self.assertEqual(payload["classification"], "accepted")
        self.assertTrue(payload["path_default"])
        self.assertTrue(payload["host_present"])
        self.assertTrue(payload["service_name_present"])
        self.assertTrue(payload["fingerprint_present"])
        self.assertEqual(payload["alpn_count"], 2)
        self.assertTrue(payload["alpn_non_ascii"])
        public = result.stdout.decode()
        self.assertNotIn(private, public)
        self.assertNotIn(UUID, public)
        self.assertNotIn("vless://", public)

    def test_dynamic_errors_do_not_echo_private_values(self):
        private = "private-secret-header"
        uri = f"vless://{UUID}@example.invalid:443?headerType={private}".encode()
        result = self.run_adapter(uri)
        self.assertEqual(result.returncode, 0)
        payload = json.loads(result.stdout)
        self.assertEqual(payload["classification"], "unsupported_tcp_header")
        self.assertNotIn(private, result.stdout.decode())
        self.assertNotIn(UUID, result.stdout.decode())

    def test_arguments_invalid_utf8_and_oversize_fail_safely(self):
        usage = self.run_adapter(b"", "unexpected")
        self.assertEqual(usage.returncode, 2)
        self.assertEqual(usage.stdout, b"")
        invalid = self.run_adapter(b"\xffprivate")
        self.assertEqual(json.loads(invalid.stdout)["classification"], "invalid_input")
        oversized = self.run_adapter(b"x" * (1024 * 1024 + 1))
        self.assertEqual(json.loads(oversized.stdout)["classification"], "invalid_input")
        for result in (invalid, oversized):
            self.assertLess(len(result.stdout), 512)
            self.assertNotIn(b"private", result.stdout)


if __name__ == "__main__":
    unittest.main()
