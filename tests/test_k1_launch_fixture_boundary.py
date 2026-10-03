"""Offline launch/package boundary checks; never load or start a system unit."""
import configparser
import hashlib
from pathlib import Path
import stat
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = (ROOT / "crates/omavless-netguard/tests/fixtures/"
           "omavless-k1-openfile-fixture.service")
UNIT_RUNNER = (ROOT / "crates/omavless-netguard/tests/support/"
               "openfile_unit_vm_fixture.sh")
NEGATIVE_DROPIN = (ROOT / "crates/omavless-netguard/tests/fixtures/"
                   "omavless-k1-openfile-negative-dropin.conf")


class K1LaunchFixtureBoundaryTests(unittest.TestCase):
    def test_namespace_filter_pair_changes_only_filter_and_expectation(self):
        base = ROOT / "crates/omavless-netguard/tests/fixtures"
        filtered = (base / "omavless-k1-namespace-filter-fixture.service").read_text()
        control = (base / "omavless-k1-namespace-filter-control.service").read_text()
        self.assertEqual(control, filtered.replace(
            "FIXTURE=filtered", "FIXTURE=control").replace(
            "RestrictNamespaces=yes", "RestrictNamespaces=no"))
        unit = configparser.ConfigParser(interpolation=None, strict=True)
        unit.optionxform = str
        unit.read_string(filtered)
        self.assertEqual(unit.sections(), ["Unit", "Service"])
        self.assertEqual(dict(unit.defaults()), {})
        self.assertEqual(dict(unit["Service"]), {
            "Type": "exec", "User": "root",
            "ExecStart": "/run/omavless-k1-namespace-filter-fixture/probe",
            "Environment": "OMAVLESS_K1_NAMESPACE_FILTER_FIXTURE=filtered",
            "PrivateUsers": "no", "PrivatePIDs": "no", "PrivateNetwork": "no",
            "NoNewPrivileges": "yes", "CapabilityBoundingSet": "",
            "AmbientCapabilities": "", "RestrictNamespaces": "yes",
            "RuntimeMaxSec": "10s", "TimeoutStopSec": "2s",
        })

    def test_namespace_filter_runner_pins_both_cases_and_checks_effective_filter(self):
        base = ROOT / "crates/omavless-netguard/tests"
        runner = base / "support/namespace_filter_vm_fixture.sh"
        source = runner.read_text()
        for name in ["fixture", "control"]:
            unit = base / f"fixtures/omavless-k1-namespace-filter-{name}.service"
            self.assertIn(hashlib.sha256(unit.read_bytes()).hexdigest(), source)
        for token in ["for mode in control filtered", "-p RestrictNamespaces",
                      "SystemCallFilter", "CapabilityBoundingSet", "AmbientCapabilities",
                      "PrivateUsers PrivatePIDs PrivateNetwork", "DropInPaths",
                      "PropagatesStopTo", "StopPropagatedFrom"]:
            self.assertIn(token, source)
        self.assertLess(source.index("-p RestrictNamespaces"),
                        source.index('systemctl start "$unit"'))
        self.assertEqual(subprocess.run(["bash", "-n", str(runner)],
                                       capture_output=True).returncode, 0)

    def test_vm_unit_runner_pins_fixed_unit_and_has_valid_shell_syntax(self):
        source = UNIT_RUNNER.read_text()
        digest = hashlib.sha256(FIXTURE.read_bytes()).hexdigest()
        self.assertIn(f"expected_unit_sha={digest}", source)
        self.assertIn("systemd-detect-virt --vm", source)
        self.assertIn("OMAVLESS_K1_OPENFILE_UNIT_VM", source)
        self.assertIn("-p DropInPaths", source)
        self.assertIn("-p Requires", source)
        self.assertIn("-p ExecStart", source)
        self.assertIn("for property in DropInPaths Wants", source)
        self.assertIn("start_attempted=0", source)
        self.assertIn("if [[ $start_attempted == 1 ]]", source)
        self.assertLess(source.index("start_attempted=1"),
                        source.index('systemctl start "$unit"'))
        self.assertIn("ln -s \"$source_unit\" \"$linked_unit\"", source)
        self.assertIn("unlink \"$linked_unit\"", source)
        self.assertNotIn("nft ", source)
        self.assertEqual(subprocess.run(["bash", "-n", str(UNIT_RUNNER)],
                                        capture_output=True, text=True).returncode, 0)

    def test_negative_dropin_targets_only_the_owned_synthetic_stop_recipient(self):
        self.assertEqual(NEGATIVE_DROPIN.read_text(),
                         "# SPDX-License-Identifier: MIT\n"
                         "# Benign VM-only negative case: runner must refuse any effective drop-in.\n"
                         "[Unit]\n"
                         "PropagatesStopTo=omavless-k1-openfile-stop-recipient.service\n"
                         "[Service]\n"
                         "Environment=OMAVLESS_K1_UNEXPECTED_DROPIN=1\n")

    def test_fixture_has_only_fixed_bounded_descriptor_inspection(self):
        # Exact allowlist: duplicate/unknown directives, activation sections,
        # extra commands, production paths and capability grants need review.
        unit = configparser.ConfigParser(interpolation=None, strict=True)
        unit.optionxform = str
        unit.read_string(FIXTURE.read_text())
        self.assertEqual(dict(unit.defaults()), {})
        self.assertEqual(unit.sections(), ["Unit", "Service"])
        self.assertEqual(dict(unit["Unit"]), {
            "Description": "OmaVLESS K1 descriptor-match fixture (not protection)",
        })
        self.assertEqual(dict(unit["Service"]), {
            "Type": "exec",
            "User": "root",
            "ExecStart": "/run/omavless-k1-openfile-fixture/probe",
            "Environment": "OMAVLESS_K1_OPENFILE_FIXTURE=1",
            "OpenFile": "/proc/1/ns/net:k1-host-netns:read-only",
            "PrivateUsers": "no",
            "PrivatePIDs": "no",
            "PrivateNetwork": "no",
            "NoNewPrivileges": "yes",
            "CapabilityBoundingSet": "",
            "AmbientCapabilities": "",
            "RuntimeMaxSec": "10s",
            "TimeoutStopSec": "2s",
        })

    def test_actual_arch_stager_excludes_fixture_and_root_activation_payload(self):
        # Run the real stager against the real source tree, so accidentally
        # broadening its copy scope to include this fixture is detectable.
        # No Cargo build, makepkg, root, service manager or installed binary.
        with tempfile.TemporaryDirectory(prefix="omavless-k1-payload-") as temp:
            scratch = Path(temp)
            payload = scratch / "payload"
            payload.mkdir()
            binary = scratch / "synthetic-binary"
            binary.write_bytes(b"synthetic package bytes; must never execute\n")
            binary.chmod(0o700)
            result = subprocess.run(
                ["bash", str(ROOT / "packaging/arch/stage-payload.sh"),
                 str(payload), str(binary)],
                capture_output=True, text=True, timeout=15,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            members = list(payload.rglob("*"))
            self.assertFalse(any(path.is_symlink() for path in members))
            files = {str(path.relative_to(payload)): path for path in members
                     if path.is_file()}
            expected = {
                "usr/bin/omavless": (binary, 0o755),
                "usr/lib/systemd/user/omavless-runtime.service":
                    (ROOT / "packaging/systemd/omavless-runtime.service", 0o644),
                "usr/lib/systemd/user/omavless-login-prepare.service":
                    (ROOT / "packaging/systemd/omavless-login-prepare.service", 0o644),
                "usr/share/licenses/omavless/LICENSE": (ROOT / "LICENSE", 0o644),
                "usr/share/licenses/omavless/THIRD_PARTY_NOTICES.md":
                    (ROOT / "THIRD_PARTY_NOTICES.md", 0o644),
                "usr/share/doc/omavless/README.md":
                    (ROOT / "packaging/arch/README.md", 0o644),
            }
            # A positive complete inventory prevents vacuous absence checks
            # against an empty payload. It also rejects future root units,
            # activation hooks, NetGuard binaries and developer fixture copies.
            self.assertEqual(set(files), set(expected))
            for name, (source, mode) in expected.items():
                with self.subTest(member=name):
                    self.assertEqual(files[name].read_bytes(), source.read_bytes())
                    self.assertEqual(stat.S_IMODE(files[name].stat().st_mode), mode)
            allowed_directories = {
                str(parent) for name in expected
                for parent in Path(name).parents if str(parent) != "."
            }
            self.assertEqual(
                {str(path.relative_to(payload)) for path in members if path.is_dir()},
                allowed_directories,
            )
            self.assertTrue(all(path.is_file() or path.is_dir() for path in members))


if __name__ == "__main__":
    unittest.main()
