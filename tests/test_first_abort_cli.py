"""Source topology guards, not installed or behavioral acceptance."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "crates/omavless-runtime/src"


class FirstAbortCli(unittest.TestCase):
    def test_explicit_cli_precedes_normal_owner_paths(self):
        text = (SRC / "main.rs").read_text()
        body = text.split("fn run()", 1)[1]
        self.assertLess(body.index("arguments_admitted"), body.index("abort_from_private_input"))
        self.assertLess(body.index("abort_from_private_input"), body.index("RuntimeServer"))
        self.assertIn("OLD restored; recovery fence remains. Normal startup is still blocked.", body)

    def test_existing_lock_and_bounded_private_input_only(self):
        text = (SRC / "restore_abort_cli.rs").read_text().split("#[cfg(test)]", 1)[0]
        for token in ("deny_unknown_fields", "Zeroizing", "MAX_INPUT + 1", "from_slice",
                      "LockExclusiveNonblock", "OFlag::O_NOFOLLOW", "open_private_directory",
                      "same_member", "same_directory", "self.refused.set(!valid)",
                      "stopped.recheck()", "Uid::effective()"):
            self.assertIn(token, text)
        for forbidden in ("O_CREAT", "create(true)", "set_permissions", "remove_file",
                          "remove_dir", "Command::", "RuntimeServer", "Debug)]\nstruct Request"):
            self.assertNotIn(forbidden, text)
        self.assertLess(text.index("StoppedRuntime::acquire(paths"), text.index("abort_first_restore_current("))

    def test_checked_constructor_preserves_fixed_host_and_per_effect_guard(self):
        text = (SRC / "restore_first_abort_owner.rs").read_text()
        checked = text.split("pub(super) fn current_checked(", 1)[1].split("\nfn run<", 1)[0]
        for token in ("admitted()", "open_existing", "RuntimePaths::current",
                      "DesiredPaths::current", "CutoverPaths::current", "NativeHostPaths::current",
                      "ObservationOnlyNativeHost::new", "run_checked"):
            self.assertIn(token, checked)
        self.assertIn("admitted()\n            && lock.authorizes", text)
        self.assertIn("first_abort_owner_external_admission_is_retained_at_every_checkpoint", text)


if __name__ == "__main__":
    unittest.main()
