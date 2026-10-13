# SPDX-License-Identifier: MIT
"""Finite DATA/mock controls. No native authority, VM or installed effects."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location(
    "final_new_driver", Path(__file__).parent / "restore_host/final_new_recovery.py")
driver = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(driver)
old, loss = driver.old, driver.loss


class Parked(BaseException):
    pass


def wrap(magic, body, domain):
    raw = magic + body
    return raw + hashlib.sha256(domain + raw).digest()


def committed(old_pair, new_pair, desired, owner, phase=2, nonce=bytes([9]) * 16):
    # Model only the after-cut record, never a filesystem stage/Intent fixture.
    members = old_pair + new_pair
    ready = b"OVRPAIR1" + b"".join(len(item).to_bytes(4, "big") for item in members)
    ready += b"".join(hashlib.sha256(item).digest() for item in members)
    fields = (json.loads(owner)["generation"].to_bytes(8, "big")
              + json.loads(desired)["generation"].to_bytes(8, "big") + b"\x01"
              + hashlib.sha256(desired).digest() + hashlib.sha256(ready).digest() + nonce)
    terminal = wrap(b"OVRDEC01", bytes([phase]) + fields, b"omavless-restore-decision-v1\0")
    pair = b"".join(len(item).to_bytes(4, "big") + hashlib.sha256(item).digest() for item in new_pair)
    receipt = wrap(b"OVRFIN01", terminal + pair, b"omavless-restore-finalization-v1\0")
    return wrap(b"OVRCLS01", receipt, b"omavless-restore-closure-v1\0")


def history(closure, uid, generation=2):
    ticket = wrap(b"OVRDSP01", uid.to_bytes(4, "little") + bytes(4)
                  + generation.to_bytes(8, "little") + closure,
                  b"omavless-restore-disposition-candidate-v1\0")
    return wrap(b"OVRDON01", ticket, b"omavless-restore-disposition-complete-v1\0")


def request(home=driver.HOME):
    return json.dumps({"schema": 1, "archive": str(home / ".cache" / driver.EVIDENCE / "new.ovb"),
                       "passphrase": "synthetic test passphrase"}).encode()


class FinalNew(unittest.TestCase):
    def test_exact206_commands_and_private_closed_archive_request(self):
        self.assertEqual(driver.ACCOUNT, "ovrestore206")
        self.assertEqual(driver.HOME, Path("/home/ovrestore206"))
        self.assertEqual(driver.EVIDENCE, "restore099-pc-final-new")
        self.assertEqual(old.ACCOUNT, "ovrestore202")
        self.assertEqual(old.FIXED_ACCOUNTS[driver.ACCOUNT], driver.HOME)
        driver.phase_command("cut", driver.CUT, request())
        driver.phase_command("recover", driver.RECOVER, request())
        for raw in [b'{"schema":1}', b"{}", request(Path("/home/ovrestore205")),
                    request()[:-1] + b',"step":0}']:
            with self.assertRaises(old.Refused):
                driver.phase_command("cut", driver.CUT, raw)
        for phase, argv in [("cut", driver.CUT + ["0"]), ("pause", driver.CUT),
                            ("stop", ["/bin/sh"]), ("recover", driver.previous.RECOVER)]:
            with self.assertRaises(old.Refused):
                driver.phase_command(phase, argv, request())

    def test_cli_only_preflight_positive_no_scope_or_retained_selector(self):
        with mock.patch.object(driver, "preflight") as before, mock.patch.object(driver, "positive") as effect:
            driver.main(["preflight"])
            driver.main(["positive"])
            before.assert_called_once_with()
            effect.assert_called_once_with()
        for argv in [[], ["recover-retained"], ["positive", "205"], ["positive", "--home", "/root"]]:
            with self.assertRaises(old.Refused):
                driver.main(argv)

    def test_cut_marker_not_completed_restore_or_unreached_reply(self):
        driver.cut_result(driver.CUT_MARKER)
        for raw in [b"", driver.CUT_MARKER[:-1], driver.CUT_MARKER + b"extra",
                    b"t4_current_pair_completed\n", b'{"cutReachedStillFenced":false}\n',
                    driver.previous.CUT_MARKER]:
            with self.assertRaises(old.Refused):
                driver.cut_result(raw)

    def test_committed_closure_exact_outputs_stage_and_boundaries_data(self):
        before, after = [b"OLD store", b"OLD template"], [b"NEW store", b"NEW template"]
        desired = b'{"generation":7,"connected":false,"profileId":"","mode":"rule"}'
        owner = b'{"generation":2,"phase":"rust"}'
        raw = committed(before, after, desired, owner)
        driver.closure_matches(raw, before, after, desired, owner)
        for value, a, b, d, o in [
            (raw[:-1], before, after, desired, owner),
            (committed(before, after, desired, owner, 3), before, after, desired, owner),
            (committed(before, after, desired, owner, nonce=bytes(16)), before, after, desired, owner),
            (raw, [b"wrong", before[1]], after, desired, owner),
            (raw, before, [after[0], b"wrong"], desired, owner),
            (raw, before, after, b"{}", owner), (raw, before, after, desired, b"{}"),
        ]:
            with self.assertRaises(old.Refused):
                driver.closure_matches(value, a, b, d, o)
        # An observed nonzero nonce can vary; no original Intent/nonce custody
        # exists to assert equality against. It remains comparison DATA only.
        other = committed(before, after, desired, owner, nonce=bytes([8]) * 16)
        driver.closure_matches(other, before, after, desired, owner)
        for pair in [[before[0], after[1]], [after[0], before[1]]]:
            with self.assertRaises(old.Refused):
                driver.stage_digest(before, pair)

    def test_actual_nested_history_binds_exact_observed_closure_uid_generation(self):
        before, after = [b"old", b"old template"], [b"new", b"new template"]
        desired, owner = b'{"generation":7}', b'{"generation":2}'
        raw = committed(before, after, desired, owner)
        data = history(raw, os.getuid())
        driver.history_matches(data, raw, os.getuid())
        other = committed(before, after, desired, owner, nonce=bytes([8]) * 16)
        for item, selected, uid in [(data[:-1], raw, os.getuid()), (data, other, os.getuid()),
                                    (data, raw, os.getuid() + 1), (history(raw, os.getuid(), 3), raw, os.getuid())]:
            with self.assertRaises(old.Refused):
                driver.history_matches(item, selected, uid)
        aborted = committed(before, after, desired, owner, 3)
        with self.assertRaises(old.Refused):
            driver.history_matches(history(aborted, os.getuid()), aborted, os.getuid())

    def test_both_prefixes_all_history_and_dangling_slots_refuse(self):
        with tempfile.TemporaryDirectory() as root:
            config, state = Path(root) / "config", Path(root) / "state"
            config.mkdir()
            state.mkdir()
            driver.inventory(config, state, ())
            for parent, name in [(state, name) for name in driver.previous.HISTORY] + [
                (state, ".restore-foreign"), (state, "restore-foreign"),
                (state, "routing-preset.pending.json"), (config, ".restore-foreign"),
                (config, "restore-foreign")]:
                path = parent / name
                path.symlink_to(parent / "missing")
                with self.assertRaises(old.Refused):
                    driver.inventory(config, state, ())
                path.unlink()

    def test_unsafe_cache_parent_and_prior_scope_before_runtime_reads(self):
        with tempfile.TemporaryDirectory() as root:
            home, actual = Path(root), Path(root) / "actual"
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
                (parent / driver.EVIDENCE).mkdir(mode=0o700)
                with mock.patch.object(old, "account_scope", return_value=(os.getuid(), home)), \
                        mock.patch.object(old, "ordinary") as read:
                    with self.assertRaises(old.Refused):
                        driver.preflight()
                    read.assert_not_called()

    def test_actual_capture_stderr_diagnostic_exit_not_authority_no_retry(self):
        for code in (0, 2):
            child = mock.Mock(returncode=code)
            child.poll.return_value = code
            with tempfile.TemporaryDirectory() as root:
                effect = driver.Effects(Path(root), Path("/run/user/1"), driver.HOME)
                def spawn(*_, **kwargs):
                    os.write(kwargs["stderr"], b"RESTORE_RELEASE_DISPOSITION_VERIFIED\n")
                    return child
                try:
                    with mock.patch.object(old.subprocess, "Popen", side_effect=spawn) as launched:
                        if code == 0:
                            self.assertEqual(effect.once("recover", driver.RECOVER, request()), b"")
                        else:
                            with self.assertRaises(old.Refused):
                                effect.once("recover", driver.RECOVER, request())
                        self.assertEqual(effect.codes["recover"], code)
                        self.assertIs(effect.children[0], child)
                        with self.assertRaises(old.Refused):
                            effect.once("recover", driver.RECOVER, request())
                        self.assertEqual(launched.call_count, 1)
                        child.kill.assert_not_called()
                        child.terminate.assert_not_called()
                finally:
                    effect.close_completed()

    def test_actual_poll_loss_deadline_and_oversized_capture_keep_originals(self):
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
                    with mock.patch.object(old.subprocess, "Popen", side_effect=spawn) as launched, \
                            mock.patch.object(old.time, "monotonic", side_effect=[0, 150]), \
                            mock.patch.object(old, "park", side_effect=Parked):
                        with self.assertRaises(Parked):
                            effect.once("cut", driver.CUT, request())
                        self.assertEqual(launched.call_count, 1)
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
                              (run / "omavless-login.receipt", b"packaged login")]:
                loss._private_write(path, raw)
            phases, held, earned = [], [], {}
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
                        earned["backup-store"] = (config / "profiles.json").read_bytes()
                        loss._private_write(home / ".cache" / driver.EVIDENCE / "new.ovb", b"archive DATA")
                    elif phase == "preset":
                        replace(config / "route-template.yaml", b"Roscom template")
                    elif phase == "import":
                        value = dict(initial, routingPreset="roscomvpn-default", profiles=[
                            {"name": "Loss-cut fixture", "uri": loss.SYNTHETIC_URI}])
                        replace(config / "profiles.json", json.dumps(value).encode())
                    elif phase == "cut":
                        if fault in ("broken", "nonzero"):
                            if fault == "nonzero":
                                raise old.Refused()
                            return b""
                        before = [(config / "profiles.json").read_bytes(), b"Roscom template"]
                        after = [earned["backup-store"], b"China template"]
                        replace(config / "profiles.json", after[0])
                        replace(config / "route-template.yaml", after[1])
                        earned["closure"] = committed(before, after, desired, owner, 3 if fault == "aborted" else 2)
                        loss._private_write(state / "restore-closure.complete", earned["closure"])
                        if fault == "receipt":
                            loss._private_write(state / "restore-finalization.pending", b"unexpected")
                        return driver.CUT_MARKER
                    elif phase == "recover":
                        old.require("stop" in phases and "closure" in earned)
                        if fault == "new-replacement":
                            replace(config / "profiles.json", earned["backup-store"])
                        (state / "restore-closure.complete").unlink()
                        loss._private_write(state / driver.previous.HISTORY[0], history(earned["closure"], os.getuid()))
                    return b'{"ok":true}\n'
                def close_completed(self):
                    pass
            original_held = old.Held
            def pin(*args):
                item = original_held(*args)
                held.append(item)
                name = Path(args[0]).name
                # Never look up fabricated/retired stage, Intent or terminal.
                self.assertNotIn(name, {name for name, _ in loss.STAGED} | {
                    "restore-decision.intent", "restore-decision.terminal"})
                if name == "restore-closure.complete":
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
            def process_identity(*_):
                return "changed" if fault == "process-changed" and "import" in phases else "original"
            try:
                with mock.patch.object(driver, "HOME", home), \
                        mock.patch.object(old, "account_scope", return_value=(os.getuid(), run)), \
                        mock.patch.object(old, "Held", side_effect=pin), \
                        mock.patch.object(driver, "Effects", MockEffects), \
                        mock.patch.object(old, "ordinary", side_effect=ordinary), \
                        mock.patch.object(old, "reply", side_effect=reply), \
                        mock.patch.object(old, "no_network"), \
                        mock.patch.object(old, "process_identity", side_effect=process_identity), \
                        mock.patch.object(old, "manager", side_effect=manager), \
                        mock.patch.object(old, "park", side_effect=Parked):
                    if fault:
                        with self.assertRaises(Parked):
                            driver.positive()
                        if fault != "new-replacement":
                            self.assertNotIn("stop", phases)
                            self.assertNotIn("recover", phases)
                        self.assertTrue(all(item.fd is not None for item in held))
                    else:
                        driver.positive()
                        self.assertEqual(phases, ["backup", "preset", "import", "cut", "stop", "recover"])
                        self.assertTrue(all(item.fd is None for item in held))
                    self.assertNotIn("pause", phases)
                    self.assertFalse(any(p.name in {name for name, _ in loss.STAGED}
                                         or p.name.startswith("restore-decision.") for p in state.iterdir()))
            finally:
                for item in held:
                    item.close()

    def test_sequence_only_phase_earned_closure_history_no_stage_or_intent(self):
        self.sequence()

    def test_broken_cut_reply_parks_without_stop_recover(self):
        self.sequence("broken")

    def test_known_nonzero_cut_parks_without_stop_recover(self):
        self.sequence("nonzero")

    def test_original_process_change_refuses_before_cut(self):
        self.sequence("process-changed")

    def test_aborted_closure_refuses_before_stop(self):
        self.sequence("aborted")

    def test_unretired_receipt_refuses_before_stop(self):
        self.sequence("receipt")

    def test_same_byte_new_inode_replacement_after_recovery_refuses(self):
        self.sequence("new-replacement")


if __name__ == "__main__":
    unittest.main()
