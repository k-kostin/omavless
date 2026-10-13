# SPDX-License-Identifier: MIT
"""Finite phase-earned history controls. No installed/VM/native effects."""
import errno
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


ROOT = Path(__file__).parent
driver = load("history_driver", ROOT / "restore_host/final_history_recovery.py")
fixtures = load("history_data", ROOT / "test_restore_final_new_driver.py")
old, loss = driver.old, driver.loss


class Parked(BaseException):
    pass


def request(home=driver.HOME):
    return json.dumps({"schema": 1, "archive": str(home / ".cache" / driver.EVIDENCE / "new.ovb"),
                       "passphrase": "synthetic test passphrase"}).encode()


class HistoryDriver(unittest.TestCase):
    def test_fixed207_complete_cut_and_private_input_scope(self):
        self.assertEqual(driver.ACCOUNT, "ovrestore207")
        self.assertEqual(driver.HOME, Path("/home/ovrestore207"))
        self.assertEqual(old.ACCOUNT, "ovrestore202")
        self.assertEqual(old.FIXED_ACCOUNTS[driver.ACCOUNT], driver.HOME)
        driver.phase_command("complete", driver.COMPLETE, request())
        driver.phase_command("cut", driver.current.CUT, request())
        for phase, argv, raw in [("complete", driver.COMPLETE + ["0"], request()),
                                 ("complete", driver.COMPLETE, request(Path("/home/ovrestore206"))),
                                 ("restart-start", ["/bin/sh"], b""),
                                 ("cut", driver.current.CUT, b'{"schema":1}')]:
            with self.assertRaises(old.Refused):
                driver.phase_command(phase, argv, raw)
        for mode in ("direct", "rule"):
            args = [str(old.ELF), "plugin", "mode", "instance", "1",
                    "restore099-history-mode-" + mode, mode]
            driver.phase_command("mode-" + mode, args, b"")
            with self.assertRaises(old.Refused):
                driver.phase_command("mode-" + mode, args[:-1] + ["global"], b"")

    def test_only_preflight_positive_no_reuse_or_arbitrary_control(self):
        with mock.patch.object(driver, "preflight") as before, mock.patch.object(driver, "positive") as effect:
            driver.main(["preflight"])
            driver.main(["positive"])
            before.assert_called_once_with()
            effect.assert_called_once_with()
        for args in [[], ["positive", "205"], ["recover-retained"], ["start"], ["positive", "--home", "/root"]]:
            with self.assertRaises(old.Refused):
                driver.main(args)

    def test_history0_decoding_is_uid_generation_phase_comparison_only(self):
        closure = fixtures.committed([b"old", b"old template"], [b"new", b"new template"],
                                      b'{"generation":7}', b'{"generation":2}')
        raw = fixtures.history(closure, os.getuid())
        self.assertEqual(driver.closure_from_history(raw, os.getuid(), 2), closure)
        for value, uid, generation in [(raw[:-1], os.getuid(), 2), (raw, os.getuid() + 1, 2),
                                       (raw, os.getuid(), 3)]:
            with self.assertRaises(old.Refused):
                driver.closure_from_history(value, uid, generation)

    def test_prior_history_fd_bytes_metadata_and_xattrs_retained(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "restore-disposition.history"
            loss._private_write(path, b"synthetic bounded data")
            held = []
            try:
                pin = driver.history_pin(path, os.getuid(), held)
                driver.history_unchanged(pin)
                path.rename(Path(root) / "displaced")
                loss._private_write(path, b"synthetic bounded data")
                with self.assertRaises(old.Refused):
                    driver.history_unchanged(pin)
                self.assertIsNotNone(pin[0].fd)
            finally:
                for item in held:
                    item.close()
        with mock.patch.object(driver.os, "listxattr", return_value=["user.fixture"]), \
                mock.patch.object(driver.os, "getxattr", return_value=b"bounded"):
            self.assertEqual(driver.attributes(7), {"user.fixture": b"bounded"})
        with mock.patch.object(driver.os, "listxattr", return_value=["user.fixture"]), \
                mock.patch.object(driver.os, "getxattr", return_value=bytes(4097)):
            with self.assertRaises(old.Refused):
                driver.attributes(7)

    def test_unknown_sparse_full_and_extra_prefixes_refuse_without_eviction(self):
        with tempfile.TemporaryDirectory() as root:
            config, state = Path(root) / "config", Path(root) / "state"
            config.mkdir()
            state.mkdir()
            for names in [(driver.previous.HISTORY[1],), driver.previous.HISTORY,
                          (driver.previous.HISTORY[0], "restore-disposition.history.8"),
                          (driver.previous.HISTORY[0], ".restore-unknown")]:
                for name in names:
                    loss._private_write(state / name, b"private DATA")
                with self.assertRaises(old.Refused):
                    driver.inventory(config, state, (driver.previous.HISTORY[0],))
                self.assertEqual({p.name for p in state.iterdir()}, set(names))
                for name in names:
                    (state / name).unlink()

    def test_history_xattr_value_drift_refuses_even_with_same_file_pin(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "history"
            loss._private_write(path, b"bounded synthetic DATA")
            held = []
            try:
                with mock.patch.object(driver, "attributes", return_value={"user.fixture": b"old"}):
                    pin = driver.history_pin(path, os.getuid(), held)
                with mock.patch.object(driver, "attributes", return_value={"user.fixture": b"new"}):
                    with self.assertRaises(old.Refused):
                        driver.history_unchanged(pin)
                self.assertIsNotNone(pin[0].fd)
            finally:
                for item in held:
                    item.close()

    def test_unsafe_parent_prior_scope_before_runtime_read(self):
        with tempfile.TemporaryDirectory() as root:
            home = Path(root)
            parent = home / ".cache"
            parent.mkdir(mode=0o755)
            parent.chmod(0o755)
            with mock.patch.object(driver, "HOME", home):
                with self.assertRaises(old.Refused):
                    driver.evidence_path(os.getuid())
                parent.chmod(0o700)
                (parent / driver.EVIDENCE).mkdir(mode=0o700)
                with mock.patch.object(old, "account_scope", return_value=(os.getuid(), home)), \
                        mock.patch.object(old, "ordinary") as read:
                    with self.assertRaises(old.Refused):
                        driver.preflight()
                    read.assert_not_called()

    def test_readiness_only_exact_transport_not_ready_can_poll(self):
        run = Path("/run/user/1")
        errors = [FileNotFoundError(errno.ENOENT, "not ready", str(run / "omavless/control.sock")),
                  ConnectionRefusedError(errno.ECONNREFUSED, "not ready"), TimeoutError("deadline")]
        for error in errors:
            with mock.patch.object(old, "ordinary", side_effect=[error, "ready"]) as read, \
                    mock.patch.object(old.time, "monotonic", side_effect=[0, 0.1, 0.2, 0.3, 0.4]), \
                    mock.patch.object(old.time, "sleep"):
                self.assertEqual(driver.ordinary_started(1, run, lambda: None), "ready")
                self.assertEqual(read.call_count, 2)

    def test_readiness_semantic_refusal_or_wrong_path_never_retried(self):
        for error in [old.Refused(), FileNotFoundError(errno.ENOENT, "wrong", "/proc/1/exe"),
                      OSError(errno.EACCES, "wrong")]:
            with mock.patch.object(old, "ordinary", side_effect=error) as read, \
                    mock.patch.object(old.time, "monotonic", side_effect=[0, 0.1]):
                with self.assertRaises(type(error)):
                    driver.ordinary_started(1, Path("/run/user/1"), lambda: None)
                self.assertEqual(read.call_count, 1)

    def test_readiness_expiration_cannot_start_or_repeat_effect(self):
        error = FileNotFoundError(errno.ENOENT, "not ready", "/run/user/1/omavless/control.sock")
        with mock.patch.object(old, "ordinary", side_effect=error) as read, \
                mock.patch.object(old.time, "monotonic", side_effect=[0, 0.1, 10.1]), \
                mock.patch.object(driver.Effects, "once") as effect:
            with self.assertRaises(old.Refused):
                driver.ordinary_started(1, Path("/run/user/1"), lambda: None)
            self.assertEqual(read.call_count, 1)
            effect.assert_not_called()

    def test_actual_capture_known_nonzero_or_diagnostics_do_not_authorize_restart(self):
        for code in (0, 2):
            child = mock.Mock(returncode=code)
            child.poll.return_value = code
            with tempfile.TemporaryDirectory() as root:
                effect = driver.Effects(Path(root), Path("/run/user/1"), driver.HOME)
                def spawn(*_, **kwargs):
                    os.write(kwargs["stderr"], b"RESTORE_RELEASE_DISPOSITION_VERIFIED\n")
                    os.write(kwargs["stdout"], driver.COMPLETE_MARKER)
                    return child
                try:
                    with mock.patch.object(old.subprocess, "Popen", side_effect=spawn) as started:
                        if code == 0:
                            self.assertEqual(effect.once("complete", driver.COMPLETE, request()), driver.COMPLETE_MARKER)
                        else:
                            with self.assertRaises(old.Refused):
                                effect.once("complete", driver.COMPLETE, request())
                        self.assertEqual(effect.codes["complete"], code)
                        self.assertIs(effect.children[0], child)
                        with self.assertRaises(old.Refused):
                            effect.once("complete", driver.COMPLETE, request())
                        self.assertEqual(started.call_count, 1)
                        child.kill.assert_not_called()
                        child.terminate.assert_not_called()
                finally:
                    effect.close_completed()

    def sequence(self, fault=None):
        with tempfile.TemporaryDirectory() as root:
            home, run = Path(root) / "home", Path(root) / "run"
            config, state = home / ".config/omavless", home / ".local/state/omavless"
            for path in (config, state, home / ".cache", run):
                path.mkdir(parents=True, mode=0o700)
            initial = {"version": 3, "profiles": [], "subscriptions": [], "activeId": "", "lastId": "",
                       "routingPreset": "china-cn-direct", "startup": {"enabled": False, "target": "last"}}
            owner = b'{"generation":2,"phase":"rust"}'
            desired = b'{"generation":7,"connected":false,"profileId":"","mode":"rule"}'
            for path, raw in [(config / "profiles.json", json.dumps(initial).encode()),
                              (config / "route-template.yaml", b"China template"),
                              (state / "desired.json", desired), (state / "ownership.json", owner),
                              (run / "omavless-login.receipt", b"required Login")]:
                loss._private_write(path, raw)
            phases, pins, earned = [], [], {}
            def replace(path, raw):
                path.unlink()
                loss._private_write(path, raw)
            class MockEffects:
                def __init__(self, *_):
                    self.used = set()
                def once(self, phase, argv, data=b""):
                    driver.phase_command(phase, argv, data)
                    phases.append(phase)
                    self.used.add(phase)
                    if phase == "backup":
                        earned["new"] = (config / "profiles.json").read_bytes()
                        loss._private_write(home / ".cache" / driver.EVIDENCE / "new.ovb", b"synthetic archive DATA")
                        return b"t4_current_backup_created\n"
                    if phase.startswith("preset-"):
                        replace(config / "route-template.yaml", b"Roscom template")
                    elif phase.startswith("import-"):
                        ident = "first-id" if phase.endswith("first") or fault == "same-id" else "second-id"
                        value = dict(initial, routingPreset="roscomvpn-default", profiles=[{
                            "id": ident, "name": "Loss-cut fixture", "uri": loss.SYNTHETIC_URI}])
                        replace(config / "profiles.json", json.dumps(value).encode())
                    elif phase in ("complete", "cut"):
                        if phase == "complete" and fault in ("first-broken", "first-nonzero"):
                            if fault == "first-nonzero":
                                raise old.Refused()
                            return b""
                        if phase == "cut" and fault == "cut-broken":
                            return b""
                        before = [(config / "profiles.json").read_bytes(), b"Roscom template"]
                        after = [earned["new"], b"China template"]
                        generation = (state / "desired.json").read_bytes()
                        nonce = bytes([9 if phase == "complete" or fault == "same-nonce" else 8]) * 16
                        closure = fixtures.committed(before, after, generation, owner, nonce=nonce)
                        replace(config / "profiles.json", after[0])
                        replace(config / "route-template.yaml", after[1])
                        if phase == "complete":
                            # First genuine modeled completion EARNS history0.
                            earned["prior"] = fixtures.history(closure, os.getuid())
                            loss._private_write(state / driver.previous.HISTORY[0], earned["prior"])
                            return driver.COMPLETE_MARKER
                        earned["closure"] = closure
                        loss._private_write(state / "restore-closure.complete", closure)
                        if fault == "prior-replaced":
                            replace(state / driver.previous.HISTORY[0], earned["prior"])
                        return driver.current.CUT_MARKER
                    elif phase.startswith("mode-"):
                        value = json.loads((state / "desired.json").read_bytes())
                        value["generation"] += 1
                        value["mode"] = phase.removeprefix("mode-")
                        replace(state / "desired.json", json.dumps(value).encode())
                    elif phase == "recover":
                        old.require("stop" in phases and "closure" in earned)
                        (state / "restore-closure.complete").unlink()
                        loss._private_write(state / driver.previous.HISTORY[1],
                                            fixtures.history(earned["closure"], os.getuid()))
                    return b'{"ok":true}\n'
                def close_completed(self):
                    pass
            actual_held = old.Held
            def held(*args):
                item = actual_held(*args)
                pins.append(item)
                name = Path(args[0]).name
                self.assertNotIn(name, {leaf for leaf, _ in loss.STAGED} | {
                    "restore-decision.intent", "restore-decision.terminal"})
                if name == driver.previous.HISTORY[0]:
                    self.assertIn("complete", phases)
                if name == driver.previous.HISTORY[1]:
                    self.assertIn("recover", phases)
                if name == "restore-closure.complete":
                    self.assertIn("cut", phases)
                return item
            def ordinary(*_):
                if "recover" in phases:
                    instance, revision = "final", 0
                elif "restart-start" in phases:
                    instance, revision = "restarted", 0
                else:
                    instance, revision = "original", 1 if "complete" in phases else 0
                return 2147483000, {"revision": revision, "result": {"instanceId": instance}}
            def reply(_run, _uid, method, _request):
                self.assertIn("cut", phases)
                self.assertNotIn("stop", phases)
                result = {"runtimeOwnership": False, "desired": "disconnected",
                          "actual": "disconnected", "transition": None}
                if method == "system.hello":
                    result["instanceId"] = "restarted"
                return 2147483000, {"revision": 0, "result": result}
            def manager(*_):
                stopped = phases[-1] in ("restart-stop", "stop")
                return {"MainPID": "0" if stopped else "2147483000",
                        "ActiveState": "inactive" if stopped else "active", "SubState": "dead"}
            try:
                with mock.patch.object(driver, "HOME", home), \
                        mock.patch.object(old, "account_scope", return_value=(os.getuid(), run)), \
                        mock.patch.object(driver, "Effects", MockEffects), \
                        mock.patch.object(old, "Held", side_effect=held), \
                        mock.patch.object(old, "ordinary", side_effect=ordinary), \
                        mock.patch.object(old, "reply", side_effect=reply), \
                        mock.patch.object(old, "process_identity", return_value="original-process"), \
                        mock.patch.object(old, "manager", side_effect=manager), \
                        mock.patch.object(old, "no_network"), \
                        mock.patch.object(old, "park", side_effect=Parked):
                    if fault:
                        with self.assertRaises(Parked):
                            driver.positive()
                        if fault.startswith("first-"):
                            self.assertNotIn("restart-stop", phases)
                            self.assertNotIn("restart-start", phases)
                        self.assertNotIn("stop", phases)
                        self.assertNotIn("recover", phases)
                        self.assertTrue(all(item.fd is not None for item in pins))
                    else:
                        driver.positive()
                        self.assertEqual(phases, ["backup", "preset-first", "import-first", "complete",
                            "restart-stop", "restart-start", "mode-direct", "mode-rule",
                            "preset-second", "import-second", "cut", "stop", "recover"])
                        self.assertTrue(all(item.fd is None for item in pins))
                        self.assertEqual((state / driver.previous.HISTORY[0]).read_bytes(), earned["prior"])
                    self.assertNotIn("pause", phases)
            finally:
                for item in pins:
                    item.close()

    def test_genuine_phase_earned_history0_then_current_closure_history1(self):
        self.sequence()

    def test_first_completed_missing_reply_cannot_restart_or_advance(self):
        self.sequence("first-broken")

    def test_first_completed_nonzero_remains_nonzero_no_restart(self):
        self.sequence("first-nonzero")

    def test_different_real_profile_ids_required_before_second_cut(self):
        self.sequence("same-id")

    def test_second_cut_broken_reply_never_stops_or_recovers(self):
        self.sequence("cut-broken")

    def test_prior_history_identical_byte_inode_replacement_parks(self):
        self.sequence("prior-replaced")

    def test_second_observed_transaction_must_differ_from_prior_data(self):
        self.sequence("same-nonce")


if __name__ == "__main__":
    unittest.main()
