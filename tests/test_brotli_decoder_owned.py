"""Terminal ownership controls; no real subprocess, signals, wait or guest."""
import os
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch
from tests.brotli_decoder_provenance import owned
from tests.real_resolved_binary import probe as base


class OwnedTests(unittest.TestCase):
    def test_first_nonzero_invalid_unknown_or_cancel_never_reaps_signals_or_requeries(self):
        for first in (1,-15,False,'invalid',OSError('unknown'),KeyboardInterrupt()):
            child = SimpleNamespace(pid=123,returncode=None)
            with patch.object(base,'UNSETTLED',[]), \
                 patch.object(base,'child_status',side_effect=[first,0]) as query, \
                 patch.object(owned.os,'waitpid') as reap, \
                 patch.object(base,'live_group') as group, \
                 patch.object(owned.os,'killpg') as killpg, patch.object(owned.os,'kill') as kill:
                with self.assertRaises(BaseException): owned.settle(base,child,5)
                with self.assertRaises(BaseException): owned.settle(base,child,5)
                self.assertEqual(query.call_count,1)
                reap.assert_not_called(); group.assert_not_called()
                kill.assert_not_called(); killpg.assert_not_called()
                self.assertIsNone(child.returncode)
                self.assertEqual(base.UNSETTLED,[child])

    def test_deadline_after_known_live_has_no_final_query_or_reap(self):
        child = SimpleNamespace(pid=123,returncode=None)
        with patch.object(base,'UNSETTLED',[]), \
             patch.object(owned.time,'monotonic',side_effect=[0,1,6]), \
             patch.object(owned.time,'sleep'), \
             patch.object(base,'child_status',side_effect=[None,0]) as query, \
             patch.object(owned.os,'waitpid') as reap:
            with self.assertRaises(base.Refused): owned.settle(base,child,5)
        self.assertEqual(query.call_count,1); reap.assert_not_called()
        self.assertIsNone(child.returncode)

    def test_only_known_zero_gets_one_exact_raw_reap(self):
        for result in ((123,0),(124,0),(123,256),OSError('unknown')):
            child = SimpleNamespace(pid=123,returncode=None)
            with patch.object(base,'UNSETTLED',[]), \
                 patch.object(base,'child_status',return_value=0) as query, \
                 patch.object(owned.os,'waitpid',side_effect=result if isinstance(result,BaseException) else None,
                              return_value=result) as reap:
                if result == (123,0):
                    owned.settle(base,child,5); self.assertEqual(child.returncode,0)
                else:
                    with self.assertRaises(BaseException): owned.settle(base,child,5)
                    with self.assertRaises(BaseException): owned.settle(base,child,5)
                    self.assertIsNone(child.returncode)
                query.assert_called_once_with(child)
                reap.assert_called_once_with(123,os.WNOHANG)

    def test_preset_status_is_not_a_fresh_observation(self):
        for code in (0,False,1):
            child = SimpleNamespace(pid=123,returncode=code)
            with patch.object(base,'UNSETTLED',[]), patch.object(base,'child_status') as query:
                with self.assertRaises(base.Refused): owned.settle(base,child,5)
                query.assert_not_called()

    def test_readelf_adapter_no_output_read_on_failure_and_fixed_scope(self):
        files = [Mock(),Mock()]
        for item in files:
            item.__enter__ = Mock(return_value=item); item.__exit__ = Mock(return_value=False)
        args = ['/proc/self/fd/10','--wide','--dynamic','--program-headers','/proc/self/fd/11']
        with patch.object(base,'UNSETTLED',[]), patch.object(base,'OwnedProcess',return_value=Mock()) as spawn, \
             patch.object(owned.tempfile,'TemporaryFile',side_effect=files), \
             patch.object(owned,'settle',side_effect=RuntimeError('unknown')):
            with self.assertRaises(RuntimeError):
                owned.command(base,args,pass_fds=(10,11),env={'PATH':'/usr/bin','LANG':'C','LC_ALL':'C'})
        spawn.assert_called_once()
        for item in files:
            item.seek.assert_not_called(); item.read.assert_not_called()
        with patch.object(base,'UNSETTLED',[]), patch.object(base,'OwnedProcess') as spawn:
            with self.assertRaises(base.Refused):
                owned.command(base,['/unapproved'],pass_fds=(10,11),env={})
            spawn.assert_not_called()


if __name__ == '__main__':
    unittest.main()
