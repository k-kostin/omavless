"""Static candidate checks only: never installs, starts or reloads a service."""
import configparser
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


UNIT = Path(__file__).with_name("omavless-dns-broker.service")


class CandidateUnitTests(unittest.TestCase):
    def setUp(self):
        self.text = UNIT.read_text(encoding="utf-8")
        self.config = configparser.ConfigParser(interpolation=None, strict=True)
        self.config.read_string(self.text)
        self.service = self.config["Service"]

    def test_matches_root_context_and_retention_contract(self):
        for key, value in {
            "Type": "notify", "NotifyAccess": "main", "User": "root",
            "Group": "root", "UMask": "0077", "FileDescriptorStoreMax": "1",
            "FileDescriptorStorePreserve": "yes", "RuntimeDirectoryPreserve": "yes",
            "RuntimeDirectory": "omavless-dns", "RuntimeDirectoryMode": "0711",
            "ExecStart": "/usr/lib/omavless/omavless-dns-broker --serve",
            "ExecStartPre": "/usr/bin/install -d -o root -g root -m 0700 /run/omavless-dns/private",
        }.items():
            self.assertEqual(self.service[key], value, key)

    def test_no_automatic_recovery_or_boot_enable(self):
        self.assertEqual(self.service["Restart"], "no")
        self.assertNotIn("Install", self.config)
        for forbidden in ("ExecStop", "ExecStopPost", "WatchdogSec", "ExecReload"):
            self.assertNotIn(forbidden, self.service)
        for value in self.service.values():
            self.assertNotIn("sudo", value)
            self.assertNotIn("pkexec", value)
            self.assertNotIn("/bin/sh", value)
            self.assertNotIn("setfacl", value)
            self.assertNotIn("rm ", value)

    def test_original_namespace_and_explicit_inspection_capability(self):
        self.assertEqual(self.service["CapabilityBoundingSet"], "CAP_NET_ADMIN")
        self.assertEqual(self.service["NoNewPrivileges"], "yes")
        self.assertEqual(self.service["RestrictAddressFamilies"], "AF_UNIX")
        self.assertEqual(self.service["PrivateNetwork"], "no")
        self.assertEqual(self.service["PrivateUsers"], "no")
        self.assertNotIn("ProcSubset", self.service)  # journal reads boot_id

    def test_resource_bounds_and_root_fs_protection(self):
        for key, value in {
            "MemoryHigh": "192M", "MemoryMax": "256M", "MemorySwapMax": "0",
            "TasksMax": "32", "LimitNOFILE": "128", "ProtectSystem": "strict",
            "ProtectHome": "yes", "ReadWritePaths": "/run/omavless-dns",
        }.items():
            self.assertEqual(self.service[key], value, key)

    @unittest.skipUnless(shutil.which("systemd-analyze"), "systemd parser unavailable")
    def test_systemd_syntax_using_inert_executable_substitution(self):
        # verify parses, never starts. Candidate binary is intentionally not installed.
        # Substitute only ExecStart for this syntax check; fixed production path above
        # is separately asserted. This is NOT installed-binary or DNS acceptance.
        parsed = self.text.replace(
            "ExecStart=/usr/lib/omavless/omavless-dns-broker --serve",
            "ExecStart=/usr/bin/true",
        )
        with tempfile.TemporaryDirectory(prefix="omavless-unit-lint-") as directory:
            candidate = Path(directory) / UNIT.name
            candidate.write_text(parsed, encoding="utf-8")
            result = subprocess.run(
                ["systemd-analyze", "verify", "--man=no", "--generators=no", str(candidate)],
                stdin=subprocess.DEVNULL, capture_output=True, timeout=20,
                env={"PATH": "/usr/bin:/bin", "LANG": "C"}, check=False,
            )
        self.assertEqual(result.returncode, 0, "Candidate unit syntax was rejected")


if __name__ == "__main__":
    unittest.main()
