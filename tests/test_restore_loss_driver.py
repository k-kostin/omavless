# SPDX-License-Identifier: MIT
"""Pure fixed-driver/data controls; never invokes a VM or installed app."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location(
    "loss_driver", Path(__file__).parent / "restore_host/loss_recovery.py")
driver = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(driver)


def wrap(magic, body, domain):
    raw = magic + body
    return raw + hashlib.sha256(domain + raw).digest()


def data(mode="mixed"):
    members = [b"old store", b"old template", b"new store", b"new template"]
    desired = b'{"generation":7,"connected":false,"profileId":"","mode":"rule"}'
    owner = b'{"generation":2,"phase":"rust"}'
    ready = b"OVRPAIR1" + b"".join(len(member).to_bytes(4, "big") for member in members)
    ready += b"".join(hashlib.sha256(member).digest() for member in members)
    fields = ((2).to_bytes(8, "big") + (7).to_bytes(8, "big") + b"\x01"
              + hashlib.sha256(desired).digest() + hashlib.sha256(ready).digest() + bytes([9]) * 16)
    intent = wrap(b"OVRDEC01", b"\x01" + fields, b"omavless-restore-decision-v1\0")
    terminal = (wrap(b"OVRDEC01", b"\x02" + fields, b"omavless-restore-decision-v1\0")
                if mode == "committed" else None)
    live = [members[2], members[1 if mode == "mixed" else 3]]
    return members, live, intent, terminal, ready, desired, owner


def history(intent, members, phase, uid):
    terminal = wrap(b"OVRDEC01", bytes([phase]) + intent[9:106],
                    b"omavless-restore-decision-v1\0")
    bindings = b"".join(len(item).to_bytes(4, "big") + hashlib.sha256(item).digest()
                        for item in members)
    receipt = wrap(b"OVRFIN01", terminal + bindings, b"omavless-restore-finalization-v1\0")
    closure = wrap(b"OVRCLS01", receipt, b"omavless-restore-closure-v1\0")
    ticket = wrap(b"OVRDSP01", uid.to_bytes(4, "little") + bytes(4)
                  + int.from_bytes(intent[9:17], "big").to_bytes(8, "little") + closure,
                  b"omavless-restore-disposition-candidate-v1\0")
    return wrap(b"OVRDON01", ticket, b"omavless-restore-disposition-complete-v1\0")


class Parked(BaseException):
    pass


class LossDriver(unittest.TestCase):
    def test_fixed_modes_accounts_commands_and_old_defaults(self):
        self.assertEqual(driver.old.ACCOUNT, "ovrestore202")
        self.assertEqual(driver.old.HOME, Path("/home/ovrestore202"))
        for mode, account, cut, recover, phase in [
            ("mixed", "ovrestore203", "cut-current-first-rename", "recover-mixed", 3),
            ("committed", "ovrestore204", "cut-current-terminal", "recover-committed", 2),
        ]:
            selected = driver.scenario(mode)
            self.assertEqual(selected[0], account)
            self.assertEqual(selected[1], Path("/home") / account)
            self.assertEqual(selected[8], phase)
            producer, consumer = driver.commands(mode)
            self.assertEqual(producer[2], cut)
            self.assertEqual(consumer[2], recover)
            self.assertEqual(len(producer), 4)
            self.assertEqual(len(consumer), 4)
            driver.phase_command(selected[1], "cut", producer)
            driver.phase_command(selected[1], "recover", consumer)
        with self.assertRaises(driver.old.Refused):
            driver.scenario("caller step")

    def test_account_pair_refuses_before_lookup_and_no_uid_override(self):
        with mock.patch.object(driver.old.pwd, "getpwnam") as lookup:
            for account, home in [("root", Path("/root")), ("ovrestore203", Path("/home/ovrestore204")),
                                  ("ovrestore202", Path("/arbitrary"))]:
                with self.assertRaises(driver.old.Refused):
                    driver.old.account_scope(account, home)
            lookup.assert_not_called()

    def test_cli_has_only_four_fixed_entries(self):
        with mock.patch.object(driver, "preflight") as before, mock.patch.object(driver, "positive") as effect:
            driver.main(["preflight-mixed"])
            before.assert_called_once_with("mixed")
            driver.main(["positive-committed"])
            effect.assert_called_once_with("committed")
        for argv in [[], ["positive"], ["positive-mixed", "0"], ["stop"], ["recover-retained"],
                     ["positive-committed", "--home", "/root"]]:
            with self.assertRaises(driver.old.Refused):
                driver.main(argv)

    def test_fixed_phase_commands_reject_caller_selectors(self):
        home = driver.scenario("mixed")[1]
        valid = [str(driver.old.ELF), "plugin", "routing-preset", "instance", "1",
                 "restore099-mixed-preset"]
        driver.phase_command(home, "preset", valid)
        for phase, argv in [("stop", ["/bin/sh", "-c", "anything"]),
                            ("cut", driver.commands("mixed")[0] + ["1"]),
                            ("preset", valid[:-1] + ["another"]),
                            ("preset", valid[:4] + ["-1", valid[5]])]:
            with self.assertRaises(driver.old.Refused):
                driver.phase_command(home, phase, argv)

    def test_cut_marker_is_positive_only_not_completed_restore(self):
        for mode in ("mixed", "committed"):
            expected = driver.scenario(mode)[5]
            driver.cut_result(mode, expected)
            for raw in [b"", b"t4_current_pair_completed\n", b'{"completed":true}\n',
                        b'{"cutReachedStillFenced":false}\n', expected + b"private-data", expected[:-1]]:
                with self.assertRaises(driver.old.Refused):
                    driver.cut_result(mode, raw)

    def test_actual_pair_stage_intent_and_terminal_are_all_required(self):
        for mode in ("mixed", "committed"):
            values = data(mode)
            driver.cut_state(mode, *values)
            members, live, intent, terminal, ready, desired, owner = values
            for index, changed in [(1, [members[0], members[1]]), (2, intent[:-1]),
                                   (4, ready[:-1]), (5, b"{}"), (6, b"{}")]:
                bad = list(values)
                bad[index] = changed
                with self.assertRaises(driver.old.Refused):
                    driver.cut_state(mode, *bad)
            identical = list(members)
            identical[3] = identical[1]
            with self.assertRaises(driver.old.Refused):
                driver.cut_state(mode, identical, live, intent, terminal, ready, desired, owner)
        with self.assertRaises(driver.old.Refused):
            driver.cut_state("mixed", *data("committed"))
        with self.assertRaises(driver.old.Refused):
            driver.cut_state("committed", *data("mixed"))

    def test_new_semantics_allow_reserialization_but_not_changed_payload(self):
        value = {"version": 3, "profiles": [], "subscriptions": [], "activeId": "", "lastId": "",
                 "routingPreset": "china-cn-direct", "startup": {"enabled": False, "target": "last"}}
        driver.portable_new(json.dumps(value, indent=2).encode(), value)
        for key, changed in [("routingPreset", "roscomvpn-default"), ("profiles", [{}]),
                             ("lastId", "invented")]:
            bad = dict(value, **{key: changed})
            with self.assertRaises(driver.old.Refused):
                driver.portable_new(json.dumps(bad).encode(), value)

    def test_history_outcome_is_three_for_old_two_for_committed(self):
        members, _, intent, _, _, _, _ = data()
        uid = os.getuid()
        for phase, pair in [(3, members[:2]), (2, members[2:])]:
            raw = history(intent, pair, phase, uid)
            driver.old.history_matches(raw, intent, uid, pair, phase)
            with self.assertRaises(driver.old.Refused):
                driver.old.history_matches(raw, intent, uid, pair, 5 - phase)
        with self.assertRaises(driver.old.Refused):
            driver.old.history_matches(raw, intent, uid, pair, 1)

    def test_capture_replacement_and_oversize_are_not_adopted(self):
        with tempfile.TemporaryDirectory() as root:
            effect = driver.CapturingEffects(Path(root), Path("/run/user/1"), driver.scenario("mixed")[1])
            try:
                capture = effect.capture("cut", ".stdout")
                os.write(capture[1], b"private synthetic data")
                self.assertEqual(effect.capture_read(capture), b"private synthetic data")
                capture[0].rename(Path(root) / "displaced")
                capture[0].write_bytes(b"private synthetic data")
                capture[0].chmod(0o600)
                with self.assertRaises(driver.old.Refused):
                    effect.capture_read(capture)
                other = effect.capture("recover", ".stdout")
                os.ftruncate(other[1], driver.old.MAX_FRAME + 1)
                with self.assertRaises(driver.old.Refused):
                    effect.capture_read(other)
            finally:
                effect.close_completed()

    def test_effect_once_and_known_exit_do_not_create_retry_authority(self):
        class Child:
            returncode = 2
            def poll(self):
                return 2
        with tempfile.TemporaryDirectory() as root:
            effect = driver.CapturingEffects(Path(root), Path("/run/user/1"), driver.scenario("mixed")[1])
            try:
                with mock.patch.object(driver.old.subprocess, "Popen", return_value=Child()) as spawn:
                    with self.assertRaises(driver.old.Refused):
                        effect.once("backup", [str(driver.old.ELF), "developer", "backup-current",
                                               "--confirm-private-export"])
                    self.assertEqual(effect.codes["backup"], 2)
                    self.assertEqual(len(effect.children), 1)
                    self.assertEqual(len(effect.capture_fds), 2)
                    with self.assertRaises(driver.old.Refused):
                        effect.once("backup", [str(driver.old.ELF), "developer", "backup-current",
                                               "--confirm-private-export"])
                    self.assertEqual(spawn.call_count, 1)
            finally:
                effect.close_completed()

    def test_poll_uncertainty_parks_with_originals_and_no_second_child(self):
        class Child:
            def poll(self):
                raise RuntimeError("private exception detail")
        with tempfile.TemporaryDirectory() as root:
            effect = driver.CapturingEffects(Path(root), Path("/run/user/1"), driver.scenario("mixed")[1])
            try:
                with mock.patch.object(driver.old.subprocess, "Popen", return_value=Child()) as spawn, \
                        mock.patch.object(driver.old, "park", side_effect=Parked) as parked:
                    with self.assertRaises(Parked):
                        effect.once("backup", [str(driver.old.ELF), "developer", "backup-current",
                                               "--confirm-private-export"])
                    self.assertEqual(spawn.call_count, 1)
                    self.assertEqual(len(effect.children), 1)
                    self.assertEqual(len(effect.capture_fds), 2)
                    self.assertTrue(all(os.fstat(fd) for fd in effect.capture_fds))
                    self.assertNotIn("private", parked.call_args.args[0])
            finally:
                effect.close_completed()

    def test_finite_deadline_parks_without_kill_or_retry(self):
        child = mock.Mock()
        child.poll.return_value = None
        with tempfile.TemporaryDirectory() as root:
            effect = driver.CapturingEffects(Path(root), Path("/run/user/1"), driver.scenario("mixed")[1])
            try:
                with mock.patch.object(driver.old.subprocess, "Popen", return_value=child) as spawn, \
                        mock.patch.object(driver.old.time, "monotonic", side_effect=[0, 150]), \
                        mock.patch.object(driver.old, "park", side_effect=Parked):
                    with self.assertRaises(Parked):
                        effect.once("cut", driver.commands("mixed")[0])
                    self.assertEqual(spawn.call_count, 1)
                    self.assertIs(effect.children[0], child)
                    self.assertTrue(all(os.fstat(fd) for fd in effect.capture_fds))
                    self.assertNotIn("cut", effect.codes)
                    child.kill.assert_not_called()
                    child.terminate.assert_not_called()
            finally:
                effect.close_completed()

    def test_postexit_capture_replacement_parks_and_retains_original(self):
        with tempfile.TemporaryDirectory() as root:
            effect = driver.CapturingEffects(Path(root), Path("/run/user/1"), driver.scenario("mixed")[1])
            child = mock.Mock(returncode=0)

            def displaced():
                path = Path(root) / "cut.stdout"
                path.rename(Path(root) / "displaced")
                path.write_bytes(driver.scenario("mixed")[5])
                path.chmod(0o600)
                return 0

            child.poll.side_effect = displaced
            try:
                with mock.patch.object(driver.old.subprocess, "Popen", return_value=child) as spawn, \
                        mock.patch.object(driver.old, "park", side_effect=Parked):
                    with self.assertRaises(Parked):
                        effect.once("cut", driver.commands("mixed")[0])
                    self.assertEqual(spawn.call_count, 1)
                    self.assertEqual(effect.codes["cut"], 0)
                    self.assertIs(effect.children[0], child)
                    self.assertEqual(os.pread(effect.capture_fds[0], 1024, 0), b"")
                    self.assertNotEqual(os.fstat(effect.capture_fds[0]).st_ino,
                                        (Path(root) / "cut.stdout").stat().st_ino)
                    child.kill.assert_not_called()
                    child.terminate.assert_not_called()
            finally:
                effect.close_completed()

    def test_absence_is_not_a_dangling_symlink(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "terminal"
            self.assertTrue(driver.absent(path))
            path.symlink_to(Path(root) / "missing")
            self.assertFalse(driver.absent(path))


if __name__ == "__main__":
    unittest.main()
