"""Synthetic immutable lineage controls; no daemon, account or CLI process."""
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from tests.first_abort_cli import lineage


class LineageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='.t4-lineage-', dir=Path.home())
        self.addCleanup(self.temp.cleanup)
        self.home = Path(self.temp.name)
        roots = {'HOME': self.home, 'CONFIG': self.home / 'config', 'STATE': self.home / 'state',
                 'ARTIFACTS': self.home / 'artifacts', 'RUNTIME': self.home / 'runtime',
                 'STAGE': self.home / 'state/restore-pair.pending', 'UID': os.getuid()}
        p = patch.multiple(lineage, **roots)
        p.start()
        self.addCleanup(p.stop)
        for name in ('CONFIG', 'STATE', 'ARTIFACTS', 'RUNTIME', 'STAGE'):
            getattr(lineage, name).mkdir(mode=0o700)
        for root, names in ((lineage.ARTIFACTS, ('archive.ovb', 'request.json', 'setup.json')),
                            (lineage.STATE, ('ownership.json', 'desired.json', 'restore-decision.intent')),
                            (lineage.RUNTIME, ('owner.lock',)),
                            (lineage.STAGE, ('old-profiles.json', 'old-route-template.yaml',
                             'new-profiles.json', 'new-route-template.yaml', 'ready.bin'))):
            for name in names:
                self.member(root / name, name.encode())
        self.member(lineage.CONFIG / 'profiles.json', b'new-profiles.json')
        self.member(lineage.CONFIG / 'route-template.yaml', b'old-route-template.yaml')
        self.obj = lineage.Lineage()
        self.addCleanup(self.close)

    def close(self):
        for fd in self.obj.held:
            os.close(fd)

    def member(self, path, data):
        path.write_bytes(data)
        path.chmod(0o600)

    def replace(self, path, data):
        replacement = path.parent / 'synthetic-new'
        self.member(replacement, data)
        replacement.replace(path)

    def complete(self):
        self.replace(lineage.CONFIG / 'profiles.json', b'old-profiles.json')
        self.replace(lineage.CONFIG / 'route-template.yaml', b'old-route-template.yaml')
        self.member(lineage.STATE / 'restore-decision.terminal', b'native-verification-is-outer-precondition')
        self.obj.first_completed()

    def test_single_expected_transition_then_original_identity_reentry(self):
        self.obj.before_first()
        self.complete()
        self.obj.reentry_boundary()
        self.assertEqual(self.obj.phase, 'aborted')
        with self.assertRaises(lineage.Refused):
            self.obj.first_completed()

    def test_same_bytes_immutable_stage_swap_seals_before_live_adoption(self):
        p = lineage.STAGE / 'old-profiles.json'
        self.replace(p, p.read_bytes())
        with self.assertRaises(lineage.Refused):
            self.obj.first_completed()
        self.assertEqual(self.obj.phase, 'refused')
        with patch.object(lineage.os, 'stat') as queried:
            with self.assertRaises(lineage.Refused):
                self.obj.reentry_boundary()
        queried.assert_not_called()

    def test_same_byte_live_swap_before_first_refuses(self):
        p = lineage.CONFIG / 'profiles.json'
        self.replace(p, p.read_bytes())
        with self.assertRaises(lineage.Refused):
            self.obj.before_first()
        self.assertEqual(self.obj.phase, 'refused')

    def test_final_same_byte_live_swap_refuses(self):
        self.complete()
        p = lineage.CONFIG / 'profiles.json'
        self.replace(p, p.read_bytes())
        with self.assertRaises(lineage.Refused):
            self.obj.reentry_boundary()
        self.assertEqual(self.obj.phase, 'refused')

    def test_final_same_byte_terminal_swap_refuses(self):
        self.complete()
        p = lineage.STATE / 'restore-decision.terminal'
        self.replace(p, p.read_bytes())
        with self.assertRaises(lineage.Refused):
            self.obj.reentry_boundary()
        self.assertEqual(self.obj.phase, 'refused')

    def test_original_intent_replacement_is_not_adopted(self):
        p = lineage.STATE / 'restore-decision.intent'
        self.replace(p, p.read_bytes())
        with self.assertRaises(lineage.Refused):
            self.obj.first_completed()
        self.assertEqual(self.obj.phase, 'refused')

    def test_unexpected_terminal_before_cli_refuses(self):
        self.member(lineage.STATE / 'restore-decision.terminal', b'foreign')
        with self.assertRaises(lineage.Refused):
            self.obj.before_first()

    def test_extra_stage_member_refuses(self):
        self.member(lineage.STAGE / 'unexpected', b'x')
        with self.assertRaises(lineage.Refused):
            self.obj.before_first()


if __name__ == '__main__':
    unittest.main()
