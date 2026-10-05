"""Inert coordinator/private-frame controls: no child, namespace or socket."""
import ast
import copy
import importlib.util
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

SOURCE=Path(__file__).with_name('positive.py')
spec=importlib.util.spec_from_file_location('retained_positive',SOURCE)
p=importlib.util.module_from_spec(spec);spec.loader.exec_module(p)

class Session:pass
class Bridge:pass
class Artifacts:pass
class Images:
    def __init__(self,*args):self.verify=Mock()


def dns(kind='baseline'):
    active=kind!='baseline';address=[192,0,2,53] if kind=='unrelated' else [198,18,0,2]
    domain='unrelated.invalid' if kind=='unrelated' else '.'
    return {'servers':[[2,address]] if active else [],
            'extended':[[2,address,0,'']] if active else [],
            'domains':[[domain,True]] if active else [],'route':active,
            'llmnr':'no','mdns':'no','tls':'no','dnssec':'no','anchors':[]}


def frame(kind='ready'):
    received=kind!='ready';final=kind=='final'
    value={'notifications':p.NOTIFICATIONS[:1 if not received else 5 if final else 3],
           'stored':received and not final,'received_tun':received,
           'actual_fixed_policy':kind=='active','observation':dns('managed') if kind=='active'
                            else dns() if received else None,
           'baseline':dns() if received else None,'unrelated_baseline':dns('unrelated'),
           'reset_while_held':final,'owner_pinned':True,'unrelated_preserved':True,
           'effects':[], 'phase':'active' if kind=='active' else None,
           'tun_exists':kind=='active','notify_alive':True,'observer_finalized':final,'monitor_alive':not final}
    for index,method in enumerate(p.METHODS[:4 if final else 3 if kind=='active' else 0]):
        value['effects'].append({'method':method,'serial':index+1,'sender':':1.7','outcome':'settled_success'})
    return value


