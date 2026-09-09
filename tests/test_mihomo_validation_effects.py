"""Explicit installed-core counterexample: -t is not a no-network sandbox.

Only a loopback synthetic server and a disposable private directory are used.
No private profile, installed configuration, route, DNS or TUN is involved.
"""
import http.server
import os
from pathlib import Path
import subprocess
import tempfile
import threading
import unittest


class ValidationEffects(unittest.TestCase):
    def test_config_test_can_attempt_geodata_download(self):
        configured = os.environ.get("OMAVLESS_TEST_MIHOMO", "")
        if not configured:
            self.skipTest("installed Mihomo opt-in not enabled")
        binary = Path(configured)
        self.assertTrue(binary.is_absolute() and binary.is_file())
        requested = threading.Event()

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                requested.set()
                self.send_response(503)
                self.send_header("Content-Length", "0")
                self.end_headers()

            def log_message(self, *_args):
                pass

        with tempfile.TemporaryDirectory(prefix="omavless-validation-effects-") as temporary:
            root = Path(temporary)
            server = http.server.HTTPServer(("127.0.0.1", 0), Handler)
            worker = threading.Thread(target=server.serve_forever, daemon=True)
            worker.start()
            try:
                config = root / "candidate.yaml"
                config.write_text(
                    "mode: rule\nipv6: false\ngeodata-mode: true\n"
                    "geox-url:\n"
                    f"  geosite: http://127.0.0.1:{server.server_port}/synthetic.dat\n"
                    "rules:\n  - GEOSITE,cn,DIRECT\n  - MATCH,DIRECT\n",
                    encoding="utf-8",
                )
                config.chmod(0o600)
                before = config.read_bytes()
                # Strip Mihomo-specific overrides and proxies. The only
                # configured download target is this test's loopback server.
                environment = {key: value for key, value in os.environ.items()
                               if not key.startswith(("CLASH_", "MIHOMO_"))
                               and not key.lower().endswith("_proxy")}
                result = subprocess.run(
                    [str(binary), "-t", "-d", str(root), "-f", str(config)],
                    cwd=root, env=environment, stdin=subprocess.DEVNULL,
                    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                    timeout=10, check=False,
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertTrue(requested.is_set(), "-t download attempt was not observed")
                self.assertEqual(config.read_bytes(), before)
            finally:
                server.shutdown()
                server.server_close()
                worker.join(timeout=2)
                self.assertFalse(worker.is_alive())


if __name__ == "__main__":
    unittest.main()
