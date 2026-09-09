# SPDX-License-Identifier: MIT
"""Pure staged-unit harness tests; never query or modify the user manager."""
import importlib.util
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


TESTS = Path(__file__).resolve().parent
HELPER_SPEC = importlib.util.spec_from_file_location("staged_test_helpers", TESTS / "native_service_acceptance.py")
HELPERS = importlib.util.module_from_spec(HELPER_SPEC)
HELPER_SPEC.loader.exec_module(HELPERS)
SPEC = importlib.util.spec_from_file_location("staged_unit_probe", TESTS / "staged_native_unit_acceptance.py")
PROBE = importlib.util.module_from_spec(SPEC)
with patch.dict(sys.modules, {"native_service_acceptance": HELPERS}):
    SPEC.loader.exec_module(PROBE)


class StagedNativeUnitTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="omavless-staged-unit-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.bases = {name: self.root / name for name in ("config", "state", "cache", "runtime")}
        self.packaged = (TESTS.parent / "packaging/systemd/omavless-runtime.service").read_text()
        self.namespace = "omavless-stage-012345abcdef"

    def test_manager_defaults_use_account_home_without_copying_secrets(self):
        bases = PROBE.manager_bases(b"UNRELATED_SECRET=not-copied\n", 1234, "/home/synthetic")
        self.assertEqual(bases, {
            "config": Path("/home/synthetic/.config"),
            "state": Path("/home/synthetic/.local/state"),
            "cache": Path("/home/synthetic/.cache"),
            "runtime": Path("/run/user/1234"),
        })

    def test_manager_custom_absolute_bases_are_respected(self):
        raw = b"HOME=/home/synthetic\nXDG_CONFIG_HOME='/tmp/custom-config'\nXDG_STATE_HOME=/tmp/custom-state\n"
        bases = PROBE.manager_bases(raw, 1234, "/home/synthetic")
        self.assertEqual(bases["config"], Path("/tmp/custom-config"))
        self.assertEqual(bases["state"], Path("/tmp/custom-state"))

    def test_manager_relative_or_ambiguous_paths_are_refused(self):
        for raw in (b"HOME=/home/other\n", b"XDG_STATE_HOME=relative\n",
                    b"XDG_CACHE_HOME=/tmp/cache extra\n", b"XDG_RUNTIME_DIR=/tmp/%t\n"):
            with self.assertRaises(HELPERS.Failure):
                PROBE.manager_bases(raw, 1234, "/home/synthetic")

    def test_canonical_refuses_symlinks_and_unit_expansion_characters(self):
        original = self.root / "directory"
        original.mkdir()
        alias = self.root / "alias"
        alias.symlink_to(original)
        for value in (alias, "relative", "/tmp/%t", "/tmp/$USER", "/tmp/line\nbreak", "/tmp/space here"):
            with self.assertRaises(HELPERS.Failure):
                PROBE.canonical(value)

    def test_unit_preserves_every_nonoverridden_packaged_line(self):
        text, environment = PROBE.render_unit(self.packaged, self.root / "omavless", self.namespace, self.bases)
        replaced = {"ExecStart", "ConditionFileIsExecutable", "ConfigurationDirectory",
                    "StateDirectory", "CacheDirectory", "RuntimeDirectory"}
        for line in self.packaged.splitlines():
            if line.split("=", 1)[0] not in replaced:
                self.assertIn(line, text.splitlines())
        self.assertIn("RuntimeDirectory=" + self.namespace + "/omavless\n", text)
        self.assertIn("ConfigurationDirectory=" + self.namespace + "/.config/omavless\n", text)
        self.assertEqual(environment["OMAVLESS_HOME"], str(self.bases["config"] / self.namespace))
        self.assertEqual(environment["XDG_STATE_HOME"], str(self.bases["state"] / self.namespace))
        self.assertEqual(environment["XDG_RUNTIME_DIR"], str(self.bases["runtime"] / self.namespace))
        self.assertNotIn("Environment=HOME=", text)
        self.assertEqual(set(environment), {"OMAVLESS_HOME", "XDG_STATE_HOME", "XDG_CACHE_HOME",
                                           "XDG_RUNTIME_DIR", "OMAVLESS_MIHOMO"})

    def test_unit_rejects_missing_or_duplicate_owned_directives(self):
        for source in (self.packaged.replace("RuntimeDirectory=omavless\n", ""),
                       self.packaged + "\nRuntimeDirectory=another\n"):
            with self.assertRaises(HELPERS.Failure):
                PROBE.render_unit(source, self.root / "omavless", self.namespace, self.bases)

    def test_unit_rejects_nonfixture_namespace(self):
        for namespace in ("omavless", "../omavless", "omavless-stage-a", "omavless-stage-012345abcdef\nExecStart=x"):
            with self.assertRaises(HELPERS.Failure):
                PROBE.render_unit(self.packaged, self.root / "omavless", namespace, self.bases)

    def test_private_directory_requires_same_user_0700_and_no_symlink(self):
        self.root.chmod(0o700)
        PROBE.private_directory(self.root, os.getuid())
        with self.assertRaises(HELPERS.Failure):
            PROBE.private_directory(self.root, os.getuid() + 1)
        self.root.chmod(0o755)
        with self.assertRaises(HELPERS.Failure):
            PROBE.private_directory(self.root, os.getuid())
        self.root.chmod(0o700)
        alias = self.root / "alias"
        alias.symlink_to(self.root)
        with self.assertRaises(HELPERS.Failure):
            PROBE.private_directory(alias, os.getuid())

    def test_explicit_run_is_required_before_host_work(self):
        with patch.object(sys, "argv", ["staged_native_unit_acceptance.py", "--binary", "/synthetic/omavless"]), \
                patch.object(PROBE, "run") as run:
            with self.assertRaisesRegex(HELPERS.Failure, "^explicit_run_required$"):
                PROBE.main()
            run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
