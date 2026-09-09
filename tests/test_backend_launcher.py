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
