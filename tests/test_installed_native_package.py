# SPDX-License-Identifier: MIT
"""Effect-free safety/policy tests; never invoke pacman, systemd, sudo or store."""
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("installed_package", Path(__file__).with_name("installed_native_package.py"))
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class Terminal(io.StringIO):
    def isatty(self):
        return True


class PackagePolicyTests(unittest.TestCase):
    def assertRefused(self, code, call):
        with self.assertRaises(gate.Refused) as result:
            call()
        self.assertEqual(str(result.exception), code)

    def test_payload_exact(self):
        gate.validate_members(("\n".join(sorted(gate.PAYLOAD)) + "\n").encode())

    def test_no_install_hook_or_extra_path(self):
        base = "\n".join(sorted(gate.PAYLOAD)) + "\n"
        for suffix in (".INSTALL\n", "etc/sudoers\n", "../outside\n", "/usr/bin/omavless\n"):
            with self.subTest(suffix=suffix):
                self.assertRefused("archive_members_invalid", lambda: gate.validate_members((base + suffix).encode()))

    def test_duplicate_member(self):
        self.assertRefused("archive_members_invalid", lambda: gate.validate_members(b"usr/bin/omavless\nusr/bin/omavless\n"))

    def test_missing_member(self):
        self.assertRefused("archive_members_invalid", lambda: gate.validate_members(b"usr/bin/omavless\n"))

    def test_special_files_and_setid_refused(self):
        for mode in ("lrwxrwxrwx", "-rwsr-xr-x", "crw-r--r--", "-rwxrwxrwx"):
            self.assertRefused("archive_member_type", lambda: gate.validate_listing(
                f"{mode} 0 root root 40 Sep 11 23:57 usr/bin/omavless\n".encode()))

    def test_root_normal_payload_modes(self):
        for mode in ("drwxr-xr-x", "-rw-r--r--", "-rwxr-xr-x"):
            gate.validate_listing(f"{mode} 0 root root 40 Sep 11 23:57 usr/bin/omavless\n".encode())

    def test_metadata_duplicates_rejected(self):
        self.assertRefused("metadata_duplicate", lambda: gate.key_values(
            b"pkgname = omavless\npkgname = hostile\n", " = ", {"pkgname"}))

    def test_metadata_required_and_exact(self):
        self.assertRefused("metadata_missing", lambda: gate.key_values(b"", "=", {"schemaVersion"}))
        self.assertRefused("metadata_invalid", lambda: gate.key_values(b"arbitrary=x\n", "=", set(), True))

    def test_raw_private_input_never_echoed(self):
        marker = "https://private.invalid/password-secret"
        for data in (marker.encode(), b"\xff" + marker.encode()):
            with self.assertRaises(gate.Refused) as result:
                gate.key_values(data, "=", {"schemaVersion"}, True)
            self.assertNotIn(marker, str(result.exception))

    def identity(self, schema="2", product="0.8.0-rc.1"):
        raw = (f"schemaVersion={schema}\nsourceCommit={'a' * 40}\nbinarySha256={'b' * 64}\n"
               "architecture=aarch64\nprovenance=caller-supplied-prebuilt\n")
        if schema == "2":
            raw += f"productVersion={product}\n"
        return raw.encode()

    def test_candidate_identity_matches_exact_package_version(self):
        value = gate.validate_build_identity(dict(arch="aarch64", pkgver="0.8.0rc1-1"), self.identity())
        self.assertEqual(value["productVersion"], "0.8.0-rc.1")
        for pkgver in ("0.8.0-1", "0.8.0rc2-1", "0.8.0rc1-2", "0.0.0.r1.gaaaaaaaaaaaa-1"):
            self.assertRefused("package_version", lambda: gate.validate_build_identity(
                dict(arch="aarch64", pkgver=pkgver), self.identity()))

    def test_legacy_identity_keeps_source_prefix_guard(self):
        package = dict(arch="aarch64", pkgver="0.0.0.r1.gaaaaaaaaaaaa-1")
        gate.validate_build_identity(package, self.identity("1"))
        self.assertRefused("build_identity", lambda: gate.validate_build_identity(
            dict(package, pkgver="0.0.0.r1.gbbbbbbbbbbbb-1"), self.identity("1")))

    def test_candidate_identity_rejects_extra_duplicate_unsafe_and_stable_versions(self):
        package = dict(arch="aarch64", pkgver="0.8.0rc1-1")
        for raw in (self.identity() + b"arbitrary=value\n",
                    self.identity() + b"productVersion=0.8.0-rc.1\n", self.identity("3"),
                    self.identity(product="0.8.0"), self.identity(product="private-secret;false"),
                    self.identity().replace(b"aarch64", b"x86_64")):
            with self.assertRaises(gate.Refused) as raised:
                gate.validate_build_identity(package, raw)
            self.assertNotIn("private-secret", str(raised.exception))

    def test_private_file_regular_owned_0600_and_no_symlink(self):
        with tempfile.TemporaryDirectory() as folder, patch.object(gate, "safe_parents"):
            target = Path(folder, "private.json")
            target.write_bytes(b"synthetic only")
            target.chmod(0o600)
            first = gate.fingerprint(target, gate.os.getuid(), True)
            self.assertEqual(first[1:], (0o600, gate.os.getuid()))
            target.chmod(0o644)
            self.assertRefused("unsafe_mode", lambda: gate.fingerprint(target, gate.os.getuid(), True))
            target.chmod(0o600)
            link = Path(folder, "link")
            link.symlink_to(target)
            with self.assertRaises(OSError):
                gate.fingerprint(link, gate.os.getuid(), True)

    def test_clean_observation_is_strict(self):
        facts = dict(ownedCoreRunning=False, visibleMihomoCount=0, visibleTunCount=0, ownedAuxiliaryMihomoCount=0)
        value = dict(availability="observed", lastKnownActual="disconnected", manualRecoveryRequired=False,
                     desired=dict(connected=False), facts=facts)
        self.assertTrue(gate.clean_observation(value))
        for recovery in (True, None, "false", 0):
            self.assertFalse(gate.clean_observation(dict(value, manualRecoveryRequired=recovery)))
        for count in (False, None, "0", 1):
            self.assertFalse(gate.clean_observation(dict(value, facts=dict(facts, visibleTunCount=count))))

    def test_package_phase_has_real_terminal_guard(self):
        auth = gate.PackageAuthorization(io.StringIO("ready\nsettled\n"), Terminal())
        called = []
        with self.assertRaises(gate.auth.AuthorizationUnsettled):
            auth.step("package_install", lambda: called.append(True))
        self.assertFalse(called)

    def test_after_cancel_no_recovery_effect(self):
        auth = gate.PackageAuthorization(Terminal("ready\nstop\nready\nsettled\n"), Terminal())
        called = []
        with self.assertRaises(gate.auth.AuthorizationUnsettled):
            auth.step("package_remove", lambda: called.append("remove"))
        with self.assertRaises(gate.auth.AuthorizationUnsettled):
            auth.step("package_install", lambda: called.append("recover"))
        self.assertEqual(called, ["remove"])

    def test_unknown_privileged_phase_refused(self):
        auth = gate.PackageAuthorization(Terminal("ready\nsettled\n"), Terminal())
        with self.assertRaises(gate.auth.AuthorizationUnsettled):
            auth.step("arbitrary_shell", lambda: self.fail("must not run"))

    def test_package_failure_has_no_implicit_recovery(self):
        current = dict(path=Path("/safe/current.pkg.tar.zst"), version="current", binary="new", source="n", fingerprint=("a", 0o600, 1000))
        rollback = dict(path=Path("/safe/old.pkg.tar.zst"), version="old", binary="old", source="o", fingerprint=("b", 0o600, 1000))
        instance = gate.PackageGate(current, rollback)
        events = []
        with patch.object(instance.authorization, "require_terminal"), \
             patch.object(gate.os, "getuid", return_value=1000), \
             patch.dict(gate.os.environ, {"HOME": str(instance.home), "XDG_RUNTIME_DIR": "/run/user/1000"}), \
             patch.object(gate, "private_snapshot", return_value={}), \
             patch.object(instance, "check_running"), patch.object(gate, "unit", return_value="enabled"), \
             patch.object(gate, "capture", return_value=b"-1"), patch.object(gate, "emit"), \
             patch.object(instance, "stop", side_effect=lambda value: events.append("stop")), \
             patch.object(instance, "pacman", side_effect=gate.Refused("package_transaction_failed")), \
             patch.object(instance, "start", side_effect=lambda value: events.append("start")):
            self.assertRefused("package_transaction_failed", instance.run)
        self.assertEqual(events, ["stop"])

    def test_pacman_exact_commands_keep_dependency_checks(self):
        current = dict(path=Path("/safe/current.pkg.tar.zst"))
        instance = gate.PackageGate(current, current)
        auth = gate.PackageAuthorization(Terminal("ready\nsettled\nready\nsettled\n"), Terminal())
        instance.authorization = auth
        with patch.object(instance, "stopped"), patch.object(instance, "archive_unchanged"), \
             patch.object(instance, "unchanged"), patch.object(gate.subprocess, "run") as run:
            run.return_value.returncode = 0
            instance.pacman(current)
            instance.pacman()
        self.assertEqual([call.args[0] for call in run.call_args_list], [
            ["/usr/bin/sudo", "/usr/bin/pacman", "-U", "--", "/safe/current.pkg.tar.zst"],
            ["/usr/bin/sudo", "/usr/bin/pacman", "-R", "--", "omavless"]])
        self.assertTrue(all("timeout" not in call.kwargs for call in run.call_args_list))

    def test_changed_recovery_archive_blocks_package_action(self):
        package = dict(path=Path("/safe/current.pkg.tar.zst"), fingerprint=("old", 0o600, 1000))
        instance = gate.PackageGate(package, package)
        with patch.object(gate, "fingerprint", return_value=("changed", 0o600, 1000)):
            self.assertRefused("archive_changed", lambda: instance.archive_unchanged(package))

    def test_removed_recovery_does_not_query_missing_runtime_unit(self):
        current = dict(path=Path("/safe/current.pkg.tar.zst"))
        instance = gate.PackageGate(current, current)
        instance.removed = True
        instance.authorization = gate.PackageAuthorization(Terminal("ready\nsettled\n"), Terminal())
        with patch.object(instance, "stopped", side_effect=AssertionError("missing unit queried")), \
             patch.object(instance, "absent") as absent, patch.object(instance, "archive_unchanged"), \
             patch.object(instance, "unchanged"), patch.object(gate.subprocess, "run") as run:
            run.return_value.returncode = 0
            instance.pacman(current)
        absent.assert_called_once()
        self.assertEqual(run.call_args.args[0], ["/usr/bin/sudo", "/usr/bin/pacman", "-U", "--", "/safe/current.pkg.tar.zst"])

    def test_removed_state_only_accepts_exact_retained_current(self):
        instance = gate.PackageGate({}, {})
        instance.removed = True
        self.assertRefused("recovery_candidate_only", lambda: instance.pacman(instance.rollback))


if __name__ == "__main__":
    unittest.main()
