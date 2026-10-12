# SPDX-License-Identifier: MIT
"""Finite DATA/mock controls, never an installed runtime or VM operation."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


ROOT = Path(__file__).parent
driver = load("final_old_driver", ROOT / "restore_host/final_old_recovery.py")
fixtures = load("loss_data", ROOT / "test_restore_loss_driver.py")
old, loss = driver.old, driver.loss


def closure(intent, pair, phase=3):
    terminal = fixtures.wrap(b"OVRDEC01", bytes([phase]) + intent[9:106],
                             b"omavless-restore-decision-v1\0")
    bindings = b"".join(len(item).to_bytes(4, "big") + hashlib.sha256(item).digest()
                        for item in pair)
    receipt = fixtures.wrap(b"OVRFIN01", terminal + bindings,
                            b"omavless-restore-finalization-v1\0")
    return fixtures.wrap(b"OVRCLS01", receipt, b"omavless-restore-closure-v1\0")


class FinalOld(unittest.TestCase):
    def test_fixed_account_scope_commands_and_closed_cut_input(self):
        self.assertEqual(driver.ACCOUNT, "ovrestore205")
        self.assertEqual(driver.HOME, Path("/home/ovrestore205"))
        self.assertEqual(driver.EVIDENCE, "restore099-pc-final-old")
        self.assertEqual(old.ACCOUNT, "ovrestore202")
        self.assertEqual(old.FIXED_ACCOUNTS[driver.ACCOUNT], driver.HOME)
        driver.phase_command("cut", driver.CUT, b'{"schema":1}')
        driver.phase_command("recover", driver.RECOVER, b"archive input")
        for data in [b"{}", b'{"schema":1,"step":0}', b'{"schema":1,"archive":"secret"}']:
            with self.assertRaises(old.Refused):
                driver.phase_command("cut", driver.CUT, data)
        for phase, argv in [("cut", driver.CUT + ["0"]), ("stop", ["/bin/sh"]),
                            ("recover", [str(old.ELF), "restore", "recover-old", "--confirm-rollback"])]:
            with self.assertRaises(old.Refused):
                driver.phase_command(phase, argv, driver.CUT_INPUT)

    def test_cli_only_two_entries_and_no_retained_replay(self):
        with mock.patch.object(driver, "preflight") as before, mock.patch.object(driver, "positive") as effect:
            driver.main(["preflight"])
            driver.main(["positive"])
            before.assert_called_once_with()
            effect.assert_called_once_with()
        for argv in [[], ["recover-retained"], ["positive", "201"], ["positive", "--home", "/root"]]:
            with self.assertRaises(old.Refused):
                driver.main(argv)

    def test_positive_cut_marker_refuses_negative_unreached_and_broken_reply(self):
        driver.cut_result(driver.CUT_MARKER)
        for raw in [b"", driver.CUT_MARKER[:-1], driver.CUT_MARKER + b"extra",
                    b"t4_current_intent_aborted\n", b'{"cutReachedStillFenced":false}\n',
                    b"t4_current_pair_completed\n"]:
            with self.assertRaises(old.Refused):
                driver.cut_result(raw)

    def test_closure_exact_abort_nonce_stage_and_pair(self):
        members, _, intent, _, _, _, _ = fixtures.data()
        raw = closure(intent, members[:2])
        driver.closure_matches(raw, intent, members[:2])
        for changed, prior, pair in [(raw[:-1], intent, members[:2]),
                                    (closure(intent, members[:2], 2), intent, members[:2]),
                                    (raw, intent, members[2:]),
                                    (raw, fixtures.wrap(b"OVRDEC01", b"\x01" + intent[9:90]
                                          + bytes([8]) * 16, b"omavless-restore-decision-v1\0"), members[:2])]:
            with self.assertRaises(old.Refused):
                driver.closure_matches(changed, prior, pair)

    def test_inventory_all_eight_history_slots_and_unknowns_refuse(self):
        with tempfile.TemporaryDirectory() as root:
            config, state = Path(root) / "config", Path(root) / "state"
            config.mkdir()
            state.mkdir()
            driver.inventory(config, state, ())
            for name in driver.HISTORY + ("restore-unknown", "restore-finalization.pending",
                                           "routing-preset.pending.json"):
                path = state / name
                path.write_bytes(b"data")
                with self.assertRaises(old.Refused):
                    driver.inventory(config, state, ())
                path.unlink()
            for name in driver.SLOTS + ("restore-foreign", ".restore-foreign"):
                path = config / name
                path.symlink_to(config / "missing")
                with self.assertRaises(old.Refused):
                    driver.inventory(config, state, ())
                path.unlink()

    def test_retired_original_fd_expected_unlink_not_substitution(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "intent"
            loss._private_write(path, b"private comparison data")
            item = old.Held(path, os.getuid(), 1024)
            try:
                with self.assertRaises(old.Refused):
                    driver.retired_pin(item)
                path.unlink()
                driver.retired_pin(item)
            finally:
                item.close()

    def test_actual_cut_child_nonzero_retained_and_not_retried(self):
        child = mock.Mock(returncode=2)
        child.poll.return_value = 2
        with tempfile.TemporaryDirectory() as root:
            effect = driver.Effects(Path(root), Path("/run/user/1"), driver.HOME)
            try:
                with mock.patch.object(old.subprocess, "Popen", return_value=child) as spawn:
                    for _ in range(2):
                        with self.assertRaises(old.Refused):
                            effect.once("cut", driver.CUT, driver.CUT_INPUT)
                    self.assertEqual(spawn.call_count, 1)
                    self.assertEqual(effect.codes["cut"], 2)
                    effect.files[0].seek(0)
                    self.assertEqual(effect.files[0].read(), driver.CUT_INPUT)
                    self.assertEqual(spawn.call_args.args[0], driver.CUT)
                    self.assertIs(effect.children[0], child)
                    child.kill.assert_not_called()
                    child.terminate.assert_not_called()
            finally:
                effect.close_completed()

    def test_poll_loss_and_deadline_park_original_without_kill(self):
        for timeout in (False, True):
            child = mock.Mock()
            if timeout:
                child.poll.return_value = None
            else:
                child.poll.side_effect = RuntimeError("private detail")
            with tempfile.TemporaryDirectory() as root:
                effect = driver.Effects(Path(root), Path("/run/user/1"), driver.HOME)
                try:
                    with mock.patch.object(old.subprocess, "Popen", return_value=child), \
                            mock.patch.object(old.time, "monotonic", side_effect=[0, 150]), \
                            mock.patch.object(old, "park", side_effect=fixtures.Parked):
                        with self.assertRaises(fixtures.Parked):
                            effect.once("cut", driver.CUT, driver.CUT_INPUT)
                    self.assertIs(effect.children[0], child)
                    self.assertTrue(all(os.fstat(fd) for fd in effect.capture_fds))
                    child.kill.assert_not_called()
                    child.terminate.assert_not_called()
                finally:
                    effect.close_completed()

    def test_preflight_prior_scope_refuses_before_any_runtime_read(self):
        with tempfile.TemporaryDirectory() as root:
            home = Path(root)
            (home / ".cache" / driver.EVIDENCE).mkdir(parents=True)
            with mock.patch.object(driver, "HOME", home), \
                    mock.patch.object(old, "account_scope", return_value=(os.getuid(), home)), \
                    mock.patch.object(old, "ordinary") as read:
                with self.assertRaises(old.Refused):
                    driver.preflight()
                read.assert_not_called()

    def test_fixed_private_cache_parent_rejects_symlink_or_permissions(self):
        with tempfile.TemporaryDirectory() as root:
            home = Path(root)
            actual = home / "actual"
            actual.mkdir(mode=0o700)
            parent = home / ".cache"
            parent.symlink_to(actual)
            with mock.patch.object(driver, "HOME", home):
                with self.assertRaises(old.Refused):
                    driver.evidence_path(os.getuid())
                parent.unlink()
                parent.mkdir(mode=0o755)
                parent.chmod(0o755)
                with self.assertRaises(old.Refused):
                    driver.evidence_path(os.getuid())
                parent.chmod(0o700)
                self.assertEqual(driver.evidence_path(os.getuid()), parent / driver.EVIDENCE)

    def test_paused_owner_same_instance_revision_and_process_required(self):
        current = (44, {"revision": 2, "result": {"instanceId": "original", "runtimeOwnership": False,
                                                "desired": "disconnected", "actual": "disconnected"}})
        fields = {"MainPID": "44", "ActiveState": "active"}
        for changed in [(44, {"revision": 3, "result": {"instanceId": "original"}}),
                        (44, {"revision": 2, "result": {"instanceId": "replacement"}}),
                        (45, current[1]),
                        (44, {"revision": 2, "result": dict(current[1]["result"], runtimeOwnership=True)})]:
            with mock.patch.object(old, "reply", return_value=changed), \
                    mock.patch.object(old, "manager", return_value=fields), \
                    mock.patch.object(old, "process_identity", return_value="original"), \
                    mock.patch.object(old, "no_network"):
                with self.assertRaises(old.Refused):
                    driver.paused_owner(os.getuid(), Path("/run/user/1"), current, "original")
        with mock.patch.object(old, "reply", return_value=current), \
                mock.patch.object(old, "manager", return_value=fields), \
                mock.patch.object(old, "process_identity", return_value="original"), \
                mock.patch.object(old, "no_network"):
            driver.paused_owner(os.getuid(), Path("/run/user/1"), current, "original")

    def run_sequential(self, broken=False, fault=None):
        # Actual temporary DATA files, but every producer operation is mocked.
        # Records appear only when their preceding phase has completed; a first
        # observation cannot accidentally borrow a pre-populated final history.
        with tempfile.TemporaryDirectory() as root:
            home, run = Path(root) / "home", Path(root) / "run"
            config, state = home / ".config/omavless", home / ".local/state/omavless"
            for path in (config, state, home / ".cache", run):
                path.mkdir(parents=True, mode=0o700)
            initial = {"version": 3, "profiles": [], "subscriptions": [], "activeId": "", "lastId": "",
                       "routingPreset": "china-cn-direct", "startup": {"enabled": False, "target": "last"}}
            desired = b'{"generation":7,"connected":false,"profileId":"","mode":"rule"}'
            owner = b'{"generation":2,"phase":"rust"}'
            for path, raw in [(config / "profiles.json", json.dumps(initial).encode()),
                              (config / "route-template.yaml", b"China template"),
                              (state / "desired.json", desired), (state / "ownership.json", owner),
                              (run / "omavless-login.receipt", b"required login")]:
                loss._private_write(path, raw)
            phases, retained, earned = [], [], {}

            def replace(path, raw):
                path.unlink()
                loss._private_write(path, raw)

            class MockEffects:
                def __init__(self, *_):
                    self.used = set()
                def once(self, phase, argv, data=b""):
                    driver.phase_command(phase, argv, data)
                    self.used.add(phase)
                    phases.append(phase)
                    if phase == "backup":
                        loss._private_write(home / ".cache" / driver.EVIDENCE / "new.ovb", b"encrypted archive")
                    elif phase == "preset":
                        replace(config / "route-template.yaml", b"Roscom template")
                    elif phase == "import":
                        value = dict(initial, routingPreset="roscomvpn-default", profiles=[
                            {"name": "Loss-cut fixture", "uri": loss.SYNTHETIC_URI}])
                        replace(config / "profiles.json", json.dumps(value).encode())
                    elif phase == "pause":
                        stage = state / "restore-pair.pending"
                        stage.mkdir(mode=0o700)
                        members = [(config / "profiles.json").read_bytes(), b"Roscom template",
                                   json.dumps(initial).encode(), b"China template"]
                        ready = b"OVRPAIR1" + b"".join(len(item).to_bytes(4, "big") for item in members)
                        ready += b"".join(hashlib.sha256(item).digest() for item in members)
                        for (name, _), raw in zip(loss.STAGED, members + [ready]):
                            loss._private_write(stage / name, raw)
                        fields = ((2).to_bytes(8, "big") + (7).to_bytes(8, "big") + b"\x01"
                                  + hashlib.sha256(desired).digest() + hashlib.sha256(ready).digest() + bytes([9]) * 16)
                        earned["intent"] = fixtures.wrap(b"OVRDEC01", b"\x01" + fields,
                                                         b"omavless-restore-decision-v1\0")
                        earned["pair"] = members[:2]
                        loss._private_write(state / "restore-decision.intent", earned["intent"])
                        return b"t4_current_intent_paused\n"
                    elif phase == "cut":
                        if broken:
                            return b""
                        old.require("intent" in earned and "recover" not in phases)
                        for item in (state / "restore-pair.pending").iterdir():
                            item.unlink()
                        (state / "restore-pair.pending").rmdir()
                        (state / "restore-decision.intent").unlink()
                        loss._private_write(state / "restore-closure.complete",
                                            closure(earned["intent"], earned["pair"], 2 if fault == "committed" else 3))
                        if fault == "replacement":
                            replace(config / "profiles.json", earned["pair"][0])
                        if fault == "receipt":
                            loss._private_write(state / "restore-finalization.pending", b"leftover")
                        return driver.CUT_MARKER
                    elif phase == "recover":
                        old.require("stop" in phases and loss.absent(state / driver.HISTORY[0]))
                        (state / "restore-closure.complete").unlink()
                        loss._private_write(state / driver.HISTORY[0],
                                            fixtures.history(earned["intent"], earned["pair"], 3, os.getuid()))
                    return b'{"ok":true}\n'
                def close_completed(self):
                    pass

            original_held = old.Held
            def held(*args):
                item = original_held(*args)
                retained.append(item)
                # This assertion is the first-observation dependency control.
                name = Path(args[0]).name
                if name.startswith("restore-disposition.history"):
                    self.assertIn("recover", phases)
                if name == "restore-closure.complete":
                    self.assertIn("cut", phases)
                if name == "restore-decision.intent":
                    self.assertIn("pause", phases)
                return item

            def ordinary(*_):
                self.assertNotIn("cut", phases) if "recover" not in phases else None
                fresh = "recover" in phases
                return 2147483000, {"revision": 2, "result": {"instanceId": "fresh" if fresh else "original"}}

            def manager(*_):
                stopped = "stop" in phases
                return {"MainPID": "0" if stopped else "2147483000",
                        "ActiveState": "inactive" if stopped else "active", "SubState": "dead"}

            def paused_reply(*_):
                self.assertIn("pause", phases)
                self.assertNotIn("stop", phases)
                return 2147483000, {"revision": 2, "result": {
                    "instanceId": "original", "runtimeOwnership": False,
                    "desired": "disconnected", "actual": "disconnected"}}

            try:
                with mock.patch.object(driver, "HOME", home), \
                        mock.patch.object(old, "account_scope", return_value=(os.getuid(), run)), \
                        mock.patch.object(old, "Held", side_effect=held), \
                        mock.patch.object(driver, "Effects", MockEffects), \
                        mock.patch.object(old, "ordinary", side_effect=ordinary), \
                        mock.patch.object(old, "reply", side_effect=paused_reply), \
                        mock.patch.object(old, "no_network"), \
                        mock.patch.object(old, "process_identity", return_value="original"), \
                        mock.patch.object(old, "manager", side_effect=manager), \
                        mock.patch.object(old, "park", side_effect=fixtures.Parked):
                    if broken or fault:
                        with self.assertRaises(fixtures.Parked):
                            driver.positive()
                        self.assertNotIn("stop", phases)
                        self.assertNotIn("recover", phases)
                        self.assertTrue(all(item.fd is not None for item in retained))
                    else:
                        driver.positive()
                        self.assertEqual(phases, ["backup", "preset", "import", "pause", "cut", "stop", "recover"])
                        self.assertTrue(all(item.fd is None for item in retained))
            finally:
                for item in retained:
                    item.close()

    def test_sequential_recipe_only_reads_phase_earned_records(self):
        self.run_sequential()

    def test_broken_cut_reply_parks_before_stop_recovery_and_retains_pins(self):
        self.run_sequential(broken=True)

    def test_unexpected_committed_closure_parks_before_stop(self):
        self.run_sequential(fault="committed")

    def test_identical_live_inode_replacement_parks_before_stop(self):
        self.run_sequential(fault="replacement")

    def test_unretired_receipt_parks_before_stop(self):
        self.run_sequential(fault="receipt")


if __name__ == "__main__":
    unittest.main()
