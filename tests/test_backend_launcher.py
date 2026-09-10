# SPDX-License-Identifier: MIT
"""Credential-free execution tests for the real frontend launcher."""
import os
from pathlib import Path
import subprocess
import tempfile
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
            (("native-traffic",), ["runtime", "traffic"]),
            (("native-routing-rules",), ["routing", "rules"]),
            (("native-routing-check",), ["routing", "check"]),
            (("native-subscription-edit-input", "synthetic-record"), ["subscription", "edit-input", "synthetic-record"]),
            (("native-profile-qr", "synthetic-record"), ["profile", "export", "synthetic-record", "qr"]),
            (("native-qr-render",), ["desktop", "qr-data-uri"]),
        ]:
            result = self.run_launcher(*args)
            self.assertEqual(result.returncode, 0)
            self.assertEqual(self.calls(), ["native:plugin:target", *["arg:" + item for item in expected]])
            self.trace.unlink()
        for args in [("native-profile-qr",), ("native-profile-qr", "id", "private-token"), ("native-qr-render", "private-token")]:
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

    def test_marketplace_without_native_and_without_artifacts_keeps_legacy(self):
        result = self.run_launcher("status")
        self.assertEqual(result.returncode, 0)
        self.assertEqual(self.calls(), ["python"])
        self.assertFalse((self.state / "omavless").exists())

    def test_native_legacy_target_preserves_exact_arguments_without_shell_eval(self):
        self.native("legacy")
        payload = 'space $(touch SHOULD_NOT_EXIST); `id` "quoted"'
        result = self.run_launcher("import", payload)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout.splitlines()[-2:], ["import", payload])
        self.assertEqual(self.calls(), ["native:plugin:target", "python"])
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
        self.assertEqual(self.run_launcher("status").returncode, 0)
        self.assertEqual(list(leaf.iterdir()), [])


if __name__ == "__main__":
    unittest.main()
