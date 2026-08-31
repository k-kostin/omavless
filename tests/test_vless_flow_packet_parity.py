import json
import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ADAPTER = ROOT / "tools" / "vless_flow_packet_parity.py"
UUID = "11111111-1111-4111-8111-111111111111"


class VlessFlowPacketParityAdapterTests(unittest.TestCase):
    def run_adapter(self, value, *arguments):
        return subprocess.run(
            [sys.executable, str(ADAPTER), *arguments],
            input=value,
            capture_output=True,
            check=False,
            timeout=5,
        )

    def test_success_returns_only_public_flow_and_packet_vocabulary(self):
        private = (
            f"vless://{UUID}@private-marker.invalid:443"
            "?type=tcp&security=tls&flow=xtls-rprx-vision-udp443"
            "&packetEncoding=xudp#Private"
        ).encode()
        result = self.run_adapter(private)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stderr, b"")
        self.assertEqual(json.loads(result.stdout), {
            "accepted": True,
            "classification": "accepted",
            "flow": "xtls-rprx-vision-udp443",
            "mihomo_flow": "xtls-rprx-vision",
            "packet_encoding": "xudp",
        })
        for marker in (b"private-marker", b"Private", UUID.encode()):
            self.assertNotIn(marker, result.stdout)

    def test_dynamic_errors_are_mapped_without_echoing_private_input(self):
        for query, classification in (
            ("flow=private-flow", "unsupported_flow"),
            ("packetEncoding=private-packet", "unsupported_packet_encoding"),
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
