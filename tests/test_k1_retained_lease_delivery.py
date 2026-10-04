"""Inert fixed delivery controls; no guest, subprocess or manager calls."""
import ast
import hashlib
import json
import os
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch
from tests import test_k1_response_diagnostic_fixture as baseline

guard=baseline.load('retained_lease_guard')
query=baseline.load('retained_lease_query')
stage=baseline.load('retained_lease_stage')


class DeliveryControls(unittest.TestCase):
    def tearDown(self):
        guard.DEADLINE=stage.DEADLINE=query.SCOPE_DEADLINE=float('inf')
        guard.TERMINAL=False
        query.UNCERTAIN=False
        query.RETAINED.clear()

    def test_new_pins_are_acyclic_and_native_zero_until_separate_freeze(self):
        paths={'guard.py':baseline.SUPPORT/'retained_lease_guard.py',
            'query-guard.py':baseline.SUPPORT/'retained_lease_query.py',
            'fixture.service':baseline.SUPPORT.parent/'fixtures'/guard.UNIT}
        for name,path in paths.items():
            self.assertEqual(stage.MEMBERS[name][0],hashlib.sha256(path.read_bytes()).hexdigest())
        self.assertEqual(guard.QUERY_SHA,stage.MEMBERS['query-guard.py'][0])
        self.assertEqual(guard.PROBE_SHA,stage.MEMBERS['probe'][0])
        self.assertEqual(guard.PROBE_SHA,'0'*64)
        self.assertEqual(guard.NATIVE_SOURCE,'0'*40)
        self.assertNotEqual(guard.STAGE,baseline.guard.STAGE)
        self.assertEqual(guard.STAGE,stage.DESTINATION)
        self.assertEqual(guard.STAGE,query.STAGE)
        self.assertIn('retained_lease::manager_retained_lease',paths['fixture.service'].read_text())
        self.assertEqual(hashlib.sha256((baseline.SUPPORT/'response_diagnostic_guard.py').read_bytes()).hexdigest(),
            'f2f126446ceb7f3611b2e67c7f51ffd91c0c7b35365a03fb2567ee8064021bd2')

    def test_both_permission_identities_are_disjoint(self):
        fresh=guard.expected_permissions()
        guard.validate_permissions(fresh)
        with self.assertRaises(RuntimeError):guard.validate_permissions(baseline.guard.expected_permissions())
        with self.assertRaises(RuntimeError):baseline.guard.validate_permissions(fresh)

    def test_same_outer_lifecycle_stops_after_every_injected_failure(self):
        helper=baseline.Flow()
        with patch.object(baseline,'guard',guard):
            obj,trace=helper.flow()
            self.assertTrue(obj.helper_known_zero and obj.lifecycle_validated)
            self.assertEqual(trace[-1],'result.json')
            for failure in ['vm','before','publish','helper','evidence',*guard.PHASES[5:9],'after','preserve','result.json']:
                obj,trace=helper.flow(failure,True)
                self.assertTrue(obj.sealed)
                self.assertEqual(trace[-1][0],'refusal.json')

    def test_typed_three_effect_proofs_refuse_legacy_and_pending_flag_changes(self):
        execution=dict(invocation=[1]*16,pid=7,start=3,exit=5,command_start=2,command_exit=4)
        value=dict(schema=1,unit=guard.UNIT,unique_owner=':1.77',start_job=dict(id=1,path='/org/freedesktop/systemd1/job/1'),
            stop_job=dict(id=2,path='/org/freedesktop/systemd1/job/2'),execution=execution,effects=3,closed_retired=True,
            absent=True,stopped=True,unref_acknowledged=True,synthetic_epoch=True,production_admission=False,**guard.LEASE_FLAGS)
        native=dict(schema=1,execution=execution,effects=3,closed_retired=True,absent=True,synthetic_epoch=True,**guard.LEASE_FLAGS)
        stopped=dict(schema=1,execution=execution,inactive_dead=True,zero_pids=True,no_job=True,empty_cgroup=True)
        guard.validate_lifecycle(value,native,stopped,':1.77')
        for key,bad in [('effects',2),('effects',True),('pending_retained',False),('generation_refused',False),('unit',baseline.guard.UNIT)]:
            with self.assertRaises(RuntimeError):guard.validate_lifecycle({**value,key:bad},native,stopped,':1.77')
        for key in guard.LEASE_FLAGS:
            with self.assertRaises(RuntimeError):guard.validate_lifecycle(value,{**native,key:False},stopped,':1.77')

    def test_query_late_spawn_retains_exact_original_before_any_wait(self):
        child=SimpleNamespace(pid=17,returncode=None)
        def spawned(*a,**k):query.SCOPE_DEADLINE=0;return child
        with patch.object(query,'OwnedProcess',side_effect=spawned),patch.object(query.os,'waitid') as waited:
            with self.assertRaises(query.Refused):query.spawn(['inert'])
            waited.assert_not_called()
        self.assertTrue(query.UNCERTAIN)
        self.assertEqual(query.RETAINED,[child])

    def test_query_late_zero_never_reaps_and_late_reap_never_claims_zero(self):
        for cut in ('observe','reap'):
            query.UNCERTAIN=False;query.SCOPE_DEADLINE=float('inf');query.RETAINED.clear()
            child=SimpleNamespace(pid=17,returncode=None)
            def observed(*a):
                if cut=='observe':query.SCOPE_DEADLINE=0
                return SimpleNamespace(si_pid=17,si_code=os.CLD_EXITED,si_status=0)
            def reaped(*a):query.SCOPE_DEADLINE=0;return (17,0)
            with patch.object(query.os,'waitid',side_effect=observed),patch.object(query.os,'waitpid',side_effect=reaped) as reap:
                with self.assertRaises(query.Refused):query.await_child(child,12)
                self.assertEqual(reap.call_count,int(cut=='reap'))
            self.assertIsNone(child.returncode)
            self.assertEqual(query.RETAINED,[child])

    def test_query_nonzero_alias_unknown_and_preset_status_never_reap(self):
        for status in (True,False,1,None):
            query.UNCERTAIN=False;query.RETAINED.clear()
            child=SimpleNamespace(pid=17,returncode=None)
            with patch.object(query.os,'waitid',return_value=SimpleNamespace(si_pid=17,si_code=os.CLD_EXITED,si_status=status)),patch.object(query.os,'waitpid') as reap:
                with self.assertRaises(query.Refused):query.await_child(child,12)
                reap.assert_not_called()
            self.assertEqual(query.RETAINED,[child])

    def test_initial_await_deadline_retains_child_without_status_query(self):
        child=SimpleNamespace(pid=17,returncode=None)
        query.SCOPE_DEADLINE=0
        with patch.object(query.os,'waitid') as observe,patch.object(query.os,'waitpid') as reap:
            with self.assertRaises(query.Refused):query.await_child(child,12)
            observe.assert_not_called();reap.assert_not_called()
        self.assertEqual(query.RETAINED,[child])

    def test_actual_outer_terminal_late_write_does_not_emit_refusal(self):
        source=(baseline.SUPPORT/'retained_lease_guard.py').read_text()
        tree=ast.parse(source)
        ns={'__name__':'inert'}
        exec(compile(ast.Module(body=tree.body[:-1],type_ignores=[]),'<inert>','exec'),ns)
        stdout,stderr=Mock(),Mock()
        def write(raw):ns['DEADLINE']=0;return len(raw)
        stdout.write.side_effect=write
        ns['main']=lambda:ns['emit_terminal'](b'fixed',stdout)
        ns['__name__']='__main__'
        with patch.object(guard.os,'umask'),patch.object(guard.sys,'stderr',SimpleNamespace(buffer=stderr)),self.assertRaises(SystemExit) as result:
            exec(compile(ast.Module(body=[tree.body[-1]],type_ignores=[]),'<outer>','exec'),ns)
        self.assertEqual(result.exception.code,2)
        stdout.write.assert_called_once();stdout.flush.assert_not_called()
        stderr.write.assert_not_called();stderr.flush.assert_not_called()

    def test_terminal_unknown_short_late_flush_no_second_attempt(self):
        for kind in ('late','short','float','bool','throw','flush'):
            guard.TERMINAL=False;guard.DEADLINE=float('inf')
            stream=Mock()
            def write(raw):
                if kind=='late':guard.DEADLINE=0
                if kind=='throw':raise OSError()
                return {'short':0,'float':float(len(raw)),'bool':True}.get(kind,len(raw))
            stream.write.side_effect=write
            if kind=='flush':stream.flush.side_effect=OSError()
            with self.assertRaises((RuntimeError,OSError)):guard.emit_terminal(b'fixed',stream)
            with self.assertRaises(RuntimeError):guard.emit_terminal(b'fixed',stream)
            stream.write.assert_called_once()
            self.assertEqual(stream.flush.call_count,int(kind=='flush'))

    def test_loader_late_open_retains_fd_and_late_write_stops_next_effect(self):
        def opened(*a,**k):stage.DEADLINE=0;return 17
        with patch.object(stage.os,'open',side_effect=opened),self.assertRaises(RuntimeError):stage.opened('inert',0)
        self.assertIn(17,stage.HELD);stage.HELD.clear();stage.DEADLINE=float('inf')
        def written(*a):stage.DEADLINE=0;return 3
        with self.assertRaises(RuntimeError):stage.checked(written,17,b'abc')

    def test_loader_no_native_or_old_scope_fallback(self):
        raw=(baseline.SUPPORT/'retained_lease_stage.py').read_text()
        self.assertNotIn('retained-private-lifecycle',raw)
        self.assertNotIn('subprocess',raw)
        self.assertIn('self.sealed=True',raw)
        tree=ast.parse(raw)
        for node in ast.walk(tree):
            if isinstance(node,ast.Call):
                name=getattr(node.func,'attr',getattr(node.func,'id',''))
                self.assertNotIn(name,('unlink','rmdir','kill','waitid','waitpid'))


if __name__=='__main__':unittest.main()
