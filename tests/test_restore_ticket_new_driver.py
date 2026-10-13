# SPDX-License-Identifier: MIT
"""Finite ticket DATA/phase-earned mocks; no native or installed effects."""
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
driver = load("ticket_driver", ROOT / "restore_host/ticket_new_recovery.py")
fixture = load("ticket_data", ROOT / "test_restore_final_new_driver.py")
old, loss = driver.old, driver.loss


def ticket(closure, uid, generation=2):
    return fixture.history(closure, uid, generation)[8:-32]


def request(home=driver.HOME):
    return json.dumps({"schema": 1, "archive": str(home / ".cache" / driver.EVIDENCE / "new.ovb"),
                       "passphrase": "synthetic test passphrase"}).encode()


class Parked(BaseException):
    pass


class TicketDriver(unittest.TestCase):
    def test_closed208_commands_request_no_other_scope_or_cut(self):
        self.assertEqual(driver.ACCOUNT, "ovrestore208")
        self.assertEqual(driver.HOME, Path("/home/ovrestore208"))
        self.assertEqual(driver.EVIDENCE, "restore099-pc-ticket-new")
        self.assertEqual(old.ACCOUNT, "ovrestore202")
        self.assertEqual(old.FIXED_ACCOUNTS[driver.ACCOUNT], driver.HOME)
        for phase, command in [("cut", driver.CUT), ("recover", driver.RECOVER)]:
            driver.phase_command(phase, command, request())
        for phase, command, raw in [("cut", driver.shared.CUT, request()),
                                    ("recover", driver.shared.RECOVER, request()),
                                    ("cut", driver.CUT, request(Path("/home/ovrestore207"))),
                                    ("cut", driver.CUT, b'{"schema":1}'),
                                    ("cut", driver.CUT + ["DispositionTicket"], request()),
                                    ("stop", ["/bin/sh"], b"")]:
            with self.assertRaises(old.Refused):
                driver.phase_command(phase, command, raw)

    def test_two_entries_only_no_retry_or_user_path_selector(self):
        with mock.patch.object(driver, "preflight") as before, mock.patch.object(driver, "positive") as effect:
            driver.main(["preflight"])
            driver.main(["positive"])
            before.assert_called_once_with()
            effect.assert_called_once_with()
        for args in [[], ["recover-retained"], ["positive", "207"], ["positive", "--home", "/root"]]:
            with self.assertRaises(old.Refused):
                driver.main(args)

    def test_actual_marker_not_completed_or_unreached_data(self):
        driver.cut_result(driver.CUT_MARKER)
        for raw in [b"", driver.CUT_MARKER[:-1], driver.CUT_MARKER + b"extra",
                    b"t4_current_pair_completed\n", driver.shared.CUT_MARKER,
                    b'{"cutReachedStillFenced":false}\n']:
            with self.assertRaises(old.Refused):
                driver.cut_result(raw)

    def test_ticket_exact_source_model_uid_generation_desired_and_closure(self):
        desired = b'{"generation":7,"connected":false,"profileId":"","mode":"rule"}'
        owner = b'{"generation":2,"phase":"rust"}'
        closure = fixture.committed([b"old", b"old template"], [b"new", b"new template"], desired, owner)
        raw = ticket(closure, os.getuid())
        self.assertEqual(len(raw), 346)
        driver.ticket_matches(raw, closure, desired, owner, os.getuid())
        other = fixture.committed([b"old", b"old template"], [b"new", b"new template"],
                                   desired, owner, nonce=bytes([8]) * 16)
        for value, chosen, d, o, uid in [
            (raw[:-1], closure, desired, owner, os.getuid()),
            (raw, other, desired, owner, os.getuid()),
            (raw, closure, desired, owner, os.getuid() + 1),
            (ticket(closure, os.getuid(), 3), closure, desired, owner, os.getuid()),
            (raw, closure, b'{"generation":8,"connected":false,"profileId":"","mode":"rule"}', owner, os.getuid()),
            (raw, closure, desired, b'{"generation":3,"phase":"rust"}', os.getuid()),
        ]:
            with self.assertRaises(old.Refused):
                driver.ticket_matches(value, chosen, d, o, uid)
        changed = bytearray(raw)
        changed[12] = 1
        changed[-32:] = fixture.hashlib.sha256(b"omavless-restore-disposition-candidate-v1\0" + changed[:-32]).digest()
        with self.assertRaises(old.Refused):
            driver.ticket_matches(bytes(changed), closure, desired, owner, os.getuid())

    def test_closed_complete_torn_or_other_pending_prefix_refuses(self):
        with tempfile.TemporaryDirectory() as root:
            config, state = Path(root) / "config", Path(root) / "state"
            config.mkdir()
            state.mkdir()
            expected = ("restore-closure.complete", driver.TICKET)
            for name in expected:
                loss._private_write(state / name, b"comparison DATA")
            driver.inventory(config, state, expected)
            for parent, name in [(state, "restore-disposition.complete"), (state, "restore-finalization.pending"),
                                  (state, "restore-decision.intent"), (state, ".restore-unknown"),
                                  (state, driver.previous.HISTORY[0]), (config, ".restore-profiles.old")]:
                path = parent / name
                path.symlink_to(parent / "missing")
                with self.assertRaises(old.Refused):
                    driver.inventory(config, state, expected)
                path.unlink()

    def test_private_parent_and_prior_evidence_refuse_before_reads(self):
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
                parent.mkdir(mode=0o700)
                (parent / driver.EVIDENCE).mkdir(mode=0o700)
                with mock.patch.object(old, "account_scope", return_value=(os.getuid(), home)), \
                        mock.patch.object(old, "ordinary") as read:
                    with self.assertRaises(old.Refused):
                        driver.preflight()
                    read.assert_not_called()

    def test_ticket_pin_metadata_inode_bytes_xattr_change_cannot_be_adopted(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / driver.TICKET
            loss._private_write(path, b"bounded DATA")
            item = old.Held(path, os.getuid(), 346)
            try:
                attrs = driver.attributes(item.fd)
                driver.pinned_unchanged(item, attrs)
                with mock.patch.object(driver, "attributes", return_value={"user.fixture": b"changed"}):
                    with self.assertRaises(old.Refused):
                        driver.pinned_unchanged(item, attrs)
                path.rename(Path(root) / "displaced")
                loss._private_write(path, b"bounded DATA")
                with self.assertRaises(old.Refused):
                    driver.pinned_unchanged(item, attrs)
            finally:
                item.close()

    def test_actual_capture_nonempty_diagnostics_code2_never_success_or_retry(self):
        for code in (0, 2):
            child = mock.Mock(returncode=code)
            child.poll.return_value = code
            with tempfile.TemporaryDirectory() as root:
                effect = driver.Effects(Path(root), Path("/run/user/1"), driver.HOME)
                def spawn(*_, **kwargs):
                    os.write(kwargs["stderr"], b"RESTORE_RECOVERY_COMPLETED\n")
                    return child
                try:
                    with mock.patch.object(old.subprocess, "Popen", side_effect=spawn) as started:
                        if code == 0:
                            self.assertEqual(effect.once("recover", driver.RECOVER, request()), b"")
                        else:
                            with self.assertRaises(old.Refused):
                                effect.once("recover", driver.RECOVER, request())
                        self.assertEqual(effect.codes["recover"], code)
                        self.assertIs(effect.children[0], child)
                        with self.assertRaises(old.Refused):
                            effect.once("recover", driver.RECOVER, request())
                        self.assertEqual(started.call_count, 1)
                        child.kill.assert_not_called()
                        child.terminate.assert_not_called()
                finally:
                    effect.close_completed()

    def test_actual_uncertain_child_and_oversize_capture_keep_originals(self):
        for kind in ("poll", "deadline", "oversize"):
            child = mock.Mock(returncode=0)
            child.poll.return_value = None if kind == "deadline" else 0
            if kind == "poll":
                child.poll.side_effect = RuntimeError("private detail")
            with tempfile.TemporaryDirectory() as root:
                effect = driver.Effects(Path(root), Path("/run/user/1"), driver.HOME)
                def spawn(*_, **kwargs):
                    if kind == "oversize":
                        os.ftruncate(kwargs["stderr"], old.MAX_FRAME + 1)
                    return child
                try:
                    with mock.patch.object(old.subprocess, "Popen", side_effect=spawn), \
                            mock.patch.object(old.time, "monotonic", side_effect=[0, 150]), \
                            mock.patch.object(old, "park", side_effect=Parked):
                        with self.assertRaises(Parked):
                            effect.once("cut", driver.CUT, request())
                        self.assertIs(effect.children[0], child)
                        self.assertTrue(all(os.fstat(fd) for fd in effect.capture_fds))
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
            desired = b'{"generation":7,"connected":false,"profileId":"","mode":"rule"}'
            owner = b'{"generation":2,"phase":"rust"}'
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
                    elif phase == "preset":
                        replace(config / "route-template.yaml", b"Roscom template")
                    elif phase == "import":
                        value = dict(initial, routingPreset="roscomvpn-default", profiles=[{
                            "name": "Loss-cut fixture", "uri": loss.SYNTHETIC_URI}])
                        replace(config / "profiles.json", json.dumps(value).encode())
                    elif phase == "cut":
                        if fault in ("broken", "nonzero"):
                            if fault == "nonzero":
                                raise old.Refused()
                            return b""
                        before = [(config / "profiles.json").read_bytes(), b"Roscom template"]
                        after = [earned["new"], b"China template"]
                        closure = fixture.committed(before, after, desired, owner)
                        raw = ticket(closure, os.getuid() + (1 if fault == "uid" else 0))
                        if fault == "crossed":
                            other = fixture.committed(before, after, desired, owner, nonce=bytes([8]) * 16)
                            raw = ticket(other, os.getuid())
                        replace(config / "profiles.json", after[0])
                        replace(config / "route-template.yaml", after[1])
                        earned["closure"], earned["ticket"] = closure, raw
                        loss._private_write(state / "restore-closure.complete", closure)
                        loss._private_write(state / driver.TICKET, raw)
                        if fault == "complete-present":
                            loss._private_write(state / "restore-disposition.complete", b"unexpected prefix")
                        return driver.CUT_MARKER
                    elif phase == "stop" and fault == "ticket-replaced":
                        replace(state / driver.TICKET, earned["ticket"])
                    elif phase == "recover":
                        old.require("stop" in phases and "ticket" in earned)
                        (state / "restore-closure.complete").unlink()
                        (state / driver.TICKET).unlink()
                        if fault == "new-replaced":
                            replace(config / "profiles.json", earned["new"])
                        raw = fixture.wrap(b"OVRDON01", earned["ticket"],
                                           b"omavless-restore-disposition-complete-v1\0")
                        loss._private_write(state / driver.previous.HISTORY[0], raw)
                    return b'{"ok":true}\n'
                def close_completed(self):
                    pass
            original_held = old.Held
            def pin(*args):
                item = original_held(*args)
                pins.append(item)
                name = Path(args[0]).name
                self.assertNotIn(name, {leaf for leaf, _ in loss.STAGED} | {
                    "restore-decision.intent", "restore-decision.terminal"})
                if name in (driver.TICKET, "restore-closure.complete"):
                    self.assertIn("cut", phases)
                if name.startswith("restore-disposition.history"):
                    self.assertIn("recover", phases)
                return item
            def ordinary(*_):
                self.assertTrue("cut" not in phases or "recover" in phases)
                return 2147483000, {"revision": 2, "result": {
                    "instanceId": "fresh" if "recover" in phases else "original"}}
            def reply(_run, _uid, method, _request):
                self.assertIn("cut", phases)
                self.assertNotIn("stop", phases)
                result = {"runtimeOwnership": False, "desired": "disconnected",
                          "actual": "disconnected", "transition": None}
                if method == "system.hello":
                    result["instanceId"] = "original"
                return 2147483000, {"revision": 2, "result": result}
            def manager(*_):
                stopped = "stop" in phases
                return {"MainPID": "0" if stopped else "2147483000",
                        "ActiveState": "inactive" if stopped else "active", "SubState": "dead"}
            try:
                with mock.patch.object(driver, "HOME", home), \
                        mock.patch.object(old, "account_scope", return_value=(os.getuid(), run)), \
                        mock.patch.object(driver, "Effects", MockEffects), \
                        mock.patch.object(old, "Held", side_effect=pin), \
                        mock.patch.object(old, "ordinary", side_effect=ordinary), \
                        mock.patch.object(old, "reply", side_effect=reply), \
                        mock.patch.object(old, "manager", side_effect=manager), \
                        mock.patch.object(old, "process_identity", return_value="original"), \
                        mock.patch.object(old, "no_network"), \
                        mock.patch.object(old, "park", side_effect=Parked):
                    if fault:
                        with self.assertRaises(Parked):
                            driver.positive()
                        if fault not in ("ticket-replaced", "new-replaced"):
                            self.assertNotIn("stop", phases)
                        if fault != "new-replaced":
                            self.assertNotIn("recover", phases)
                        self.assertTrue(all(item.fd is not None for item in pins))
                    else:
                        driver.positive()
                        self.assertEqual(phases, ["backup", "preset", "import", "cut", "stop", "recover"])
                        self.assertTrue(all(item.fd is None for item in pins))
                        self.assertEqual((state / driver.previous.HISTORY[0]).read_bytes()[8:-32], earned["ticket"])
            finally:
                for item in pins:
                    item.close()

    def test_phase_earned_ticket_closure_and_exact_history_no_fabricated_prefix(self):
        self.sequence()

    def test_missing_cut_reply_cannot_stop_or_recover(self):
        self.sequence("broken")

    def test_known_nonzero_cut_cannot_stop_or_recover(self):
        self.sequence("nonzero")

    def test_wrong_ticket_uid_refuses_before_stop(self):
        self.sequence("uid")

    def test_crossed_ticket_closure_refuses_before_stop(self):
        self.sequence("crossed")

    def test_complete_present_refuses_without_repair_before_stop(self):
        self.sequence("complete-present")

    def test_same_byte_ticket_substitution_after_stop_cannot_recover(self):
        self.sequence("ticket-replaced")

    def test_new_inode_substitution_after_recovery_cannot_claim_success(self):
        self.sequence("new-replaced")


if __name__ == "__main__":
    unittest.main()
