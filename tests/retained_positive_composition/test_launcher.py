"""Executed mocked entry controls: NEVER run an actual guest/native recipe."""
from contextlib import ExitStack
import ast
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock,patch

HERE=Path(__file__).parent
def module(name):
    spec=importlib.util.spec_from_file_location('test_'+name,HERE/(name+'.py'))
    value=importlib.util.module_from_spec(spec);spec.loader.exec_module(value);return value
l=module('launcher');v=module('validate_receipt');f=module('test_validate_receipt')


def frame(deadline=65.0):
    return {'original_ns':{name:name+':[900]' for name in l.NS},'absolute_deadline':deadline}


class Controls(unittest.TestCase):
    def setUp(self):
        self.clock=[0.0];self.timer=patch.object(l.time,'monotonic',side_effect=lambda:self.clock[0]);self.timer.start()
        l.HELD=[]

    def tearDown(self):self.timer.stop()

    def outer(self):
        owner=Mock();owner.complete.return_value={'schema':'retained-positive-zero-ledger-v1','scope':'outer',
            'roles':['namespace'],'owned_child_count':1,'utility_count':0,
            'all_owned_direct_children_exact_zero_reaped':True,
            'global_shared_argv_or_uid_absence_claimed':False,'production_effect_authority':False}
        owner.perform.side_effect=lambda operation,*args:operation(*args)
        owner.deadline=90.0;owner.local_deadline.side_effect=lambda seconds:float(seconds)
        def within(deadline):
            if self.clock[0]>=deadline:raise RuntimeError('fixed expired local fence')
        owner.within.side_effect=within
        base=Mock();base.namespace.side_effect=lambda name:name+':[900]'
        graph=SimpleNamespace(raw={'copy-manifest.json':f.RAW})
        modules={'artifacts.py':SimpleNamespace()}
        record={'schema':'retained-positive-inner-record-v1','namespace_boundary_checked':True,
            'case':f.fixture(),'parent_whole_known_zero':False,'production_effect_authority':False}
        return owner,base,(graph,modules,None,owner,base,v),record

    def test_parent_reads_result_only_after_exact_owned_completion_then_private_write(self):
        entry=l.Entry();owner,base,context,record=self.outer();writes=[];events=[]
        owner.settle_zero.side_effect=lambda *args:events.append('zero')
        old_complete=owner.complete.return_value
        owner.complete.side_effect=lambda:events.append('complete') or old_complete
        def result(*args):events.append('read');return json.dumps(record).encode()
        info=lambda fd:SimpleNamespace(st_mode=stat.S_IFREG|0o600,st_uid=1000,st_gid=1000,
            st_nlink=1,st_size=len(writes[-1]) if writes else 0)
        with patch.object(l.os,'getresuid',return_value=(1000,1000,1000)), \
             patch.object(l.os,'getresgid',return_value=(1000,1000,1000)), \
             patch.object(l,'modules',return_value=context),patch.object(l,'native_originals'), \
             patch.object(l,'directories'),patch.object(entry,'opened',side_effect=[10,11,12]), \
             patch.object(l.resource,'setrlimit'),patch.object(l,'result_bytes',side_effect=result), \
             patch.object(l.os,'write',side_effect=lambda fd,raw:writes.append(raw) or len(raw)), \
             patch.object(l.os,'fsync'),patch.object(l.os,'fstat',side_effect=info):
            l.parent(entry)
        self.assertEqual(events,['zero','complete','read'])
        argv=owner.spawn.call_args.args[0]
        self.assertEqual(argv[:3],['/usr/bin/unshare','--user','--map-root-user'])
        self.assertNotIn('--kill-child',argv)
        self.assertEqual(argv[-3:-1],[l.STAGE+'/launcher.py','--isolated-child'])
        self.assertEqual(owner.spawn.call_args.kwargs['env']['TMPDIR'],l.STAGE+'/scratch')
        self.assertEqual(json.loads(argv[-1])['absolute_deadline'],65.0)
        self.assertEqual(entry.deadline,65.0);self.assertEqual(owner.deadline,65.0)
        self.assertIs(owner.spawn.call_args.kwargs['close_fds'],True)
        self.assertEqual(writes[-1],b'RETAINED_POSITIVE_RECORDED_ONLY\n')
        whole=v.decode(writes[0]);self.assertFalse(whole['parent_whole_known_zero'])
        self.assertFalse(whole['canonical_baseline_proven']);self.assertTrue(entry.sealed)

    def test_unknown_owned_zero_or_completion_never_reads_result_or_writes(self):
        for stage in ('settle_zero','complete'):
            entry=l.Entry();owner,base,context,_=self.outer()
            getattr(owner,stage).side_effect=RuntimeError('private')
            with patch.object(l.os,'getresuid',return_value=(1000,1000,1000)), \
                 patch.object(l.os,'getresgid',return_value=(1000,1000,1000)), \
                 patch.object(l,'modules',return_value=context),patch.object(l,'native_originals'), \
                 patch.object(l,'directories'),patch.object(entry,'opened',side_effect=[10,11]), \
                 patch.object(l.resource,'setrlimit'),patch.object(l,'result_bytes') as read, \
                 patch.object(l.os,'write') as output:
                with self.assertRaises(RuntimeError):l.parent(entry)
                read.assert_not_called();output.assert_not_called()

    def test_late_owned_zero_return_prevents_completion_result_or_output(self):
        entry=l.Entry();owner,base,context,_=self.outer()
        owner.settle_zero.side_effect=lambda *args:self.clock.__setitem__(0,90.0)
        with patch.object(l.os,'getresuid',return_value=(1000,1000,1000)), \
             patch.object(l.os,'getresgid',return_value=(1000,1000,1000)), \
             patch.object(l,'modules',return_value=context),patch.object(l,'native_originals'), \
             patch.object(l,'directories'),patch.object(entry,'opened',side_effect=[10,11]), \
             patch.object(l.resource,'setrlimit'),patch.object(l,'result_bytes') as read, \
             patch.object(l.os,'write') as output:
            with self.assertRaises(l.Refused):l.parent(entry)
            owner.complete.assert_not_called();read.assert_not_called();output.assert_not_called()

    def test_module_loader_passes_same_absolute_cap_and_installs_only_retained_hooks(self):
        entry=l.Entry();entry.deadline=65.0;base=SimpleNamespace();owner=SimpleNamespace(deadline=100.0,
            available=Mock(),command=Mock(),live=Mock(),no_directory_fds=Mock())
        class Graph:
            def __init__(self,deadline):self.deadline=deadline;self.raw={}
            def load(self):return {'lifecycle.py':SimpleNamespace(Session=lambda kind:owner),'containment.py':base}
        with patch.object(l,'pinned_module',side_effect=[SimpleNamespace(Graph=Graph),v]) as pinned:
            graph,*_=l.modules(entry,'outer')
        self.assertEqual(graph.deadline,entry.deadline);self.assertEqual(owner.deadline,entry.deadline)
        self.assertIs(base.command,owner.command);self.assertIs(base.child_status,owner.live)
        self.assertIs(base.no_directory_fds,owner.no_directory_fds)
        self.assertEqual([call.args[1] for call in pinned.call_args_list],['graph.py','validate_receipt.py'])

    def test_initial_clock_unknown_precedes_every_open_spawn_and_output(self):
        for bad in (True,1,float('inf'),float('nan')):
            self.clock[0]=bad
            with patch.object(l.os,'getpid',return_value=123),patch.object(l,'parent') as parent, \
                 patch.object(l.os,'open') as opened,patch.object(l.os,'write') as output:
                self.assertEqual(l.main(l.RUN),1)
                parent.assert_not_called();opened.assert_not_called();output.assert_not_called()

    def test_outer_unknown_has_no_diagnostic_retry_or_failure_close(self):
        with patch.object(l.os,'getpid',return_value=123),patch.object(l,'parent',side_effect=RuntimeError('private')), \
             patch.object(l.os,'write') as output,patch.object(l.os,'close') as closed:
            self.assertEqual(l.main(l.RUN),1)
            output.assert_not_called();closed.assert_not_called()

    def test_inner_unknown_parks_without_output_signal_query_cleanup_or_reexec(self):
        class Parked(BaseException):pass
        with patch.object(l.os,'getpid',return_value=1) as pid, \
             patch.object(l,'child',side_effect=RuntimeError('private')), \
             patch.object(l.signal,'pause',side_effect=Parked) as pause, \
             patch.object(l.os,'write') as output,patch.object(l.os,'close') as close:
            with self.assertRaises(Parked):l.main(['--isolated-child','{}'])
            self.assertEqual(pid.call_count,1);pause.assert_called_once();output.assert_not_called();close.assert_not_called()

    def test_terminal_short_alias_throw_and_late_write_seal_before_any_second_output(self):
        for variant in ('short','float','bool','throw','late'):
            entry=l.Entry()
            def write(fd,raw):
                if variant=='throw':raise OSError('private')
                if variant=='late':self.clock[0]=90.0
                return {'short':0,'float':float(len(raw)),'bool':True}.get(variant,len(raw))
            with patch.object(l.os,'write',side_effect=write) as output:
                with self.assertRaises(l.Refused):entry.output(b'fixed')
                self.clock[0]=0.0
                with self.assertRaises(l.Refused):entry.output(b'fixed')
                self.assertEqual(output.call_count,1);self.assertTrue(entry.sealed)

    def test_fixed_public_source_original_fd_hash_before_definition_load(self):
        with tempfile.TemporaryDirectory() as directory:
            raw=(HERE/'graph.py').read_bytes();path=Path(directory)/'graph.py'
            path.write_bytes(raw);path.chmod(0o600)
            fd=os.open(path,os.O_RDONLY);entry=l.Entry();real_stat=os.fstat(fd)
            try:
                with patch.object(l.os,'open',return_value=fd) as opened, \
                     patch.object(l.os,'stat',return_value=real_stat):
                    reader=l.pinned_module(entry,'graph.py')
                self.assertEqual(reader.PINS['images.py'],'11d3a970654a707c5aa1b02ba2ff888b84dc498384796c67e413a02699e8bd0e')
                opened.assert_called_once_with(l.STAGE+'/graph.py',l.FLAGS)
                self.assertIn(fd,l.HELD)
            finally:os.close(fd)

    def test_native_four_originals_bounded_synthetic_only_and_positive_directory_close(self):
        with tempfile.TemporaryDirectory() as directory:
            os.chmod(directory,0o755);table={}
            for name in ('developer-manifest.json','mihomo','omavless-dns-broker','host-fixture'):
                raw=b'public synthetic nonexecuted '+name.encode();path=Path(directory)/name
                path.write_bytes(raw);path.chmod(0o555)
                table[name]=(len(raw),0o555,hashlib.sha256(raw).hexdigest())
            entry=l.Entry();before=set(l.HELD)
            with patch.object(l,'NATIVE',directory):
                records=l.native_originals(entry,SimpleNamespace(TABLE=table))
            self.assertEqual(set(records),set(table))
            self.assertEqual(len({fd for fd in set(l.HELD)-before if type(fd) is int}),4)
            for fd,_ in records.values():os.close(fd)

    def test_fixed_namespace_arguments_unknown_duplicates_and_wrong_context_refuse_pre_io(self):
        valid={name:name+':[900]' for name in l.NS};self.assertEqual(l.namespace_frame(valid),valid)
        for value in ({},dict(valid,unknown='x'),dict(valid,pid='private')):
            with self.assertRaises(l.Refused):l.namespace_frame(value)
        with patch.object(l.os,'getpid',return_value=123),patch.object(l,'child') as child:
            self.assertEqual(l.main(['--isolated-child','{"pid":1,"pid":2}']),1)
            child.assert_not_called()

    def child_context(self,changed=False,late=False):
        steps=[];owner=Mock();owner.retained=[];owner.perform.side_effect=lambda operation,*args:operation(*args)
        base=SimpleNamespace(isolate=lambda *args:steps.append('isolate'))
        graph=SimpleNamespace(raw={'copy-manifest.json':f.RAW},
            release_directories_positive=lambda:steps.append('release_source_dirs'))
        info=SimpleNamespace(st_dev=40,st_ino=90,st_mode=stat.S_IFREG|0o555,st_uid=0,st_gid=0,
            st_nlink=1,st_size=64,st_mtime_ns=1,st_ctime_ns=1)
        bound=SimpleNamespace(**vars(info))
        if changed:bound.st_ino+=1
        class Bridge:
            def __init__(self,*args):
                assert any(item is self for item in owner.retained)
                steps.append('bridge_retained_before_init');self.prepare=lambda original:steps.append('prepare_copies')
        class Artifacts:
            def __init__(self):
                assert any(item is self for item in owner.retained)
                steps.append('artifacts_retained_before_init');self.files={'mihomo':(50,bound,'fixed')}
        class Case:
            def __init__(self,*args):steps.append('case_constructed')
            def run(self):
                steps.append('case_run')
                if late:self_clock[0]=90.0
                return f.fixture()
        self_clock=self.clock
        loaded={'bridge.py':SimpleNamespace(Bridge=Bridge),'artifacts.py':SimpleNamespace(Sources=Artifacts),
                'positive.py':SimpleNamespace(Case=Case)}
        loaded.update({name:SimpleNamespace() for name in ('admission.py','images.py','controller.py',
            'helper.py','streams.py','bootstrap.py')})
        return (graph,loaded,SimpleNamespace(),owner,base,v),{'mihomo':(10,info)},steps

    def test_inner_mocked_positive_composes_retained_objects_before_one_private_record(self):
        context,originals,steps=self.child_context();writes=[]
        with patch.object(l.os,'getpid',return_value=1),patch.object(l.os,'getresuid',return_value=(0,0,0)), \
             patch.object(l.os,'getresgid',return_value=(0,0,0)),patch.object(l,'modules',return_value=context), \
             patch.object(l,'native_originals',return_value=originals),patch.object(l.resource,'setrlimit'), \
             patch.object(l.os,'fstat',return_value=originals['mihomo'][1]), \
             patch.object(l.os,'write',side_effect=lambda fd,raw:writes.append(raw) or len(raw)):
            self.assertEqual(l.main(['--isolated-child',json.dumps(frame())]),0)
        self.assertEqual(steps,['release_source_dirs','isolate','bridge_retained_before_init','prepare_copies',
                                'artifacts_retained_before_init','case_constructed','case_run'])
        self.assertEqual(len(writes),1);record=v.decode(writes[0]);v.validate_case(record['case'],f.RAW)
        self.assertFalse(record['parent_whole_known_zero']);self.assertFalse(record['production_effect_authority'])
        self.assertEqual(len(context[3].retained),3)
        self.assertEqual([call.args[0] for call in context[3].phase.call_args_list],
            ['before_copy_prepare','after_copy_prepare','before_artifact_admission',
             'after_artifact_admission','before_artifact_crosscheck','after_artifact_crosscheck',
             'before_case_constructor','after_case_constructor','before_case_run','after_case_run',
             'before_case_receipt_validation','after_case_receipt_validation','before_inner_record_output'])

    def test_changed_native_original_or_late_case_parks_before_later_stage_output(self):
        class Parked(BaseException):pass
        for variant in ('changed','late'):
            context,originals,steps=self.child_context(changed=variant=='changed',late=variant=='late')
            with patch.object(l.os,'getpid',return_value=1),patch.object(l.os,'getresuid',return_value=(0,0,0)), \
                 patch.object(l.os,'getresgid',return_value=(0,0,0)),patch.object(l,'modules',return_value=context), \
                 patch.object(l,'native_originals',return_value=originals),patch.object(l.resource,'setrlimit'), \
                 patch.object(l.os,'fstat',return_value=originals['mihomo'][1]), \
                 patch.object(l.signal,'pause',side_effect=Parked) as park,patch.object(l.os,'write') as output:
                with self.assertRaises(Parked):
                    l.main(['--isolated-child',json.dumps(frame())])
                output.assert_not_called();park.assert_called_once()
            if variant=='changed':self.assertNotIn('case_constructed',steps)
            else:self.assertIn('case_run',steps)
            last=context[3].phase.call_args.args[0]
            self.assertEqual(last,'before_artifact_crosscheck' if variant=='changed' else 'before_case_run')
            self.clock[0]=0.0

    def test_fixed_source_complete_phase_budget_and_public_old_frame_hash(self):
        owner=module('lifecycle')
        positive=ast.parse((HERE/'positive.py').read_text())
        case=next(n for n in positive.body if isinstance(n,ast.ClassDef) and n.name=='Case')
        role_methods={n.name:n for n in case.body if isinstance(n,ast.FunctionDef)}
        lifecycle=ast.parse((HERE/'lifecycle.py').read_text())
        session=next(n for n in lifecycle.body if isinstance(n,ast.ClassDef) and n.name=='Session')
        owner_methods={n.name:n for n in session.body if isinstance(n,ast.FunctionDef)}
        def calls(nodes,env=None):
            env={} if env is None else env;found=[]
            def label(node):
                if isinstance(node,ast.Constant) and type(node.value) is str:return node.value
                if isinstance(node,ast.Name) and node.id in env:return env[node.id]
                if isinstance(node,ast.BinOp) and isinstance(node.op,ast.Add):return label(node.left)+label(node.right)
                self.fail('unknown phase expression requires explicit budget review')
            def has_phase(node):
                return any(isinstance(n,ast.Call) and isinstance(n.func,ast.Attribute)
                    and n.func.attr=='phase' for n in ast.walk(node))
            for node in nodes:
                if isinstance(node,ast.Expr) and isinstance(node.value,ast.Call):
                    call=node.value
                    if isinstance(call.func,ast.Attribute) and call.func.attr=='phase':
                        self.assertIn(len(call.args),(1,2));self.assertFalse(call.keywords)
                        if len(call.args)==2:
                            self.assertIsInstance(call.args[1],ast.Name);self.assertEqual(call.args[1].id,'deadline')
                        found.append(label(call.args[0]))
                    elif isinstance(call.func,ast.Attribute):
                        receiver=ast.unparse(call.func.value)
                        if receiver=='self' and call.func.attr in ('spawn','mapped'):
                            found+=calls(role_methods[call.func.attr].body,{'role':label(call.args[0])})
                        elif receiver=='self.owner' and call.func.attr in ('ready','native_ready'):
                            found+=calls(owner_methods[call.func.attr].body,{'name':label(call.args[0])})
                elif isinstance(node,ast.For) and has_phase(node):
                    if isinstance(node.target,ast.Tuple):
                        self.assertEqual([n.id for n in node.target.elts],['role','argv'])
                        values=[ast.literal_eval(row.elts[0]) for row in node.iter.elts]
                    else:
                        self.assertIsInstance(node.target,ast.Name);self.assertEqual(node.target.id,'role')
                        values=ast.literal_eval(node.iter)
                    for value in values:found+=calls(node.body,{**env,'role':value})
                    self.assertFalse(node.orelse)
                elif isinstance(node,ast.Try):
                    self.assertFalse(any(has_phase(n) for n in node.handlers+node.finalbody+node.orelse))
                    found+=calls(node.body,env)
                elif isinstance(node,ast.Assign) and isinstance(node.value,ast.Call):
                    call=node.value
                    if isinstance(call.func,ast.Attribute) and ast.unparse(call.func.value)=='self' \
                            and call.func.attr=='spawn':
                        found+=calls(role_methods['spawn'].body,{'role':label(call.args[0])})
                elif isinstance(node,ast.While) and has_phase(node):
                    # Readiness repeats observations, but its one post-success
                    # label occurs only on the path that immediately returns.
                    self.assertFalse(node.orelse)
                    found+=calls(node.body,env)
                elif isinstance(node,ast.If) and has_phase(node):
                    self.fail('conditional phase requires explicit branch budget review')
            return found
        launcher=ast.parse((HERE/'launcher.py').read_text())
        child=next(n for n in launcher.body if isinstance(n,ast.FunctionDef) and n.name=='child')
        run=next(n for n in case.body if isinstance(n,ast.FunctionDef) and n.name=='run')
        outer_labels=calls(child.body);case_labels=calls(run.body)
        self.assertEqual((len(outer_labels),len(case_labels)),(13,101))
        self.assertEqual(set(outer_labels+case_labels),owner.PHASES)
        self.assertLessEqual(len(outer_labels+case_labels),owner.PHASE_LIMIT)
        self.assertEqual(owner.PHASE_LIMIT,115)
        self.assertEqual(128+owner.PHASE_LIMIT,243)
        phases=['before_store_create','before_store_mount','before_source_admission']+['before_copy']*25
        phases+=['before_source_recheck','before_fd_inventory','before_store_freeze','before_source_recheck']
        phases+=['before_bind']*50+['before_verify_copies']
        public=''.join('T3_LIVE_FD_PHASE_V1 '+phase+'\n' for phase in phases).encode('ascii')
        self.assertEqual((len(phases),len(public)),(83,2728))
        self.assertEqual(hashlib.sha256(public).hexdigest(),
            '7657611c70eea363c4eca345f1c83a1bc185d12768b43b41bbc158f4835324e6')

    def test_late_namespace_spawn_is_retained_but_cannot_observe_or_read_result(self):
        entry=l.Entry();owner,base,context,_=self.outer()
        owner.spawn.side_effect=lambda *args,**kwargs:self.clock.__setitem__(0,5.0) or SimpleNamespace(pid=999)
        with patch.object(l.os,'getresuid',return_value=(1000,1000,1000)), \
             patch.object(l.os,'getresgid',return_value=(1000,1000,1000)), \
             patch.object(l,'modules',return_value=context),patch.object(l,'native_originals'), \
             patch.object(l,'directories'),patch.object(entry,'opened',side_effect=[10,11]), \
             patch.object(l.resource,'setrlimit'),patch.object(l,'result_bytes') as read, \
             patch.object(l.os,'write') as output:
            with self.assertRaises(RuntimeError):l.parent(entry)
            owner.settle_zero.assert_not_called();owner.complete.assert_not_called()
            read.assert_not_called();output.assert_not_called()

    def test_invalid_expired_or_expanded_enclosing_child_cap_refuses_before_source_open(self):
        for cap in (True,1,float('inf'),float('nan'),0.0,-1.0,91.0):
            entry=l.Entry()
            with patch.object(l.os,'getpid',return_value=1),patch.object(l.os,'getresuid',return_value=(0,0,0)), \
                 patch.object(l.os,'getresgid',return_value=(0,0,0)),patch.object(l,'modules') as load, \
                 patch.object(l.os,'open') as opened,patch.object(l.resource,'setrlimit') as limits:
                with self.assertRaises(l.Refused):l.child(entry,frame(cap))
                load.assert_not_called();opened.assert_not_called();limits.assert_not_called()

    def test_late_child_initialization_cannot_reset_parent_absolute_budget(self):
        self.clock[0]=66.0;entry=l.Entry()
        self.assertEqual(entry.deadline,156.0)
        with patch.object(l.os,'getpid',return_value=1),patch.object(l.os,'getresuid',return_value=(0,0,0)), \
             patch.object(l.os,'getresgid',return_value=(0,0,0)),patch.object(l,'modules') as load, \
             patch.object(l.os,'open') as opened:
            with self.assertRaises(l.Refused):l.child(entry,frame(65.0))
            load.assert_not_called();opened.assert_not_called();self.assertEqual(entry.deadline,65.0)


if __name__=='__main__':unittest.main()