class Controls(unittest.TestCase):
    def fixture(self):
        owner=Session();owner.kind='inner';owner.isolated=True;owner.anchors={};owner.sealed=False
        owner.retained=[];owner.children=[];owner.available=Mock();clock=[0.0]
        def within(deadline):
            if clock[0]>=deadline:raise RuntimeError('private deadline')
        owner.within=Mock(side_effect=within);owner.local_deadline=Mock(side_effect=lambda seconds:clock[0]+seconds)
        owner.anchor=Mock();owner.live=Mock()
        copies=Bridge();copies.state='ready';artifacts=Artifacts();artifacts.sealed=False
        base=SimpleNamespace(ENV={},limits=Mock(),active=Mock(),clean=Mock())
        with patch.object(p.os,'getpid',return_value=1),patch.object(p.os,'geteuid',return_value=0), \
             patch.object(p.os,'getegid',return_value=0):
            value=p.Case(owner,SimpleNamespace(Session=Session),base,copies,SimpleNamespace(Bridge=Bridge),
                         artifacts,SimpleNamespace(Sources=Artifacts),SimpleNamespace(Images=Images),
                         SimpleNamespace(),SimpleNamespace(),SimpleNamespace(),SimpleNamespace())
        return value,clock

    def test_constructor_requires_concrete_inner_isolated_session_before_image(self):
        owner=SimpleNamespace(sealed=False)
        images=SimpleNamespace(Images=Mock())
        with self.assertRaises(p.Refused):
            p.Case(owner,SimpleNamespace(Session=Session),SimpleNamespace(),SimpleNamespace(),
                   SimpleNamespace(Bridge=Bridge),SimpleNamespace(),SimpleNamespace(Sources=Artifacts),
                   images,SimpleNamespace(),SimpleNamespace(),SimpleNamespace(),SimpleNamespace())
        self.assertTrue(owner.sealed);images.Images.assert_not_called()

    def test_five_second_late_spawn_retains_owner_child_but_never_anchors(self):
        value,clock=self.fixture();child=object()
        def spawned(*args,**kwargs):
            value.owner.children.append(child);clock[0]=5.0;return child
        value.owner.spawn=Mock(side_effect=spawned)
        with patch.object(p.os,'open',return_value=71) as opened:
            with self.assertRaises(p.Refused):value.spawn('core',['fixed'])
            clock[0]=0.0
            with self.assertRaises(p.Refused):value.spawn('core',['fixed'])
            self.assertEqual(opened.call_count,1)
        self.assertEqual(value.owner.children,[child]);self.assertEqual(value.children,{})
        value.owner.anchor.assert_not_called();self.assertTrue(value.sealed and value.owner.sealed)
        self.assertEqual(value.owner.local_deadline.call_count,1)

    def test_known_spawn_has_post_same_deadline_then_separate_anchor_stage(self):
        value,clock=self.fixture();child=object();events=[]
        value.owner.spawn=Mock(side_effect=lambda *args,**kwargs:events.append('spawn') or child)
        original=value.owner.within.side_effect
        value.owner.within.side_effect=lambda deadline:events.append('within') or original(deadline)
        value.owner.anchor.side_effect=lambda *args:events.append('anchor')
        with patch.object(p.os,'open',return_value=71):self.assertIs(value.spawn('core',['fixed']),child)
        position=events.index('spawn');self.assertEqual(events[position+1],'within')
        self.assertGreater(events.index('anchor'),position+1)
        self.assertEqual(value.owner.local_deadline.call_count,2)

    def test_late_open_retains_new_fd_no_followup_after_clock_recovers(self):
        value,clock=self.fixture()
        def opened(*args):clock[0]=5.0;return 71
        with patch.object(p.os,'open',side_effect=opened) as op:
            with self.assertRaises(p.Refused):value.opened(5.0,'fixed',0)
            clock[0]=0.0
            with self.assertRaises(p.Refused):value.opened(5.0,'fixed',0)
            self.assertEqual(op.call_count,1)
        self.assertIn(71,value.owner.retained)

    def test_exact_ready_active_and_frozen_final_frames_accept_privately(self):
        for kind in ('ready','active','final'):
            value,clock=self.fixture();actual=frame(kind)
            self.assertIs(value.observer(actual,final=kind=='final'),actual)
            self.assertFalse(value.sealed)

    def test_unknown_phase_nested_type_or_policy_is_terminal_before_sleep_requery(self):
        variants=[]
        for phase in ('quarantined','unknown',False,[],{}):
            actual=frame('active');actual['phase']=phase;variants.append(actual)
        for key in ('observation','baseline','unrelated_baseline'):
            for bad in (None,{},[],dns()|{'unknown':True},dns()|{'route':0},
                        dns()|{'extended':[[2,[198,18,0,2],False,'']]},
                        dns()|{'servers':[[True,[198,18,0,2]]]},
                        dns()|{'servers':[[2,[198,18,False,2]]]},
                        dns()|{'domains':[['.',1]]},dns()|{'llmnr':'unknown'},
                        dns()|{'anchors':['private.invalid']}):
                actual=frame('active');actual[key]=bad;variants.append(actual)
        for actual in variants:
            value,clock=self.fixture();helper=SimpleNamespace(snapshot=Mock(return_value=actual))
            with patch.object(p.time,'sleep') as sleep:
                with self.assertRaises(p.Refused):value.wait_snapshot(helper,'active')
                with self.assertRaises(p.Refused):value.wait_snapshot(helper,'active')
                helper.snapshot.assert_called_once();sleep.assert_not_called()
            self.assertTrue(value.sealed and value.owner.sealed)

    def test_known_pending_can_sleep_once_then_exact_active_receipt(self):
        value,clock=self.fixture();pending=frame('active');pending['phase']='applying'
        pending['effects'][-1]['outcome']=None;pending['observation']['route']=False
        pending['actual_fixed_policy']=False
        helper=SimpleNamespace(snapshot=Mock(side_effect=[pending,frame('active')]))
        with patch.object(p.time,'sleep') as sleep:value.wait_snapshot(helper,'active')
        sleep.assert_called_once_with(0.1);self.assertEqual(helper.snapshot.call_count,2)
        value.base.active.assert_called_once();self.assertFalse(value.sealed)

    def test_phase_from_wrong_stage_is_not_known_pending(self):
        for kind,phase in (('broker_ready','applying'),('active','releasing'),('active','cleanup_verified'),
                           ('released','applying')):
            value,clock=self.fixture();actual=frame('active');actual['phase']=phase
            helper=SimpleNamespace(snapshot=Mock(return_value=actual))
            with patch.object(p.time,'sleep') as sleep:
                with self.assertRaises(p.Refused):value.wait_snapshot(helper,kind)
                sleep.assert_not_called();helper.snapshot.assert_called_once()

    def test_denial_unsettled_prior_effect_alias_serial_or_lost_monitor_no_retry(self):
        for variant in ('denial','earlier_pending','bool_serial','duplicate_serial','dead_monitor','unfinalized','extra'):
            value,clock=self.fixture();actual=frame('active')
            if variant=='denial':actual['effects'][-1]['outcome']='org.freedesktop.DBus.Error.AccessDenied'
            if variant=='earlier_pending':actual['effects'][0]['outcome']=None
            if variant=='bool_serial':actual['effects'][0]['serial']=True
            if variant=='duplicate_serial':actual['effects'][-1]['serial']=1
            if variant=='dead_monitor':actual['monitor_alive']=False
            if variant=='unfinalized':actual['observer_finalized']=True
            if variant=='extra':actual['effects'][0]['extra']=True
            helper=SimpleNamespace(snapshot=Mock(return_value=actual))
            with patch.object(p.time,'sleep') as sleep:
                with self.assertRaises(p.Refused):value.wait_snapshot(helper,'active')
                sleep.assert_not_called();helper.snapshot.assert_called_once()

    def test_first_case_uncertainty_does_not_write_spawn_shutdown_or_close(self):
        value,clock=self.fixture();value.images.verify.side_effect=RuntimeError('private')
        value.write=Mock();value.spawn=Mock();value.owner.shutdown=Mock()
        with self.assertRaisesRegex(p.Refused,'^retained_positive_case_refused$'):value.run()
        value.write.assert_not_called();value.spawn.assert_not_called();value.owner.shutdown.assert_not_called()
        self.assertTrue(value.sealed and value.owner.sealed)

    def test_source_has_no_generic_retry_cleanup_direct_signal_or_reap(self):
        tree=ast.parse(SOURCE.read_text())
        for node in ast.walk(tree):
            if isinstance(node,ast.Try):self.assertEqual(node.finalbody,[])
            if isinstance(node,ast.Call):
                name=node.func.id if isinstance(node.func,ast.Name) else node.func.attr if isinstance(node.func,ast.Attribute) else ''
                self.assertNotIn(name,{'kill','waitpid','waitid','poll','wait','unlink','rmdir','exec','eval','system'})
        self.assertNotIn("'quarantined'",SOURCE.read_text())

    def test_final_snapshot_one_shared_deadline_late_result_prevents_next_stage(self):
        for variant in ('known','snapshot','observer','clean'):
            value,clock=self.fixture();actual=frame('final')
            def snap(*args,**kwargs):
                if variant=='snapshot':clock[0]=5.0
                return actual
            helper=SimpleNamespace(snapshot=Mock(side_effect=snap))
            original=value.observer
            def observed(*args,**kwargs):
                result=original(*args,**kwargs)
                if variant=='observer':clock[0]=5.0
                return result
            value.observer=Mock(side_effect=observed)
            value.base.clean.side_effect=lambda *args:clock.__setitem__(0,5.0) if variant=='clean' else None
            if variant=='known':self.assertIs(value.final_snapshot(helper),actual)
            else:
                with self.assertRaises(p.Refused):value.final_snapshot(helper)
                clock[0]=0.0
                with self.assertRaises(p.Refused):value.final_snapshot(helper)
            helper.snapshot.assert_called_once_with(5.0,final=True)
            if variant=='snapshot':value.observer.assert_not_called()
            if variant in ('snapshot','observer'):value.base.clean.assert_not_called()
            else:value.base.clean.assert_called_once_with(actual,'success')
            self.assertEqual(value.owner.local_deadline.call_count,1)


if __name__=='__main__':unittest.main()
