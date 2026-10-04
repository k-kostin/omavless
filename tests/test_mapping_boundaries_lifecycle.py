"""Inert owned-protocol controls. No subprocess, signal, mount or guest access."""
import importlib.util
import os
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).parent / 'decoder_mapping_boundaries'
spec = importlib.util.spec_from_file_location('decoder_lifecycle', ROOT / 'lifecycle.py')
life = importlib.util.module_from_spec(spec)
spec.loader.exec_module(life)


class Lifecycle(unittest.TestCase):
    def owned(self):
        session = life.Session()
        child = SimpleNamespace(pid=123, returncode=None)
        session.children.append(child)
        return session, child

    def seen(self, status=0, code=os.CLD_EXITED, pid=123):
        return SimpleNamespace(si_pid=pid, si_code=code, si_status=status)

    def sealed_no_io(self, session, child):
        with patch.object(life.os, 'waitid') as wait, patch.object(life.os, 'waitpid') as reap, \
             patch.object(life.os, 'kill') as kill, patch.object(life.os, 'stat') as stat:
            for action in (lambda: session.observation(child), lambda: session.live(child),
                           lambda: session.settle_zero(child, 5), lambda: session.perform(lambda: None),
                           lambda: session.shutdown('resolved', None, None)):
                with self.assertRaises(life.Refused):
                    action()
            for op in (wait, reap, kill, stat):
                op.assert_not_called()

    def test_known_zero_reaps_exactly_once(self):
        session, child = self.owned()
        with patch.object(life.os, 'waitid', return_value=self.seen()) as wait, \
             patch.object(life.os, 'waitpid', return_value=(123, 0)) as reap, \
             patch.object(life.os, 'kill') as kill:
            self.assertEqual(session.settle_zero(child, 5), 0)
            wait.assert_called_once()
            reap.assert_called_once_with(123, os.WNOHANG)
            kill.assert_not_called()
            self.assertEqual(child.returncode, 0)

    def test_nonzero_signal_boolean_wrong_pid_unknown_never_reap(self):
        for seen in (self.seen(7), self.seen(15, os.CLD_KILLED), self.seen(11, os.CLD_DUMPED),
                     self.seen(False), self.seen(pid=124), self.seen(code=False)):
            session, child = self.owned()
            with patch.object(life.os, 'waitid', return_value=seen) as wait, \
                 patch.object(life.os, 'waitpid') as reap, patch.object(life.os, 'kill') as kill:
                with self.assertRaises(life.Refused):
                    session.settle_zero(child, 5)
                wait.assert_called_once()
                reap.assert_not_called()
                kill.assert_not_called()
            self.sealed_no_io(session, child)

    def test_first_unknown_or_cancellation_cannot_retry(self):
        for error in (OSError('unknown'), ChildProcessError(), KeyboardInterrupt()):
            session, child = self.owned()
            with patch.object(life.os, 'waitid', side_effect=[error, self.seen()]) as wait, \
                 patch.object(life.os, 'waitpid') as reap:
                with self.assertRaises(type(error)):
                    session.settle_zero(child, 5)
                wait.assert_called_once()
                reap.assert_not_called()
            self.sealed_no_io(session, child)

    def test_deadline_stops_before_status_and_later_clock_cannot_recover(self):
        session, child = self.owned()
        with patch.object(life.time, 'monotonic', side_effect=[0, 6]), \
             patch.object(life.os, 'waitid') as wait, patch.object(life.os, 'waitpid') as reap:
            with self.assertRaises(life.Refused):
                session.settle_zero(child, 5)
            wait.assert_not_called()
            reap.assert_not_called()
        self.sealed_no_io(session, child)

    def test_exact_reap_unknown_never_retries_or_fabricates_zero(self):
        for result in ((0, 0), (124, 0), (123, 7 << 8), (123, 15), (123, False), ChildProcessError()):
            session, child = self.owned()
            kwargs = {'side_effect': result} if isinstance(result, Exception) else {'return_value': result}
            with patch.object(life.os, 'waitid', return_value=self.seen()), \
                 patch.object(life.os, 'waitpid', **kwargs) as reap:
                with self.assertRaises((life.Refused, ChildProcessError)):
                    session.settle_zero(child, 5)
                reap.assert_called_once()
                self.assertIsNone(child.returncode)
            self.sealed_no_io(session, child)

    def test_preset_status_never_queries(self):
        for status in (0, False, 7):
            session, child = self.owned()
            child.returncode = status
            with patch.object(life.os, 'waitid') as wait, self.assertRaises(life.Refused):
                session.settle_zero(child, 5)
            wait.assert_not_called()

    def daemon(self):
        session, child = self.owned()
        session.anchors['resolved'] = {'child': child, 'state': 'mapped', 'starttime': 99,
            'namespaces': {'pid': (5, 1234), 'net': (5, 1235)}, 'maps': [{'public': True}]}
        return session, child

    def test_shutdown_only_positive_proofs_then_one_term_zero_and_no_postexit_proc(self):
        session, child = self.daemon()
        trace = []
        copies = SimpleNamespace(verify=lambda _: trace.append('copies'),
            inventory=lambda *a: trace.append('maps') or [{'public': True}])
        base = SimpleNamespace(RESOLVER_CAPS=0x2500, verify_child=lambda *a: trace.append('credentials'))
        with patch.object(session, 'live', side_effect=lambda _: trace.append('live')), \
             patch.object(life.os, 'kill', side_effect=lambda *a: trace.append('term')) as kill, \
             patch.object(life.os, 'waitid', side_effect=lambda *a: trace.append('wait') or self.seen()), \
             patch.object(life.os, 'waitpid', side_effect=lambda *a: trace.append('reap') or (123, 0)), \
             patch.object(life.os, 'stat') as stat:
            result = session.shutdown('resolved', copies, base)
        self.assertEqual(trace, ['live', 'copies', 'maps', 'credentials', 'live', 'term', 'wait', 'reap'])
        kill.assert_called_once_with(123, life.signal.SIGTERM)
        stat.assert_not_called()
        self.assertEqual(result['exit_code'], 0)
        self.assertEqual(session.anchors['resolved']['state'], 'zero-reaped')

    def test_preterm_proof_failure_has_no_signal_or_wait(self):
        for where in ('live', 'copies', 'maps', 'credentials'):
            session, child = self.daemon()
            def step(name, result=None):
                if name == where:
                    raise OSError('unknown')
                return result
            copies = SimpleNamespace(verify=lambda _: step('copies'),
                inventory=lambda *a: step('maps', [{'public': True}]))
            base = SimpleNamespace(RESOLVER_CAPS=0x2500, verify_child=lambda *a: step('credentials'))
            with patch.object(session, 'live', side_effect=lambda _: step('live')), \
                 patch.object(life.os, 'kill') as kill, patch.object(life.os, 'waitid') as wait:
                with self.assertRaises(OSError):
                    session.shutdown('resolved', copies, base)
                kill.assert_not_called()
                wait.assert_not_called()
            self.sealed_no_io(session, child)

    def test_term_error_has_no_status_query_and_no_escalation(self):
        session, child = self.daemon()
        copies = SimpleNamespace(verify=lambda _: None, inventory=lambda *a: [{'public': True}])
        base = SimpleNamespace(RESOLVER_CAPS=0x2500, verify_child=lambda *a: None)
        with patch.object(session, 'live'), patch.object(life.os, 'kill', side_effect=ProcessLookupError()) as kill, \
             patch.object(life.os, 'waitid') as wait, patch.object(life.os, 'waitpid') as reap:
            with self.assertRaises(ProcessLookupError):
                session.shutdown('resolved', copies, base)
            kill.assert_called_once()
            wait.assert_not_called()
            reap.assert_not_called()
        self.sealed_no_io(session, child)

    def test_early_zero_daemon_exit_is_failure_without_reap(self):
        session, child = self.daemon()
        with patch.object(life.os, 'waitid', return_value=self.seen()), \
             patch.object(life.os, 'waitpid') as reap, patch.object(life.os, 'stat') as stat:
            with self.assertRaises(life.Refused):
                session.live(child)
            reap.assert_not_called()
            stat.assert_not_called()

    def test_destructor_and_internal_poll_never_query(self):
        obj = object.__new__(life.OwnedProcess)
        obj.returncode = None
        with patch.object(life.os, 'waitpid') as reap:
            obj.__del__()
            self.assertIsNone(obj._internal_poll())
            with self.assertRaises(life.Refused):
                obj.poll()
            with self.assertRaises(life.Refused):
                obj.wait()
            reap.assert_not_called()

    def test_namespace_or_starttime_drift_refuses_and_seals_before_next_wait(self):
        for drift in ('starttime', 'namespace'):
            session, child = self.daemon()
            row = session.anchors['resolved']
            row['fds'] = {'pid': 17, 'net': 18}
            row['proc_fd'] = 19
            row['proc_identity'] = (5, 333)
            with patch.object(life.os, 'waitid', return_value=None) as wait, \
                 patch.object(session, '_proc_identity', return_value=100 if drift == 'starttime' else 99), \
                 patch.object(life, 'ns_identity', return_value=(5, 999)), \
                 patch.object(life.os, 'stat', return_value=SimpleNamespace(st_dev=5, st_ino=1234)), \
                 patch.object(life.os, 'kill') as kill, patch.object(life.os, 'waitpid') as reap:
                with self.assertRaises(life.Refused):
                    session.live(child)
                wait.assert_called_once()
                kill.assert_not_called()
                reap.assert_not_called()
            self.sealed_no_io(session, child)

    def test_wrong_proc_topology_never_claims_child_identity(self):
        session, child = self.daemon()
        with patch.object(life.os, 'waitid', return_value=None), \
             patch.object(life.os, 'readlink', return_value='wrong'), \
             patch.object(life.os, 'stat') as stat, patch.object(life, 'bounded') as read:
            with self.assertRaises(life.Refused):
                session.live(child)
            stat.assert_not_called()
            read.assert_not_called()
        self.sealed_no_io(session, child)

    def test_bus_shutdown_requires_resolved_known_zero_before_any_observation(self):
        session, child = self.daemon()
        session.anchors['bus'] = dict(session.anchors['resolved'])
        with patch.object(life.os, 'waitid') as wait, patch.object(life.os, 'kill') as kill:
            with self.assertRaises(life.Refused):
                session.shutdown('bus', None, None)
            wait.assert_not_called()
            kill.assert_not_called()
        self.sealed_no_io(session, child)


if __name__ == '__main__':
    unittest.main()
