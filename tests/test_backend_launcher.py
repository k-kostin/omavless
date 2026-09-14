# SPDX-License-Identifier: MIT
"""Credential-free execution tests for the real frontend launcher."""
import os
from pathlib import Path
import subprocess
import tempfile
import time
import unittest


LAUNCHER = Path(__file__).resolve().parents[1] / "backend.sh"


class LauncherTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="omavless-launcher-")
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.bin = self.base / "bin"
        self.bin.mkdir()
        self.trace = self.base / "trace"
        self.state = self.base / "state"
        self.state.mkdir(mode=0o700)
        self.env = {
            "PATH": str(self.bin), "HOME": str(self.base),
            "XDG_STATE_HOME": str(self.state), "BRIDGE_TEST_TRACE": str(self.trace),
        }
        (self.bin / "dirname").symlink_to("/usr/bin/dirname")
        self.script("python3", 'printf "python\\n" >> "$BRIDGE_TEST_TRACE"\nprintf "%s\\n" "$@"\n')

    def script(self, name, body):
        target = self.bin / name
        target.write_text("#!/bin/sh\n" + body)
        target.chmod(0o700)

    def native(self, target="rust", target_exit=0, snapshot_exit=0):
        self.script("omavless", f'''
printf 'native:%s:%s\\n' "$1" "$2" >> "$BRIDGE_TEST_TRACE"
if [ "$1" = plugin ] && [ "$2" = target ]; then
  printf '%s\\n' '{target}'
  exit {target_exit}
fi
if [ "$1" = plugin ] && [ "$2" = snapshot ]; then
  printf '%s\\n' '{{"synthetic":true}}'
  exit {snapshot_exit}
fi
exit 99
''')

    def run_launcher(self, *args):
        return subprocess.run(
            ["/bin/sh", str(LAUNCHER), *args], env=self.env,
            capture_output=True, text=True, timeout=5, check=False,
        )

    def test_native_only_tree_never_launches_python_when_owner_becomes_legacy_or_disappears(self):
        launcher = self.base / "backend.sh"
        launcher.write_text(LAUNCHER.read_text())
        for available in (True, False):
            if available:
                self.native(target="legacy")
            else:
                (self.bin / "omavless").unlink()
            for command in ("status", "connect", "cleanup-runtime"):
                self.trace.unlink(missing_ok=True)
                result = subprocess.run(["/bin/sh", str(launcher), command], env=self.env,
                                        capture_output=True, text=True, timeout=5)
                self.assertEqual(result.returncode, 71)
                self.assertEqual(result.stdout, "")
                self.assertNotIn("python", self.calls())

    def test_legacy_backend_symlink_and_directory_are_not_executed(self):
        launcher = self.base / "backend.sh"
        launcher.write_text(LAUNCHER.read_text())
        backend = self.base / "backend.py"
        for kind in ("symlink", "directory"):
            if kind == "symlink":
                backend.symlink_to(LAUNCHER.parent / "backend.py")
            else:
                backend.mkdir()
            result = subprocess.run(["/bin/sh", str(launcher), "status"], env=self.env,
                                    capture_output=True, text=True, timeout=5)
            self.assertEqual(result.returncode, 71)
            self.assertNotIn("python", self.calls())
            if kind == "symlink":
                backend.unlink()
            else:
                backend.rmdir()

    def calls(self):
        return self.trace.read_text().splitlines() if self.trace.exists() else []

    def action_native(self, code=0):
        self.script("omavless", f'''
if [ "$1" = plugin ] && [ "$2" = target ]; then
  printf 'native:plugin:target\\n' >> "$BRIDGE_TEST_TRACE"
  printf 'rust\\n'
  exit 0
fi
for argument in "$@"; do printf 'arg:%s\\n' "$argument" >> "$BRIDGE_TEST_TRACE"; done
printf '%s\\n' '{{"synthetic_action":true}}'
printf '%s\\n' 'Synthetic fixed error' >&2
exit {code}
''')

    def test_native_observation_uses_only_fixed_read_and_refuses_extra_arguments(self):
        self.action_native()
        result = self.run_launcher("native-observation")
        self.assertEqual(result.returncode, 0)
        self.assertEqual(self.calls(), ["native:plugin:target", "arg:runtime", "arg:observation"])
        self.trace.unlink()
        result = self.run_launcher("native-observation", "private-token")
        self.assertEqual(result.returncode, 71)
        self.assertEqual(self.calls(), ["native:plugin:target"])
        self.assertEqual(result.stdout, "")
        self.assertNotIn("private-token", result.stderr)

    def test_startup_cleanup_selects_native_without_qml_owner_discovery(self):
        self.action_native()
        for command in ["cleanup-runtime", "cleanup-qr"]:
            with self.subTest(command=command):
                result = self.run_launcher(command)
                self.assertEqual(result.returncode, 0)
                self.assertEqual(self.calls(), ["native:plugin:target", "arg:desktop", "arg:cleanup"])
                self.trace.unlink()
        service = (LAUNCHER.parent / "plugin/Service.qml").read_text()
        self.assertIn('Component.onCompleted: Quickshell.execDetached(["bash", backendPath, "cleanup-runtime"])', service)

    def test_native_removal_is_fixed_no_argument_and_no_legacy_fallback(self):
        self.action_native(code=73)
        result = self.run_launcher("watch-plugin-removal")
        self.assertEqual(result.returncode, 73)
        self.assertEqual(self.calls(), ["native:plugin:target", "arg:plugin", "arg:watch-removal"])
        self.trace.unlink()
        result = self.run_launcher("watch-plugin-removal", "private-token")
        self.assertEqual(result.returncode, 71)
        self.assertEqual(self.calls(), ["native:plugin:target"])
        self.assertNotIn("private-token", result.stderr)
        service = (LAUNCHER.parent / "plugin/Service.qml").read_text()
        self.assertIn('if [ -x /usr/bin/omavless ]; then exec /usr/bin/omavless plugin watch-removal; fi', service)

    def test_native_cleanup_refuses_paths_and_never_evaluates_input(self):
        self.action_native()
        marker = self.base / "must-not-exist"
        payload = '$(touch "' + str(marker) + '"); private-token'
        for command in ["cleanup-runtime", "cleanup-qr"]:
            result = self.run_launcher(command, payload)
            self.assertEqual(result.returncode, 71)
            self.assertEqual(self.calls(), ["native:plugin:target"])
            self.assertEqual(result.stdout, "")
            self.assertNotIn("private-token", result.stderr)
            self.assertFalse(marker.exists())
            self.trace.unlink()

    def test_native_cleanup_failure_is_preserved_without_legacy_fallback(self):
        self.action_native(code=73)
        result = self.run_launcher("cleanup-runtime")
        self.assertEqual(result.returncode, 73)
        self.assertEqual(self.calls(), ["native:plugin:target", "arg:desktop", "arg:cleanup"])
        self.trace.unlink()
        self.native(target="rust", target_exit=1)
        self.assertEqual(self.run_launcher("cleanup-runtime").returncode, 71)
        self.assertEqual(self.calls(), ["native:plugin:target"])

    def test_source_cleanup_refuses_legacy_with_or_without_native_binary(self):
        for with_binary in [False, True]:
            if with_binary:
                self.native("legacy")
            for command in ["cleanup-runtime", "cleanup-qr"]:
                result = self.run_launcher(command)
                self.assertEqual(result.returncode, 71)
                self.assertEqual(result.stdout, "")
                self.assertEqual(self.calls(), ["native:plugin:target"] if with_binary else [])
                self.trace.unlink(missing_ok=True)

    def test_native_onboarding_completion_has_fixed_fenced_arguments(self):
        self.action_native()
        result = self.run_launcher("native-onboarding-complete", "instance", "7", "operation")
        self.assertEqual(result.returncode, 0)
        self.assertEqual(self.calls(), ["native:plugin:target", "arg:plugin", "arg:onboarding-complete", "arg:instance", "arg:7", "arg:operation"])
        for args in [("native-onboarding-complete",), ("native-onboarding-complete", "instance", "7", "operation", "private-token")]:
            self.trace.unlink()
            result = self.run_launcher(*args)
            self.assertEqual(result.returncode, 71)
            self.assertEqual(self.calls(), ["native:plugin:target"])
            self.assertNotIn("private-token", result.stderr)

    def test_native_quit_has_fixed_arity_and_no_legacy_fallback(self):
        self.action_native()
        result = self.run_launcher("native-quit", "instance", "7", "operation")
        self.assertEqual(result.returncode, 0)
        self.assertEqual(self.calls(), ["native:plugin:target", "arg:plugin", "arg:quit", "arg:instance", "arg:7", "arg:operation"])
        for args in [("native-quit",), ("native-quit", "instance", "7", "operation", "private-token")]:
            self.trace.unlink()
            result = self.run_launcher(*args)
            self.assertEqual(result.returncode, 71)
            self.assertNotIn("private-token", result.stderr)
            self.assertEqual(self.calls(), ["native:plugin:target"])
        self.trace.unlink()
        self.native(target="legacy")
        self.assertEqual(self.run_launcher("native-quit", "instance", "7", "operation").returncode, 71)
        self.assertNotIn("python", self.calls())

    def test_confirmed_quit_survives_only_its_waiting_ui_wrapper_destruction(self):
        self.script("omavless", '''
if [ "$1" = plugin ] && [ "$2" = target ]; then printf 'rust\\n'; exit 0; fi
printf 'started\\n' > "$BRIDGE_TEST_TRACE"
/usr/bin/sleep 0.3
printf 'finished\\n' >> "$BRIDGE_TEST_TRACE"
''')
        worker = subprocess.Popen(["/bin/sh", str(LAUNCHER), "native-quit", "instance", "0", "operation"],
                                  env=self.env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            deadline = time.monotonic() + 3
            while "started" not in self.calls() and time.monotonic() < deadline:
                time.sleep(0.01)
            self.assertIn("started", self.calls())
            worker.kill()  # exact synthetic wrapper, never a live plugin process
            worker.wait(timeout=3)
            while "finished" not in self.calls() and time.monotonic() < deadline:
                time.sleep(0.01)
            self.assertIn("finished", self.calls())
        finally:
            if worker.poll() is None:
                worker.kill()
                worker.wait(timeout=3)

    def test_native_diagnostics_is_fixed_read_without_extra_arguments_or_legacy_fallback(self):
        self.action_native()
        result = self.run_launcher("native-diagnostics-summary")
        self.assertEqual(result.returncode, 0)
        self.assertEqual(self.calls(), ["native:plugin:target", "arg:diagnostics", "arg:summary"])
        self.trace.unlink()
        result = self.run_launcher("native-diagnostics-summary", "private-token")
        self.assertEqual(result.returncode, 71)
        self.assertEqual(result.stdout, "")
        self.assertNotIn("private-token", result.stderr)
        self.assertEqual(self.calls(), ["native:plugin:target"])
        self.trace.unlink()
        self.native(target="legacy")
        self.assertEqual(self.run_launcher("native-diagnostics-summary").returncode, 71)
        self.assertNotIn("python", self.calls())

    def test_native_qr_fixed_read_and_renderer_no_extra_arguments(self):
        self.action_native()
        for args, expected in [
            (("native-core-readiness",), ["desktop", "core-readiness"]),
            (("native-profile-details", "synthetic-record"), ["profile", "details", "synthetic-record"]),
            (("native-traffic",), ["runtime", "traffic"]),
            (("native-routing-rules",), ["routing", "rules"]),
            (("native-support-report",), ["diagnostics", "export"]),
            (("native-clipboard-copy",), ["desktop", "clipboard-copy"]),
            (("native-routing-check",), ["routing", "check"]),
            (("native-subscription-edit-input", "synthetic-record"), ["subscription", "edit-input", "synthetic-record"]),
            (("native-profile-qr", "synthetic-record"), ["profile", "export", "synthetic-record", "qr"]),
            (("native-profile-file", "synthetic-record"), ["profile", "export", "synthetic-record", "file"]),
            (("native-export-write",), ["desktop", "export-file"]),
            (("native-pick-report-export",), ["desktop", "pick-report-export"]),
            (("native-pick-profile-export",), ["desktop", "pick-profile-export"]),
            (("native-qr-render",), ["desktop", "qr-data-uri"]),
        ]:
            result = self.run_launcher(*args)
            self.assertEqual(result.returncode, 0)
            self.assertEqual(self.calls(), ["native:plugin:target", *["arg:" + item for item in expected]])
            self.trace.unlink()
        for args in [("native-profile-details",), ("native-profile-details", "id", "private-token"), ("native-core-readiness", "private-token"), ("native-profile-qr",), ("native-profile-qr", "id", "private-token"), ("native-qr-render", "private-token"), ("native-profile-file",), ("native-profile-file", "id", "private-token"), ("native-export-write", "private-token")]:
            result = self.run_launcher(*args)
            self.assertEqual(result.returncode, 71)
            self.assertEqual(self.calls(), ["native:plugin:target"])
            self.assertNotIn("private-token", result.stderr)
            self.trace.unlink()

    def test_native_qr_private_input_is_unchanged_stdin_only(self):
        self.script("omavless", '''
if [ "$1" = plugin ] && [ "$2" = target ]; then printf 'rust\\n'; exit 0; fi
for argument in "$@"; do printf 'arg:%s\\n' "$argument" >> "$BRIDGE_TEST_TRACE"; done
exec /usr/bin/cat
''')
        synthetic = 'vless://synthetic-token;$(false)'
        result = subprocess.run(["/bin/sh", str(LAUNCHER), "native-qr-render"],
                                input=synthetic, env=self.env, capture_output=True, text=True, timeout=5)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, synthetic)
        self.assertEqual(self.calls(), ["arg:desktop", "arg:qr-data-uri"])

    def test_native_long_operation_launchers_are_fixed_and_bounded(self):
        self.action_native()
        for command, tail, expected in [
            ("native-subscription-probe", ["instance", "op", "10000000-0000-4000-8000-000000000001", "4"], ["subscription", "probe"]),
            ("native-subscription-probe-results", ["instance", "op"], ["subscription", "probe-results"]),
            ("native-subscriptions-refresh-all", ["instance", "op", "4"], ["subscription", "refresh-all"]),
            ("native-providers-refresh", ["instance", "op", "4"], ["routing", "refresh-providers"]),
            ("native-operation-get", ["instance", "op"], ["operation", "get"]),
            ("native-operation-cancel", ["instance", "op"], ["operation", "cancel"]),
        ]:
            result = self.run_launcher(command, *tail)
            self.assertEqual(result.returncode, 0)
            self.assertEqual(self.calls(), ["native:plugin:target", *["arg:" + value for value in expected + tail]])
            self.trace.unlink()
            for invalid in [tail[:-1], tail + ["private-token"]]:
                result = self.run_launcher(command, *invalid)
                self.assertEqual(result.returncode, 71)
                self.assertNotIn("private-token", result.stderr)
                self.assertEqual(self.calls(), ["native:plugin:target"])
                self.trace.unlink()

    def test_native_actions_preserve_exact_fixed_mapping_and_arguments(self):
        self.action_native()
        for action, tail in [
            ("connect", ["profile-one", "global"]),
            ("disconnect", []),
            ("mode", ["direct"]),
            ("profile-rename", []),
            ("profile-favorite", []),
            ("profile-delete", []),
            ("profile-import", []),
            ("profile-replace", []),
            ("routing-preset", []),
            ("custom-rule-add", []),
            ("custom-rule-delete", []),
        ]:
            with self.subTest(action=action):
                args = ["instance-one", "4", "operation-one", *tail]
                result = self.run_launcher("native-" + action, *args)
                self.assertEqual(result.returncode, 0)
                self.assertEqual(self.calls(), ["native:plugin:target", "arg:plugin", "arg:" + action, *["arg:" + value for value in args]])
                self.trace.unlink()

    def test_native_editor_fixed_reads_refuse_extra_arguments(self):
        self.action_native()
        for args, expected in [
            (("native-profile-edit-input", "synthetic-record"), ["profile", "edit-input", "synthetic-record"]),
            (("native-profile-editor",), ["desktop", "edit"]),
        ]:
            result = self.run_launcher(*args)
            self.assertEqual(result.returncode, 0)
            self.assertEqual(self.calls(), ["native:plugin:target", *["arg:" + value for value in expected]])
            self.trace.unlink()
        for args in [("native-profile-edit-input",), ("native-profile-edit-input", "id", "private-token"), ("native-profile-editor", "private-token"), ("native-subscription-edit-input",), ("native-subscription-edit-input", "id", "private-token")]:
            result = self.run_launcher(*args)
            self.assertEqual(result.returncode, 71)
            self.assertNotIn("private-token", result.stderr)
            self.assertEqual(self.calls(), ["native:plugin:target"])
            self.trace.unlink()

    def test_native_editor_and_replacement_keep_private_input_on_stdin(self):
        self.script("omavless", '''
if [ "$1" = plugin ] && [ "$2" = target ]; then printf 'rust\\n'; exit 0; fi
for argument in "$@"; do printf 'arg:%s\\n' "$argument" >> "$BRIDGE_TEST_TRACE"; done
exec /usr/bin/cat
''')
        for args in [("native-profile-editor",), ("native-profile-replace", "instance", "4", "operation")]:
            synthetic = 'record\nSynthetic\nvless://synthetic;$(false)'
            result = subprocess.run(["/bin/sh", str(LAUNCHER), *args], input=synthetic,
                                    env=self.env, capture_output=True, text=True, timeout=5)
            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stdout, synthetic)
            self.assertNotIn("synthetic", " ".join(self.calls()))
            self.trace.unlink()

    def test_native_action_transport_unknown_exit_and_envelope_are_preserved(self):
        self.action_native(code=73)
        result = self.run_launcher("native-connect", "instance", "4", "operation", "profile", "rule")
        self.assertEqual(result.returncode, 73)
        self.assertEqual(result.stdout, '{"synthetic_action":true}\n')
        self.assertEqual(result.stderr, "Synthetic fixed error\n")
        self.assertNotIn("python", self.calls())

    def test_native_import_readers_are_fixed_and_refuse_extra_arguments(self):
        self.action_native()
        for command, expected in [
            ("native-import-preview", ["import", "preview"]),
            ("native-import-clipboard", ["desktop", "clipboard-read"]),
            ("native-import-file", ["desktop", "pick-import"]),
            ("native-import-path", ["desktop", "file-read"]),
        ]:
            result = self.run_launcher(command)
            self.assertEqual(result.returncode, 0)
            self.assertEqual(self.calls(), ["native:plugin:target", *["arg:" + x for x in expected]])
            self.trace.unlink()
            result = self.run_launcher(command, "private-input")
            self.assertEqual(result.returncode, 71)
            self.assertEqual(self.calls(), ["native:plugin:target"])
            self.assertNotIn("private-input", result.stderr)
            self.trace.unlink()

    def test_native_action_arguments_are_never_shell_evaluated(self):
        self.action_native()
        marker = self.base / "must-not-exist"
        payload = '$(touch "' + str(marker) + '"); `id` "space quote"'
        result = self.run_launcher("native-connect", "instance", "4", "operation", payload, "rule")
        self.assertEqual(result.returncode, 0)
        self.assertIn("arg:" + payload, self.calls())
        self.assertFalse(marker.exists())
        self.assertNotIn("python", self.calls())

    def test_legacy_owner_refuses_all_native_prefixed_commands(self):
        for with_binary in [False, True]:
            if with_binary:
                self.native("legacy")
            for command in ["native-connect", "native-disconnect", "native-mode", "native-profile-rename", "native-profile-favorite", "native-profile-delete", "native-observation", "native-raw"]:
                result = self.run_launcher(command, "private-token")
                self.assertEqual(result.returncode, 71)
                self.assertEqual(result.stdout, "")
                self.assertNotIn("private-token", result.stderr)
            self.assertNotIn("python", self.calls())

    def test_profile_private_stdin_is_forwarded_without_argv_or_shell_evaluation(self):
        self.script("omavless", '''
if [ "$1" = plugin ] && [ "$2" = target ]; then printf 'rust\\n'; exit 0; fi
for argument in "$@"; do printf 'arg:%s\\n' "$argument" >> "$BRIDGE_TEST_TRACE"; done
exec /usr/bin/cat
''')
        marker = self.base / "must-not-exist"
        payload = 'profile-one\\n$(touch "' + str(marker) + '"); `id`'
        result = subprocess.run(
            ["/bin/sh", str(LAUNCHER), "native-profile-rename", "instance", "4", "operation"],
            input=payload, env=self.env, capture_output=True, text=True, timeout=5,
        )
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, payload)
        self.assertEqual(self.calls(), ["arg:plugin", "arg:profile-rename", "arg:instance", "arg:4", "arg:operation"])
        self.assertFalse(marker.exists())

    def test_source_without_native_and_without_artifacts_refuses_before_python(self):
        result = self.run_launcher("status")
        self.assertEqual(result.returncode, 71)
        self.assertEqual(self.calls(), [])
        self.assertIn("native package", result.stderr)
        self.assertIn("NATIVE_INSTALL.md", result.stderr)
        self.assertFalse((self.state / "omavless").exists())

    def test_legacy_target_rejects_arguments_without_shell_eval_or_echo(self):
        self.native("legacy")
        payload = 'space $(touch SHOULD_NOT_EXIST); `id` "quoted"'
        result = self.run_launcher("import", payload)
        self.assertEqual(result.returncode, 71)
        self.assertEqual(result.stdout, "")
        self.assertNotIn(payload, result.stderr)
        self.assertEqual(self.calls(), ["native:plugin:target"])
        self.assertFalse(Path("SHOULD_NOT_EXIST").exists())

    def test_rust_status_uses_only_fixed_native_snapshot(self):
        self.native()
        result = self.run_launcher("status")
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, '{"synthetic":true}\n')
        self.assertEqual(self.calls(), ["native:plugin:target", "native:plugin:snapshot"])

    def test_rust_unknown_commands_and_extra_status_arguments_never_fall_back(self):
        self.native()
        for args in [("connect", "private-token"), ("status", "extra"), (), ("run-core",)]:
            with self.subTest(args=args):
                result = self.run_launcher(*args)
                self.assertEqual(result.returncode, 70)
                self.assertEqual(result.stdout, "")
                self.assertNotIn("private-token", result.stderr)
        self.assertNotIn("python", self.calls())
        self.assertNotIn("native:plugin:snapshot", self.calls())

    def test_native_socket_failure_keeps_native_exit_code(self):
        self.native(snapshot_exit=1)
        result = self.run_launcher("status")
        self.assertEqual(result.returncode, 70)
        self.assertNotIn("python", self.calls())

    def test_selector_failure_or_unknown_output_never_falls_back(self):
        for target, code in [("rust", 1), ("private-token", 0), ("", 0)]:
            self.native(target, code)
            result = self.run_launcher("status")
            self.assertEqual(result.returncode, 71)
            self.assertEqual(result.stdout, "")
            self.assertNotIn("private-token", result.stderr)
        self.assertNotIn("python", self.calls())

    def test_missing_native_with_any_ownership_artifact_refuses(self):
        leaf = self.state / "omavless"
        leaf.mkdir(mode=0o700)
        for name in ["ownership.json", "frontend-bridge.target"]:
            artifact = leaf / name
            for symlink in [False, True]:
                if symlink:
                    artifact.symlink_to(leaf / "missing")
                else:
                    artifact.write_text("private-token")
                result = self.run_launcher("status")
                self.assertEqual(result.returncode, 71)
                self.assertNotIn("private-token", result.stderr)
                artifact.unlink()
        self.assertEqual(self.calls(), [])

    def test_missing_native_rejects_unsafe_or_unreadable_ancestry(self):
        leaf = self.state / "omavless"
        leaf.symlink_to(self.base / "missing")
        self.assertEqual(self.run_launcher("status").returncode, 71)
        leaf.unlink()
        self.state.chmod(0)
        try:
            if os.getuid() != 0:
                self.assertEqual(self.run_launcher("status").returncode, 71)
        finally:
            self.state.chmod(0o700)
        for path in ["relative/private-token", ""]:
            self.env["XDG_STATE_HOME"] = path
            self.assertEqual(self.run_launcher("status").returncode, 71)
        self.assertEqual(self.calls(), [])

    def test_empty_existing_state_directory_is_not_changed(self):
        leaf = self.state / "omavless"
        leaf.mkdir(mode=0o700)
        self.assertEqual(self.run_launcher("status").returncode, 71)
        self.assertEqual(list(leaf.iterdir()), [])


if __name__ == "__main__":
    unittest.main()
