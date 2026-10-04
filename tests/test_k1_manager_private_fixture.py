"""Source-only fixture admission/package boundary; no manager or netlink calls."""
import configparser
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
CRATE = ROOT / "crates/omavless-netguard"


class ManagerPrivateFixture(unittest.TestCase):
    def test_fixed_unit_has_no_automatic_kill_restart_or_namespace_join(self):
        parser = configparser.ConfigParser(interpolation=None, strict=True)
        parser.optionxform = str
        parser.read(CRATE / "tests/fixtures/omavless-k1-manager-private-lifecycle.service")
        self.assertEqual(parser.sections(), ["Unit", "Service"])
        self.assertEqual(dict(parser.defaults()), {})
        self.assertEqual(dict(parser["Unit"]), {
            "Description": "OmaVLESS K1 isolated manager-created lifecycle fixture",
        })
        self.assertEqual(dict(parser["Service"]), {
            "Type": "oneshot", "RemainAfterExit": "yes", "User": "root", "Group": "root",
            "SupplementaryGroups": "",
            "ExecStart": "/run/omavless-k1-manager-private-lifecycle/probe --exact kernel_observer::creator_lifecycle::manager_private::manager_private_lifecycle --ignored --nocapture --test-threads=1",
            "Environment": "OMAVLESS_K1_MANAGER_PRIVATE=1",
            "OpenFile": "/proc/1/ns/net:k1-host-netns:read-only",
            "PrivateUsers": "no", "PrivatePIDs": "no", "PrivateNetwork": "yes",
            "RestrictNamespaces": "yes", "NoNewPrivileges": "yes",
            "CapabilityBoundingSet": "CAP_NET_ADMIN", "AmbientCapabilities": "",
            "UMask": "0077", "Restart": "no", "TimeoutStartSec": "infinity",
            "TimeoutStopSec": "infinity", "RuntimeMaxSec": "infinity",
            "KillMode": "none", "SendSIGKILL": "no", "StandardInput": "null",
            "StandardOutput": "append:/run/omavless-k1-manager-private-lifecycle/native.stdout",
            "StandardError": "append:/run/omavless-k1-manager-private-lifecycle/native.stderr",
        })

    def test_native_gate_precedes_socket_and_uses_existing_creator(self):
        text = (CRATE / "src/kernel_manager_private_fixture.rs").read_text()
        body = text[text.index("fn run("):]
        self.assertLess(body.index("held.isolation.recheck()?"), body.index("FixtureCreator::open("))
        for token in ["LockedState::from_root", "RootStateStore::open_test_parent",
                      "inspect_policy_inventory", ".request(ARM", ".request(DISARM",
                      "independent.observe().is_err()", "Box::leak", "catch_unwind",
                      "std::thread::park()", "K1_MANAGER_PRIVATE_UNCERTAIN_RETAINED"]:
            self.assertIn(token, text)
        for forbidden in ["Command::", "unshare(", "setns(&self.host", "remove_dir", "remove_file", "unsafe ", "kill("]:
            self.assertNotIn(forbidden, text)
        observer = (CRATE / "src/kernel_observer.rs").read_text()
        self.assertIn('#[cfg(test)]\n#[path = "kernel_creator_lifecycle.rs"]', observer)
        self.assertIn('#[ignore = "fixed manager-owned PrivateNetwork fixture;', text)

    def test_no_package_or_product_caller(self):
        for path in (ROOT / "packaging").rglob("*"):
            if path.is_file():
                self.assertNotIn(b"manager-private-lifecycle", path.read_bytes())
        self.assertNotIn("manager_private", (CRATE / "src/lib.rs").read_text())


if __name__ == "__main__":
    unittest.main()
