"""Inert retained child/role/zero-ledger controls, never subprocess execution."""
import importlib.util
import os
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

spec=importlib.util.spec_from_file_location('retained_owner',Path(__file__).with_name('lifecycle.py'))
l=importlib.util.module_from_spec(spec);spec.loader.exec_module(l)


class Controls(unittest.TestCase):
    def setUp(self):
        self.clock=patch.object(l.time,'monotonic',return_value=0.0)
        self.clock.start()

    def tearDown(self):
        self.clock.stop()

    def child(self,session,role,pid=17):
        child=SimpleNamespace(pid=pid,returncode=None)
        with patch.object(l,'OwnedProcess',return_value=child):
            self.assertIs(session.spawn(['/fixed/developer/fixture'],role=role),child)
        return child

    def seen(self,child):
        return SimpleNamespace(si_pid=child.pid,si_code=os.CLD_EXITED,si_status=0)

    def reap(self,session,child):
        with patch.object(l.os,'waitid',return_value=self.seen(child)), \
             patch.object(l.os,'waitpid',return_value=(child.pid,0)):
            self.assertEqual(session.settle_zero(child,6),0)

    def test_outer_one_exact_namespace_child_zero_raw_ledger(self):
        session=l.Session('outer');child=self.child(session,'namespace')
        self.reap(session,child);result=session.complete()
        self.assertEqual(result['roles'],['namespace'])
        self.assertEqual(result['owned_child_count'],1)
        self.assertTrue(result['all_owned_direct_children_exact_zero_reaped'])
        self.assertFalse(result['global_shared_argv_or_uid_absence_claimed'])
        self.assertFalse(result['production_effect_authority'])
        self.assertNotIn('pid',result)

    def test_all_five_roles_and_utilities_require_independent_exact_raw_zero(self):
        session=l.Session('inner')
        for index,role in enumerate(('utility','utility','bus','resolved','core','broker','host'),17):
            child=self.child(session,role,index);self.reap(session,child)
            if role!='utility':session.anchors[role]={'child':child,'state':'zero-reaped'}
        result=session.complete()
        self.assertEqual(result['roles'],['broker','bus','core','host','resolved'])
        self.assertEqual(result['owned_child_count'],7);self.assertEqual(result['utility_count'],2)

    def test_bare_returncode_zero_cannot_fabricate_reap_ledger(self):
        session=l.Session('outer');child=self.child(session,'namespace');child.returncode=0
        with self.assertRaises(l.Refused):session.complete()
        self.assertTrue(session.sealed)

    def test_missing_role_anchor_or_nonzero_raw_ledger_refuses(self):
        for variant in ('missing_role','missing_anchor','live_anchor','raw_status','float_returncode'):
            session=l.Session('inner')
            roles=('bus','resolved','core','broker') if variant=='missing_role' else ('bus','resolved','core','broker','host')
            for index,role in enumerate(roles,17):
                child=self.child(session,role,index);self.reap(session,child)
                session.anchors[role]={'child':child,'state':'zero-reaped'}
            if variant=='missing_anchor':session.anchors.pop('host')
            if variant=='live_anchor':session.anchors['host']['state']='mapped'
            if variant=='raw_status':session.zero_reaped[id(child)]=(child.pid,256)
            if variant=='float_returncode':child.returncode=0.0
            with self.assertRaises(l.Refused):session.complete()
            self.assertTrue(session.sealed)

    def test_fixed_scope_duplicate_role_and_child_cap_refuse_before_spawn(self):
        for kind,role in (('outer','core'),('inner','namespace'),('inner','arbitrary')):
            session=l.Session(kind)
            with patch.object(l,'OwnedProcess') as spawn:
                with self.assertRaises(l.Refused):session.spawn(['/ignored'],role=role)
                spawn.assert_not_called()
        session=l.Session('inner');self.child(session,'core')
        with patch.object(l,'OwnedProcess') as spawn:
            with self.assertRaises(l.Refused):session.spawn(['/ignored'],role='core')
            spawn.assert_not_called()
        session=l.Session('inner');session.children=[SimpleNamespace(pid=i,returncode=None) for i in range(1,97)]
        with patch.object(l,'OwnedProcess') as spawn:
            with self.assertRaises(l.Refused):session.spawn(['/ignored'],role='utility')
            spawn.assert_not_called()

    def test_nonzero_unknown_float_bool_waitid_permanently_blocks_reap_and_retry(self):
        for field,value in (('si_pid',True),('si_pid',17.0),('si_pid',18),
                            ('si_code',True),('si_code',float(os.CLD_EXITED)),('si_code',os.CLD_KILLED),
                            ('si_status',True),('si_status',0.0),('si_status',1)):
            session=l.Session('outer');child=self.child(session,'namespace');seen=self.seen(child)
            setattr(seen,field,value)
            with patch.object(l.os,'waitid',return_value=seen) as observe,patch.object(l.os,'waitpid') as reap:
                with self.assertRaises(l.Refused):session.settle_zero(child,6)
                reap.assert_not_called();self.assertTrue(session.sealed)
                with self.assertRaises(l.Refused):session.observation(child)
                self.assertEqual(observe.call_count,1)
            self.assertIsNone(child.returncode);self.assertFalse(session.zero_reaped)

    def test_exact_final_waitpid_shape_and_status_unknown_do_not_reobserve(self):
        for result in ((True,0),(17.0,0),(17,False),(17,0.0),(0,0),(17,256),(17,9)):
            session=l.Session('outer');child=self.child(session,'namespace')
            with patch.object(l.os,'waitid',return_value=self.seen(child)) as observe, \
                 patch.object(l.os,'waitpid',return_value=result) as reap:
                with self.assertRaises(l.Refused):session.settle_zero(child,6)
                with self.assertRaises(l.Refused):session.settle_zero(child,6)
                self.assertEqual(observe.call_count,1);self.assertEqual(reap.call_count,1)
            self.assertFalse(session.zero_reaped);self.assertIsNone(child.returncode)

    def test_expiry_after_waitid_refuses_before_reap_and_clock_recovery(self):
        session=l.Session('outer');child=self.child(session,'namespace');clock=[0.0]
        def observed(*args):clock[0]=91.0;return self.seen(child)
        with patch.object(l.time,'monotonic',side_effect=lambda:clock[0]), \
             patch.object(l.os,'waitid',side_effect=observed) as observe,patch.object(l.os,'waitpid') as reap:
            with self.assertRaises(l.Refused):session.settle_zero(child,6)
            reap.assert_not_called();clock[0]=0.0
            with self.assertRaises(l.Refused):session.observation(child)
            self.assertEqual(observe.call_count,1)

    def test_local_six_second_expiry_after_waitid_refuses_before_reap(self):
        session=l.Session('outer');child=self.child(session,'namespace');clock=[0.0]
        def observed(*args):clock[0]=6.0;return self.seen(child)
        with patch.object(l.time,'monotonic',side_effect=lambda:clock[0]), \
             patch.object(l.os,'waitid',side_effect=observed),patch.object(l.os,'waitpid') as reap:
            with self.assertRaises(l.Refused):session.settle_zero(child,6)
            reap.assert_not_called()
        self.assertTrue(session.sealed)

    def test_initial_nonfinite_clock_keeps_sealed_before_any_spawn(self):
        for clock in (float('nan'),float('inf'),True,1,1e308):
            obj=l.Session.__new__(l.Session)
            with patch.object(l.time,'monotonic',return_value=clock),patch.object(l,'OwnedProcess') as spawn:
                with self.assertRaises(l.Refused):obj.__init__('inner')
                spawn.assert_not_called()
            self.assertTrue(obj.sealed)

    def test_live_role_mismatch_anchor_refuses_before_process_query(self):
        session=l.Session('inner');child=self.child(session,'broker')
        with patch.object(l.os,'waitid') as query:
            with self.assertRaises(l.Refused):session.anchor('core',child)
            query.assert_not_called()
        self.assertTrue(session.sealed)

    def test_shutdown_role_order_and_image_failure_prevent_any_signal(self):
        for variant in ('order','image'):
            session=l.Session('inner');child=self.child(session,'broker')
            row={'child':child,'state':'mapped','maps':['synthetic'], 'starttime':1,'namespaces':{'pid':(1,2),'net':(1,2)}}
            session.anchors['broker']=row
            session.anchors['core']={'state':'mapped' if variant=='order' else 'zero-reaped'}
            session.live=Mock()
            images=SimpleNamespace(verify=Mock(side_effect=RuntimeError() if variant=='image' else None),inventory=Mock(return_value=['synthetic']))
            with patch.object(l.os,'kill') as signal:
                with self.assertRaises((l.Refused,RuntimeError)):session.shutdown('broker',images,Mock())
                signal.assert_not_called()
            self.assertTrue(session.sealed)


if __name__=='__main__':unittest.main()
