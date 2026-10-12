# SPDX-License-Identifier: MIT
"""Pure observer contracts; never invokes the installed application or a VM."""
import importlib.util
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location(
    "restore_host", Path(__file__).parent / "restore_host/old_recovery.py")
driver = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(driver)


class RestoreObserver(unittest.TestCase):
    def test_park_step_does_not_unwind_on_broken_output_or_interrupted_sleep(self):
        with mock.patch('builtins.print', side_effect=BrokenPipeError), \
                mock.patch.object(driver.time, 'sleep', side_effect=KeyboardInterrupt):
            driver.parked_step('public phase')

    def test_observation_read_is_bounded(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'capture'
            path.write_bytes(b'12345')
            self.assertEqual(driver.bounded(path, 5), b'12345')
            with self.assertRaises(driver.Refused):
                driver.bounded(path, 4)

    def test_fixture_effects_cannot_be_repeated(self):
        with tempfile.TemporaryDirectory() as directory:
            effects = driver.Effects(Path(directory), Path('/run/user/1234'))
            effects.used.add('stop')
            with mock.patch.object(driver.subprocess, 'Popen') as process:
                with self.assertRaises(driver.Refused):
                    effects.once('stop', ['/usr/bin/systemctl'])
                process.assert_not_called()

    def test_history_binds_uid_transaction_and_both_old_members(self):
        def wrap(magic, body, domain):
            raw = magic + body
            return raw + hashlib.sha256(domain + raw).digest()
        members = (b'synthetic old store', b'synthetic old template')
        fields = (1).to_bytes(8, 'big') + bytes(89)
        intent = wrap(b'OVRDEC01', b'\x01' + fields, b'omavless-restore-decision-v1\0')
        terminal = wrap(b'OVRDEC01', b'\x03' + fields, b'omavless-restore-decision-v1\0')
        bindings = b''.join(len(v).to_bytes(4, 'big') + hashlib.sha256(v).digest() for v in members)
        receipt = wrap(b'OVRFIN01', terminal + bindings, b'omavless-restore-finalization-v1\0')
        closure = wrap(b'OVRCLS01', receipt, b'omavless-restore-closure-v1\0')
        ticket = wrap(b'OVRDSP01', (1234).to_bytes(4, 'little') + bytes(4)
                      + (1).to_bytes(8, 'little') + closure,
                      b'omavless-restore-disposition-candidate-v1\0')
        complete = wrap(b'OVRDON01', ticket, b'omavless-restore-disposition-complete-v1\0')
        driver.history_matches(complete, intent, 1234, members)
        for raw, uid, old in ((complete[:-1], 1234, members), (complete, 1235, members),
                              (complete, 1234, (b'new', members[1]))):
            with self.assertRaises(driver.Refused):
                driver.history_matches(raw, intent, uid, old)

    def test_closed_command_surface(self):
        for command in ([], ["recover"], ["stop"], ["preflight", "anything"]):
            with self.assertRaises(driver.Refused):
                driver.main(command)

    def test_strict_frame(self):
        self.assertEqual(driver.decode(b'{"x":1}\n'), {"x": 1})
        for raw in (b'', b'{}', b'{}\n{}\n', b'[]\n', b'{"x":1,"x":2}\n',
                    b'{"x":NaN}\n', b'{"x":"\xff"}\n', b' ' * driver.MAX_FRAME + b'\n'):
            with self.assertRaises(driver.Refused):
                driver.decode(raw)

    def test_off_requires_coherent_original(self):
        hello = {"revision": 1, "result": {"instanceId": "fixture", "runtimeOwnership": True}}
        status = {"revision": 1, "result": {"runtimeOwnership": True,
                  "desired": "disconnected", "actual": "disconnected", "transition": None}}
        driver.off_triplet((42, hello), (42, status), (42, hello))
        for field, value in (("actual", "connected"), ("transition", "busy"),
                             ("runtimeOwnership", False)):
            altered = json.loads(json.dumps(status))
            altered["result"][field] = value
            with self.assertRaises(driver.Refused):
                driver.off_triplet((42, hello), (42, altered), (42, hello))
        with self.assertRaises(driver.Refused):
            driver.off_triplet((42, hello), (43, status), (42, hello))

    def test_held_detects_replacement_and_does_not_modify(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "old"
            path.write_bytes(b"synthetic")
            path.chmod(0o600)
            held = driver.Held(path, os.getuid(), 64)
            try:
                held.unchanged()
                replacement = Path(directory) / "replacement"
                replacement.write_bytes(b"synthetic")
                replacement.chmod(0o600)
                replacement.replace(path)
                with self.assertRaises(driver.Refused):
                    held.unchanged()
                self.assertEqual(path.read_bytes(), b"synthetic")
            finally:
                held.close()

    def test_private_reader_refuses_symlinks_permissions_and_size(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "old"
            path.write_bytes(b"synthetic")
            path.chmod(0o644)
            with self.assertRaises(driver.Refused):
                driver.Held(path, os.getuid(), 64)
            path.chmod(0o600)
            with self.assertRaises(driver.Refused):
                driver.Held(path, os.getuid(), 2)
            link = Path(directory) / "link"
            link.symlink_to(path)
            with self.assertRaises(driver.Refused):
                driver.Held(link, os.getuid(), 64)


if __name__ == "__main__":
    unittest.main()
