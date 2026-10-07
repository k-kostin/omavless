"""Mocked delayed-return controls; no process, ELF, account, service or guest."""
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

from tests.first_abort_cli import root_guard as guard
from tests.first_abort_cli import stage_loader as loader


class Boundaries(unittest.TestCase):
    def setUp(self):
        self.core = SimpleNamespace(UNCERTAIN=False, retained=[])
        def quarantine(child):
            self.core.UNCERTAIN = True
            self.core.retained.append(child)
            raise guard.Refused()
        self.core.quarantine = quarantine
        for item in (patch.object(guard, 'core', self.core),
                     patch.object(guard, 'DEADLINE', float('inf')),
                     patch.object(loader, 'DEADLINE', float('inf'))):
            item.start(); self.addCleanup(item.stop)

    def test_late_observation_never_reaps_and_late_reap_never_returns_known_zero(self):
        for late in ('waitid', 'waitpid'):
            self.core.UNCERTAIN = False
            child = SimpleNamespace(pid=123, returncode=None)
            clock = [0.0]
            seen = SimpleNamespace(si_pid=123, si_code=guard.os.CLD_EXITED, si_status=0)
            def observe(*_):
                if late == 'waitid': clock[0] = 6.0
                return seen
            def reap(*_):
                clock[0] = 6.0
                return (123, 0)
            with patch.object(guard.time, 'monotonic', side_effect=lambda:clock[0]), \
                 patch.object(guard.os, 'waitid', side_effect=observe) as observed, \
                 patch.object(guard.os, 'waitpid', side_effect=reap) as reaped:
                with self.assertRaises(guard.Refused): guard.await_allowed(child, 5, (0,))
                with self.assertRaises(guard.Refused): guard.await_allowed(child, 5, (0,))
                observed.assert_called_once()
                self.assertEqual(reaped.call_count, int(late == 'waitpid'))
                self.assertIsNone(child.returncode)
                self.assertIs(self.core.retained[-1], child)

    def test_raw_reap_aliases_never_authorize_output_or_second_query(self):
        for code, allowed in ((0, (0,)), (1, (0, 1)), (2, (2,))):
            for raw in (float(code << 8), False, (code << 8) + 65536, -65536):
                self.core.UNCERTAIN = False
                child = SimpleNamespace(pid=123, returncode=None)
                seen = SimpleNamespace(si_pid=123, si_code=guard.os.CLD_EXITED, si_status=code)
                with patch.object(guard.os, 'waitid', return_value=seen) as observed, \
                     patch.object(guard.os, 'waitpid', return_value=(123, raw)) as reaped:
                    with self.assertRaises(guard.Refused): guard.await_allowed(child, 5, allowed)
                    with self.assertRaises(guard.Refused): guard.await_allowed(child, 5, allowed)
                    observed.assert_called_once(); reaped.assert_called_once()
                    self.assertIsNone(child.returncode)

    def test_spawn_returning_late_retains_exact_original_before_wait_or_output(self):
        obj = guard.Guard.__new__(guard.Guard)
        obj.count = obj.log_bytes = 0
        obj.evidence = Mock()
        obj.evidence.create.side_effect = [10, 11]
        child = SimpleNamespace(pid=123, returncode=None)
        def spawn(*_, **__):
            guard.DEADLINE = 0
            return child
        self.core.spawn = Mock(side_effect=spawn)
        with patch.object(guard.os, 'waitid') as observed, patch.object(guard.os, 'waitpid') as reaped, \
             patch.object(guard.os, 'pread') as read:
            with self.assertRaises(guard.Refused): obj.run_child(['/fixed'], 0, 'fixed')
            observed.assert_not_called(); reaped.assert_not_called(); read.assert_not_called()
        self.assertEqual(self.core.retained, [child])
        self.assertIsNone(child.returncode)
        with patch.object(guard.os, 'waitid') as observed:
            with self.assertRaises(guard.Refused): guard.await_allowed(child, 5, (0,))
            observed.assert_not_called()
        self.assertIs(self.core.retained[-1], child)

    def test_late_namespace_observation_never_rewinds_request_or_starts_child(self):
        obj = guard.Guard.__new__(guard.Guard)
        obj.source_recheck = Mock(); obj.artifacts_pin = Mock()
        obj.artifacts = {name:SimpleNamespace(sha='a'*64,recheck=Mock()) for name in ('helper','omavless')}
        obj.namespace_pins = {'net':(17,1,2)}
        obj.run_child = Mock()
        def inspect(*_, **__):
            guard.DEADLINE = 0
            return SimpleNamespace(st_dev=1,st_ino=2)
        with patch.object(guard.os, 'fstat', side_effect=inspect),patch.object(guard.os, 'stat', side_effect=inspect), \
             patch.object(guard.os, 'lseek') as rewound:
            with self.assertRaises(guard.Refused): obj.native('launch','fixed',request=12)
            rewound.assert_not_called(); obj.run_child.assert_not_called()

    def copy_controls(self, module, operation, mutations):
        for late in ('pread', *mutations):
            self.core.UNCERTAIN = False
            with patch.object(module, 'DEADLINE', float('inf')):
                mocks = {}
                patches = []
                for name in ('pread', *mutations):
                    def call(*_, name=name):
                        if name == late: module.DEADLINE = 0
                        return b'x' if name == 'pread' else 1 if name == 'write' else None
                    item = patch.object(module.os, name, side_effect=call)
                    mocks[name] = item.start(); patches.append(item)
                try:
                    with self.assertRaises((guard.Refused, RuntimeError)): operation()
                    reached = ('pread', *mutations).index(late)
                    for index, name in enumerate(('pread', *mutations)):
                        self.assertEqual(mocks[name].call_count, int(index <= reached), (late, name))
                finally:
                    for item in reversed(patches): item.stop()

    def test_loader_delayed_read_write_chmod_sync_forbid_next_effect(self):
        self.copy_controls(loader, lambda:loader.copy_file(10, 11, 1, 0o500),
                           ('write', 'fchmod', 'fsync'))

    def test_guard_delayed_read_write_chown_chmod_sync_forbid_next_effect(self):
        pin = SimpleNamespace(fd=10, before=SimpleNamespace(st_size=1))
        self.copy_controls(guard, lambda:guard.copy_elf(pin, 11),
                           ('write', 'fchown', 'fchmod', 'fsync'))

    def test_both_copiers_reject_bool_float_unknown_short_before_mode_changes(self):
        pin = SimpleNamespace(fd=10, before=SimpleNamespace(st_size=1))
        for module, operation in ((loader, lambda:loader.copy_file(10, 11, 1, 0o500)),
                                  (guard, lambda:guard.copy_elf(pin, 11))):
            for count in (True, 1.0, None, 0):
                self.core.UNCERTAIN = False
                with patch.object(module.os, 'pread', return_value=b'x'), \
                     patch.object(module.os, 'write', return_value=count) as written, \
                     patch.object(module.os, 'fchown') as owner, patch.object(module.os, 'fchmod') as mode, \
                     patch.object(module.os, 'fsync') as sync:
                    with self.assertRaises((guard.Refused, RuntimeError)): operation()
                    written.assert_called_once()
                    owner.assert_not_called(); mode.assert_not_called(); sync.assert_not_called()

    def test_evidence_late_parent_recheck_never_creates_next_file_or_directory(self):
        evidence = guard.DeadlineEvidence.__new__(guard.DeadlineEvidence)
        evidence.directory = SimpleNamespace(fd=12, recheck=Mock(side_effect=lambda:setattr(guard,'DEADLINE',0)))
        with patch.object(guard.os, 'open') as opened:
            with self.assertRaises(guard.Refused): evidence.create('fixed.json')
            opened.assert_not_called()
        self.core.UNCERTAIN = False
        guard.DEADLINE = float('inf')
        stage = SimpleNamespace(fd=12, path=Path('/fixed'),
                                recheck=Mock(side_effect=lambda:setattr(guard,'DEADLINE',0)))
        with patch.object(guard.os, 'mkdir') as mkdir:
            with self.assertRaises(guard.Refused): guard.DeadlineEvidence(stage, Mock())
            mkdir.assert_not_called()

    def test_evidence_late_full_write_or_sync_cannot_authorize_next_sync(self):
        for late in ('write', 'file-sync', 'directory-sync'):
            self.core.UNCERTAIN = False
            with patch.object(guard, 'DEADLINE', float('inf')):
                evidence = guard.DeadlineEvidence.__new__(guard.DeadlineEvidence)
                evidence.directory = SimpleNamespace(fd=12)
                evidence.create = Mock(return_value=11)
                def write(_, data):
                    if late == 'write': guard.DEADLINE = 0
                    return len(data)
                def sync(fd):
                    if fd == (11 if late == 'file-sync' else 12): guard.DEADLINE = 0
                with patch.object(guard.os, 'write', side_effect=write), patch.object(guard.os, 'fsync', side_effect=sync) as synced:
                    with self.assertRaises(guard.Refused): evidence.write('fixed.json', {})
                    self.assertEqual(synced.call_count, {'write':0, 'file-sync':1, 'directory-sync':2}[late])

    def test_terminal_full_typed_write_flush_deadline_and_permanent_seal(self):
        for module in (loader, guard):
            for fault in ('success', 'bool', 'float', 'unknown', 'short', 'late-write', 'late-flush'):
                self.core.UNCERTAIN = False
                stream = Mock()
                def write(raw):
                    if fault == 'late-write': module.DEADLINE = 0
                    return {'bool':True, 'float':float(len(raw)), 'unknown':None, 'short':0}.get(fault, len(raw))
                stream.write.side_effect = write
                if fault == 'late-flush': stream.flush.side_effect = lambda:setattr(module,'DEADLINE',0)
                with patch.object(module, 'DEADLINE', float('inf')), patch.object(module, 'TERMINAL', False):
                    def output():
                        if module is guard: guard.emit_terminal(b'x', stream, success=True)
                        else: loader.emit_terminal(b'x', stream)
                    if fault == 'success': output()
                    else:
                        with self.assertRaises((guard.Refused, RuntimeError)): output()
                    self.assertTrue(module.TERMINAL)
                    self.assertEqual(stream.flush.call_count, int(fault in ('success', 'late-flush')))
                    with self.assertRaises((guard.Refused, RuntimeError)): output()
                    stream.write.assert_called_once()


if __name__ == '__main__': unittest.main()
