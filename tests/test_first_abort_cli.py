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

    def test_stopped_owner_is_independent_of_initial_lock_inode_and_netns(self):
        cli = (SRC / "restore_abort_cli.rs").read_text()
        body = cli.split("pub fn abort_from_private_input", 1)[1].split("#[cfg(test)]", 1)[0]
        self.assertLess(body.index("StoppedOwner::capture"), body.index("abort_first_restore_current"))
        self.assertIn("stopped.recheck() && owner.recheck() && stopped.recheck()", body)
        self.assertIn("live_runtime_with_pre_acquire_replaced_lock_is_not_stopped", cli)
        text = (SRC / "restore_abort_stopped_owner.rs").read_text().split("#[cfg(test)]\nmod tests {", 1)[0]
        for token in ("initial.uids.contains(&uid)", "self.manager.recheck", "recovery_self(&myself.command)",
                      "MAX_PIDS: usize = 4096", "16 * 1024 * 1024", "Duration::from_secs(2)",
                      "WaitPidFlag::WNOWAIT", "known_reap", "checked_once", ".env_clear()",
                      "proc_visibility", "visible_proc_mount", "hidepid=0", "listener_paths(uid, socket)",
                      "InventoryError::KnownOwner(*pid)", "inspect_inventory",
                      'TrustedExecutable::capture("/usr/bin/systemctl")', "Command::new(tool.exec_path())",
                      '"/usr/lib/systemd/systemd"', '"omavless-runtime.service"', '"omavless.service"'):
            self.assertIn(token, text)
        for forbidden in (".try_wait(", ".kill(", ".wait(", "killpg", "pre_exec", "sudo"):
            self.assertNotIn(forbidden, text)
        # The shared source now also owns the separately admitted OLD recovery
        # Stop/Start. Keep the historical Abort observer's no-effect contract,
        # rather than incorrectly forbidding those fixed commands everywhere.
        query = text.split("fn query(", 1)[1].split("enum FixedRecoveryCommand", 1)[0]
        for forbidden in ('"start"', '"stop"'):
            self.assertNotIn(forbidden, query)
        capture = text.split("pub(super) fn capture(uid:", 1)[1].split("fn capture_inner(", 1)[0]
        self.assertIn("SelfInvocation::Recovery,\n            None,", capture)
        observe = text.split("fn observe(&self)", 1)[1].split("fn observe_inner(", 1)[0]
        self.assertIn("self.observe_inner(None, true)", observe)

    def test_cached_owner_counterexample_is_actual_frozen_child_not_service_override(self):
        text = (SRC / "restore_abort_cached_owner_tests.rs").read_text()
        for token in ("RuntimeServer::bind", "cached.nlink()", "fs::remove_file(&paths.socket)",
                      'File::open("/proc/self/exe")', "source_hash", "0o500", "tempdir_in",
                      "inspect_inventory", "InventoryError::KnownOwner(child.id())",
                      "WaitPidFlag::WNOWAIT", "known_reap", "complete.contains(&child.id())"):
            self.assertIn(token, text)
        for forbidden in (".try_wait(", ".kill(", ".wait(", "service_record(", "query("):
            self.assertNotIn(forbidden, text)


if __name__ == "__main__":
    unittest.main()
