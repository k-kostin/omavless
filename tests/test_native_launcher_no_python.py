# SPDX-License-Identifier: MIT
"""Synthetic native-dispatch conformance, not installed R6 acceptance.

Python runs this development test driver only. Each child receives a private
PATH containing only a harmless shell implementation of the fixed `omavless`
executable, never Python, the real daemon, systemctl, or network tools. This
proves the real launcher boundary; it does not prove native command semantics
or an installed application's complete Python-absence acceptance matrix.
"""

from dataclasses import dataclass
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import unittest


LAUNCHER = Path(__file__).resolve().parents[1] / "backend.sh"
FENCE = ("synthetic-instance", "7", "synthetic-operation")
RECORD = "synthetic-record"


@dataclass(frozen=True)
class Case:
    alias: str
    tail: tuple
    command: tuple
    fixed_arity: bool = True
    private_stdin: bool = False


def fixed(alias, command, *tail, suffix=(), private_stdin=False):
    return Case(alias, tuple(tail), (*command, *tail, *suffix), True, private_stdin)


def action(name, *tail, private_stdin=False):
    # These aliases deliberately delegate semantic arity to Rust. The shell
    # must preserve each token, not reimplement the Rust request schema.
    arguments = (*FENCE, *tail)
    return Case("native-" + name, arguments, ("plugin", name, *arguments), False, private_stdin)


# One exhaustive mapping matrix drives success, errors, arity, stdin and
# quoting coverage. Synthetic IDs are intentionally not private store records.
CASES = (
    fixed("watch-plugin-removal", ("plugin", "watch-removal")),
    fixed("native-quit", ("plugin", "quit"), *FENCE),
    fixed("cleanup-runtime", ("desktop", "cleanup")),
    fixed("cleanup-qr", ("desktop", "cleanup")),
    fixed("native-subscription-probe", ("subscription", "probe"), FENCE[0], FENCE[2], RECORD, FENCE[1]),
    fixed("native-subscription-probe-results", ("subscription", "probe-results"), FENCE[0], FENCE[2]),
    fixed("native-subscriptions-refresh-all", ("subscription", "refresh-all"), FENCE[0], FENCE[2], FENCE[1]),
    fixed("native-providers-refresh", ("routing", "refresh-providers"), FENCE[0], FENCE[2], FENCE[1]),
    fixed("native-operation-get", ("operation", "get"), FENCE[0], FENCE[2]),
    fixed("native-operation-cancel", ("operation", "cancel"), FENCE[0], FENCE[2]),
    fixed("native-desktop-capabilities", ("desktop", "capabilities")),
    fixed("native-startup-capabilities", ("capabilities",)),
    fixed("native-startup-configure", ("plugin", "startup-configure"), *FENCE, private_stdin=True),
    fixed("native-support-report", ("diagnostics", "export")),
    fixed("native-clipboard-copy", ("desktop", "clipboard-copy"), private_stdin=True),
    fixed("native-onboarding-complete", ("plugin", "onboarding-complete"), *FENCE),
    fixed("native-core-readiness", ("desktop", "core-readiness")),
    fixed("native-connection-test", ("runtime", "test")),
    fixed("native-profile-details", ("profile", "details"), RECORD),
    fixed("native-ping", ("runtime", "ping"), private_stdin=True),
    fixed("native-traffic", ("runtime", "traffic")),
    fixed("native-routing-rules", ("routing", "rules")),
    fixed("native-routing-check", ("routing", "check"), private_stdin=True),
    fixed("native-diagnostics-summary", ("diagnostics", "summary")),
    fixed("native-profile-qr", ("profile", "export"), RECORD, suffix=("qr",)),
    fixed("native-profile-file", ("profile", "export"), RECORD, suffix=("file",)),
    fixed("native-export-write", ("desktop", "export-file"), private_stdin=True),
    fixed("native-pick-report-export", ("desktop", "pick-report-export"), private_stdin=True),
    fixed("native-pick-profile-export", ("desktop", "pick-profile-export"), private_stdin=True),
    fixed("native-qr-render", ("desktop", "qr-data-uri"), private_stdin=True),
    fixed("native-profile-edit-input", ("profile", "edit-input"), RECORD),
    fixed("native-subscription-edit-input", ("subscription", "edit-input"), RECORD),
    fixed("native-profile-editor", ("desktop", "edit"), private_stdin=True),
    fixed("native-observation", ("runtime", "observation")),
    fixed("native-import-preview", ("import", "preview"), private_stdin=True),
    fixed("native-import-clipboard", ("desktop", "clipboard-read")),
    fixed("native-import-file", ("desktop", "pick-import")),
    fixed("native-import-path", ("desktop", "file-read"), private_stdin=True),
    action("connect", RECORD, "global"),
    action("disconnect"),
    action("mode", "rule"),
    action("profile-rename", RECORD, private_stdin=True),
    action("profile-favorite", RECORD, "true"),
    action("profile-delete", RECORD),
    action("profile-import", private_stdin=True),
    action("profile-bundle-import", private_stdin=True),
    action("profile-replace", RECORD, private_stdin=True),
    action("subscription-add", private_stdin=True),
    action("subscription-update", RECORD, private_stdin=True),
    action("subscription-delete", RECORD),
    action("subscription-refresh", RECORD),
    action("routing-preset", "synthetic-preset", "true"),
    action("custom-rule-add", "domain", "direct", private_stdin=True),
    action("custom-rule-delete", "synthetic-rule"),
    fixed("status", ("plugin", "snapshot")),
)


class NativeLauncherNoPythonTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="omavless-no-python-launcher-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.bin = self.root / "bin"
        self.bin.mkdir(mode=0o700)
        self.state = self.root / "state"
        self.state.mkdir(mode=0o700)
        self.calls = self.root / "calls"
        self.arguments = self.root / "arguments"
        self.input = self.root / "stdin"
        self.env = {
            "PATH": str(self.bin), "HOME": str(self.root),
            "XDG_STATE_HOME": str(self.state),
            "TEST_CALLS": str(self.calls), "TEST_ARGUMENTS": str(self.arguments),
            "TEST_INPUT": str(self.input), "TEST_TARGET": "rust",
            "TEST_TARGET_EXIT": "0", "TEST_NATIVE_EXIT": "0",
        }
        stub = self.bin / "omavless"
        stub.write_text('''#!/bin/sh
if [ "$1" = plugin ] && [ "$2" = target ]; then
  printf 'selector\\n' >> "$TEST_CALLS"
  printf '%s\\n' "$TEST_TARGET"
  exit "$TEST_TARGET_EXIT"
fi
printf 'native\\n' >> "$TEST_CALLS"
printf '%s\\0' "$@" > "$TEST_ARGUMENTS"
/bin/cat > "$TEST_INPUT"
printf '%s\\n' '{"synthetic":true}'
printf '%s\\n' 'Synthetic fixed native diagnostic' >&2
exit "$TEST_NATIVE_EXIT"
''')
        stub.chmod(0o700)

    def run_launcher(self, arguments, data=b""):
        for file in (self.calls, self.arguments, self.input):
            file.unlink(missing_ok=True)
        return subprocess.run(
            ["/bin/sh", str(LAUNCHER), *arguments], input=data,
            env=self.env, capture_output=True, timeout=5, check=False,
        )

    def recorded_calls(self):
        return self.calls.read_text().splitlines() if self.calls.exists() else []

    def recorded_arguments(self):
        return tuple(part.decode() for part in self.arguments.read_bytes().split(b"\0")[:-1])

    def assert_no_legacy(self, result):
        for token in (b"python", b"backend.py", b"dirname"):
            self.assertNotIn(token, result.stdout + result.stderr)

    def test_fixture_path_has_no_python_or_real_host_commands(self):
        self.assertEqual(sorted(path.name for path in self.bin.iterdir()), ["omavless"])
        for command in ("python", "python3", "systemctl", "sudo", "pkexec", "curl", "omarchy"):
            self.assertIsNone(shutil.which(command, path=self.env["PATH"]))
        result = subprocess.run(
            ["/bin/sh", "-c", "command -v python3"], env=self.env,
            capture_output=True, timeout=5, check=False,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, b"")

    def test_matrix_covers_every_native_alias_once(self):
        native = r"(?:native-[a-z-]+|watch-plugin-removal|cleanup-runtime|cleanup-qr)"
        groups = re.findall(r"(?m)^\s*(" + native + r"(?:\|" + native + r")*)\)", LAUNCHER.read_text())
        aliases = {alias for group in groups for alias in group.split("|")}
        expected = [case.alias for case in CASES]
        self.assertEqual(len(expected), len(set(expected)))
        self.assertEqual(set(expected), aliases | {"status"})

    def test_all_native_aliases_dispatch_exact_argv_without_python(self):
        for case in CASES:
            with self.subTest(alias=case.alias):
                result = self.run_launcher((case.alias, *case.tail))
                self.assertEqual(result.returncode, 0)
                self.assertEqual(self.recorded_calls(), ["selector", "native"])
                self.assertEqual(self.recorded_arguments(), case.command)
                self.assertEqual(result.stdout, b'{"synthetic":true}\n')
                self.assert_no_legacy(result)

    def test_every_native_failure_is_terminal_without_python_fallback(self):
        for case in CASES:
            for code in (1, 73):
                with self.subTest(alias=case.alias, code=code):
                    self.env["TEST_NATIVE_EXIT"] = str(code)
                    result = self.run_launcher((case.alias, *case.tail))
                    self.assertEqual(result.returncode, 70 if case.alias == "status" else code)
                    self.assertEqual(self.recorded_calls(), ["selector", "native"])
                    self.assertEqual(self.recorded_arguments(), case.command)
                    self.assert_no_legacy(result)

    def test_private_input_is_byte_identical_stdin_not_arguments_or_output(self):
        # Deliberately includes shell syntax, newlines and non-UTF8/NUL bytes.
        # The launcher is not a parser; Rust must receive the unchanged bytes.
        payload = b'synthetic-private-input\n$(false); "quoted" \\path\n\x00\xff\n'
        for case in CASES:
            if not case.private_stdin:
                continue
            with self.subTest(alias=case.alias):
                result = self.run_launcher((case.alias, *case.tail), payload)
                self.assertEqual(result.returncode, 0)
                self.assertEqual(self.input.read_bytes(), payload)
                self.assertEqual(self.recorded_arguments(), case.command)
                self.assertNotIn(b"synthetic-private-input", result.stdout + result.stderr)
                self.assertNotIn(b"synthetic-private-input", self.arguments.read_bytes())

    def test_fixed_arity_rejects_without_dispatch_or_private_echo(self):
        for case in CASES:
            if not case.fixed_arity:
                continue
            invalid = [(case.alias, *case.tail, "synthetic-private-extra")]
            if case.tail:
                invalid.append((case.alias, *case.tail[:-1]))
            for arguments in invalid:
                with self.subTest(alias=case.alias, count=len(arguments)):
                    result = self.run_launcher(arguments, b"synthetic-private-stdin")
                    self.assertEqual(result.returncode, 70 if case.alias == "status" else 71)
                    self.assertEqual(self.recorded_calls(), ["selector"])
                    self.assertFalse(self.arguments.exists())
                    self.assertFalse(self.input.exists())
                    self.assertNotIn(b"synthetic-private", result.stdout + result.stderr)
                    self.assert_no_legacy(result)

    def test_semantic_aliases_preserve_opaque_argv_and_delegate_validation(self):
        marker = self.root / "must-not-exist"
        unusual = ("", "two words", "line\nbreak", "*?[x]", "--option", f'$(/usr/bin/touch "{marker}"); literal')
        for case in CASES:
            if case.fixed_arity:
                continue
            with self.subTest(alias=case.alias):
                result = self.run_launcher((case.alias, *unusual))
                self.assertEqual(result.returncode, 0)
                self.assertEqual(self.recorded_arguments(), (*case.command[:2], *unusual))
                self.assertFalse(marker.exists())
                self.assert_no_legacy(result)

    def test_unknown_selector_or_failed_selector_never_dispatches(self):
        for target, code in (("unknown", "0"), ("", "0"), ("rust\nlegacy", "0"), ("rust", "73"), ("legacy", "73")):
            self.env["TEST_TARGET"] = target
            self.env["TEST_TARGET_EXIT"] = code
            for case in CASES:
                with self.subTest(alias=case.alias, target=target, code=code):
                    result = self.run_launcher((case.alias, *case.tail), b"synthetic-private")
                    self.assertEqual(result.returncode, 71)
                    self.assertEqual(self.recorded_calls(), ["selector"])
                    self.assertEqual(result.stdout, b"")
                    self.assertNotIn(b"synthetic-private", result.stderr)
                    self.assert_no_legacy(result)

    def test_legacy_selection_refuses_every_explicit_native_alias(self):
        self.env["TEST_TARGET"] = "legacy"
        for case in CASES:
            if not case.alias.startswith("native-"):
                continue
            with self.subTest(alias=case.alias):
                result = self.run_launcher((case.alias, *case.tail))
                self.assertEqual(result.returncode, 71)
                self.assertEqual(self.recorded_calls(), ["selector"])
                self.assert_no_legacy(result)

    def test_unknown_native_or_legacy_command_never_falls_through_native_owner(self):
        for arguments in ((), ("native-not-a-command",), ("connect",), ("edit-config",), ("arbitrary-shell", "synthetic-private")):
            result = self.run_launcher(arguments)
            self.assertEqual(result.returncode, 70)
            self.assertEqual(self.recorded_calls(), ["selector"])
            self.assertNotIn(b"synthetic-private", result.stdout + result.stderr)
            self.assert_no_legacy(result)

    def test_missing_binary_with_native_artifacts_blocks_all_aliases(self):
        (self.bin / "omavless").unlink()
        directory = self.state / "omavless"
        directory.mkdir(mode=0o700)
        for name in ("ownership.json", "frontend-bridge.target"):
            for dangling in (False, True):
                marker = directory / name
                if dangling:
                    marker.symlink_to(directory / "nonexistent")
                else:
                    marker.write_text("synthetic artifact: content must not be parsed")
                    marker.chmod(0o600)
                try:
                    for case in CASES:
                        with self.subTest(alias=case.alias, artifact=name, dangling=dangling):
                            result = self.run_launcher((case.alias, *case.tail), b"synthetic-private")
                            self.assertEqual(result.returncode, 71)
                            self.assertEqual(self.recorded_calls(), [])
                            self.assertEqual(result.stdout, b"")
                            self.assertNotIn(b"synthetic-private", result.stderr)
                            self.assert_no_legacy(result)
                finally:
                    marker.unlink()


if __name__ == "__main__":
    unittest.main()
