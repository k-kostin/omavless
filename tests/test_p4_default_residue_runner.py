"""Inert runner/source graph guards; never call main, Go or an engine command."""
import ast
from contextlib import ExitStack, contextmanager
import hashlib
import importlib.util
import os
from pathlib import Path
import tempfile
import sys
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).parent / 'fixtures/p4_awg_peer/run_default_residue_overlay.py'
spec = importlib.util.spec_from_file_location('p4_residue_runner', SOURCE)
subject = importlib.util.module_from_spec(spec)
spec.loader.exec_module(subject)


class RunnerControls(unittest.TestCase):
    def test_three_add_only_overlays_and_frozen_helpers_are_exact(self):
        values = subject.inputs()
        self.assertEqual(len(values),7)
        for name, digest in (('supervisor.py',subject.SUPERVISOR_SHA),
                             ('export-helper.py',subject.HELPER_SHA),
                             ('receipt-parser.py',subject.RECEIPT_SHA)):
            self.assertEqual(hashlib.sha256(values[name]).hexdigest(),digest)
        for _,(digest,artifact) in subject.OVERLAYS.items():
            self.assertEqual(hashlib.sha256(values[artifact]).hexdigest(),digest)
        self.assertEqual(set(subject.receipts.SELECTORS),{'reject','exhaust'})

    def test_definition_loader_reuses_exact_pinned_bodies_no_path_reload(self):
        raw = subject.SUPERVISOR_BYTES
        with patch.object(subject.owned.subprocess,'Popen') as create:
            subject.definitions(raw,'inert-supervisor')
            create.assert_not_called()
        for changed in (raw+b'\n',b'UNKNOWN',True):
            with self.assertRaises(ValueError):subject.definitions(changed,'inert')
        code = SOURCE.read_text()
        self.assertIn('exec(definitions(SUPERVISOR_BYTES',code)
        self.assertNotIn('exec(compile(SUPERVISOR_BYTES',code)
        names = {node.name for node in ast.parse(raw).body if isinstance(node,ast.FunctionDef)}
        self.assertEqual(names,{'members','reap','command','private_directory','save'})
        self.assertEqual(subject.owned.command.__code__.co_filename,'fixed-p4-supervisor')
        self.assertEqual(subject.owned.UNSETTLED,[])
        self.assertEqual(subject.owned.HELD_GRAPHS,[])

    def test_helper_parser_and_overlay_drift_refuse_without_children(self):
        with patch.object(subject.owned.subprocess,'Popen') as create:
            with patch.object(subject,'object_bytes',return_value=b'UNKNOWN'):
                with self.assertRaises(ValueError):subject.pinned(SOURCE,subject.HELPER_SHA)
            with patch.object(Path,'read_bytes',return_value=b'UNKNOWN'):
                with self.assertRaises(ValueError):subject.inputs()
            create.assert_not_called()

    def test_frozen_object_nonfollow_singlelink_mode_size_predicates(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory);path=root/'member'
            subject.owned.save(root,'member',b'fixed inert bytes')
            self.assertEqual(subject.object_bytes(path,100),b'fixed inert bytes')
            with self.assertRaises(ValueError):subject.object_bytes(path,2)
            alias=root/'alias';alias.symlink_to(path)
            with self.assertRaises(OSError):subject.object_bytes(alias,100)
            alias.unlink();os.link(path,alias)
            with self.assertRaises(ValueError):subject.object_bytes(path,100)
            alias.unlink();path.chmod(0o620)
            with self.assertRaises(ValueError):subject.object_bytes(path,100)

    def test_build_and_execute_are_distinct_offline_exact_single_case(self):
        code=SOURCE.read_text()
        for literal in ('choices=("build", "execute")','choices=tuple(receipts.SELECTORS)',
                        '"-c"','"-tags=p4_cookie_overlay,p4_default_residue_overlay"',
                        '"-mod=readonly"','GOPROXY="off"','GOSUMDB="off"',
                        'GOTOOLCHAIN="local"','GOWORK="off"','GOENV="off"',
                        'GOTMPDIR=str(scratch)','TMPDIR=str(scratch)',
                        '"-test.count=1"','"-test.timeout=600s"',
                        'body_limit_seconds=570','supervisor_timeout_seconds=620',
                        'intrinsic_engine_heap_cap=False'):
            self.assertIn(literal,code)
        self.assertNotIn('--count',code)
        self.assertNotIn('--diagnostic',code)
        self.assertNotIn('helpers.bounded_command(',code)
        self.assertNotIn('subprocess.Popen(',code)

    def test_attempt_precedes_only_execution_and_uncertain_graph_is_preserved(self):
        code=SOURCE.read_text()
        self.assertLess(code.index('owned.save(artifacts, "execution-attempt.json"'),
                        code.index('"tool", "test2json"'))
        for literal in ('"supervision-refusal.json"','"complete_output_retained": False',
                        '"unsettled_anchors": len(owned.UNSETTLED)','"pass": False',
                        'if owned.UNSETTLED:','receipts.verify_events(out, args.case)',
                        '"actual_default_residue_nonpass_preserve"'):
            self.assertIn(literal,code)
        self.assertEqual(code.count('"tool", "test2json"'),1)
        self.assertNotIn('rmtree',code)
        self.assertNotIn('import shutil',code)
        # Source positions only, not supervision/cancellation execution proof.

    def test_limits_are_inertly_bounded_without_relaxing_inherited_soft_caps(self):
        calls=[]
        with patch.object(subject.resource,'getrlimit',return_value=(subject.resource.RLIM_INFINITY,subject.resource.RLIM_INFINITY)), \
             patch.object(subject.resource,'setrlimit',side_effect=lambda kind,value:calls.append((kind,value))):
            subject.limits()
        self.assertEqual([value[0] for _,value in calls],[512,1024,660,256*1024*1024,0])
        calls=[]
        with patch.object(subject.resource,'getrlimit',return_value=(1,2)), \
             patch.object(subject.resource,'setrlimit',side_effect=lambda kind,value:calls.append((kind,value))):
            subject.limits()
        self.assertEqual([value for _,value in calls],[(1,2)]*4+[(0,2)])
        code=SOURCE.read_text()
        self.assertNotIn('resource.RLIMIT_AS',code)
        self.assertNotIn('resource.RLIMIT_RSS',code)

    @contextmanager
    def harness(self, failure=None):
        # Mocked coordinator paths only: no Go/tool/engine process, archive
        # execution, resource change or native tool bytes are touched.
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            root=Path(directory)
            paths={name:root/name for name in ('source','modules','scratch','cache','artifacts')}
            for path in paths.values():path.mkdir(mode=0o700)
            argv=['inert','--phase','build','--case','reject']
            for flag,name in (('source','source'),('module-cache','modules'),('scratch','scratch'),('cache','cache'),('artifacts','artifacts')):
                argv += ['--'+flag,str(paths[name])]
            stack.enter_context(patch.object(sys,'argv',argv))
            stack.enter_context(patch.object(subject,'limits'))
            read=Path.read_bytes
            stack.enter_context(patch.object(Path,'read_bytes',lambda path:b'INERT_TOOL_NEVER_READ' if str(path)=='/usr/bin/go' else read(path)))
            git=stack.enter_context(patch.object(subject.owned.helpers,'git',return_value=b'f'*40))
            verify=stack.enter_context(patch.object(subject.owned.helpers,'verify_source',return_value=b'INERT_ARCHIVE_NOT_EXECUTED'))
            exported=[]
            def export(archive,path):
                (path/'device').mkdir(mode=0o700);exported.append(path);return {}
            stack.enter_context(patch.object(subject.owned.helpers,'export_source',side_effect=export))
            stack.enter_context(patch.object(subject.owned.helpers,'verify_export'))
            saved=[];save=subject.owned.save
            def record(path,name,raw,mode=0o600):
                saved.append(name);return save(path,name,raw,mode)
            stack.enter_context(patch.object(subject.owned,'save',side_effect=record))
            def command(args,*unused):
                kind='modules' if args[1]=='mod' else 'compile' if args[1]=='test' else 'execute'
                if failure==kind:return 2,b'',b'INERT_FIXED_NONPASS'
                if failure==kind+'_throw':raise ValueError('INERT_FIXED_REFUSAL')
                if kind=='modules':return 0,b'all modules verified\n',b''
                if kind=='compile':save(paths['artifacts'],'device.test',b'INERT_NOT_AN_ELF_NOT_EXECUTED',0o700);return 0,b'',b''
                from tests.test_p4_default_residue_source import ReceiptControls
                control=ReceiptControls()
                return 0,control.encoded(control.events('reject')),b''
            commands=stack.enter_context(patch.object(subject.owned,'command',side_effect=command))
            output=stack.enter_context(patch('builtins.print'))
            yield paths,argv,git,verify,exported,saved,commands,output

    def test_mocked_failed_builds_preserve_without_postfailure_query_or_receipt(self):
        for failure in ('modules','compile','modules_throw','compile_throw'):
            with self.subTest(failure=failure),self.harness(failure) as state:
                paths,_,git,verify,exported,saved,commands,output=state
                with self.assertRaises(ValueError):subject.main()
                self.assertEqual(verify.call_count,1);self.assertEqual(git.call_count,3)
                self.assertTrue(exported[0].exists())
                self.assertNotIn('build-receipt.json',saved)
                output.assert_not_called()
                self.assertEqual(commands.call_count,1 if failure.startswith('modules') else 2)

    def test_mocked_each_final_drift_cut_has_no_success_receipt(self):
        for phase in ('build','execute'):
            for cut in ('input','fixture','tool','source','unknown_owner'):
                with self.subTest(phase=phase,cut=cut),self.harness() as state:
                    paths,argv,git,verify,exported,saved,_,_=state
                    if phase=='execute':
                        subject.main();argv[argv.index('build')]='execute';saved.clear()
                    if cut=='input':
                        captured=subject.inputs();changed={**captured,'runner.py':b'INERT_DRIFT'}
                        context=patch.object(subject,'inputs',side_effect=[captured,changed])
                    elif cut=='fixture':context=patch.object(subject.owned.helpers,'git',side_effect=[b'f'*40,b'',b'',b'0'*40])
                    elif cut=='tool':
                        read=Path.read_bytes;go_reads=[]
                        def drift(path):
                            if str(path)=='/usr/bin/go':
                                go_reads.append(True);return b'INERT_TOOL_NEVER_READ' if len(go_reads)==1 else b'INERT_DRIFT'
                            return read(path)
                        context=patch.object(Path,'read_bytes',drift)
                    elif cut=='source':context=patch.object(subject.owned.helpers,'verify_source',side_effect=[b'INERT_ARCHIVE_NOT_EXECUTED',ValueError('INERT_DRIFT')])
                    else:context=patch.object(subject,'recheck',side_effect=subject.owned.Unsettled('INERT_UNKNOWN_OWNER'))
                    with context,self.assertRaises((ValueError,subject.owned.Unsettled)):subject.main()
                    self.assertTrue(exported[0].exists())
                    self.assertNotIn('build-receipt.json' if phase=='build' else 'execution-receipt.json',saved)
                    if phase=='execute':self.assertTrue((paths['artifacts']/'execution-attempt.json').exists())

    def test_mocked_success_receipt_is_last_after_rechecks_and_flushed_output(self):
        with self.harness() as state:
            paths,argv,_,verify,exported,saved,_,output=state
            for phase,name in (('build','build-receipt.json'),('execute','execution-receipt.json')):
                if phase=='execute':argv[argv.index('build')]='execute'
                checkpoint=[]
                recheck=subject.recheck
                def checked(*args):checkpoint.append('recheck');return recheck(*args)
                def printed(*args,**kwargs):
                    self.assertEqual(checkpoint,['recheck']);self.assertNotIn(name,saved)
                    self.assertTrue(args[0].startswith('PROVISIONAL_'))
                    self.assertTrue(kwargs['flush']);checkpoint.append('print')
                with patch.object(subject,'recheck',side_effect=checked),patch('builtins.print',side_effect=printed):subject.main()
                self.assertEqual(saved[-1],name);self.assertEqual(checkpoint,['recheck','print'])
            self.assertTrue(exported[0].exists())
            self.assertEqual(verify.call_count,4)

    def test_mocked_failed_final_output_never_issues_success_receipt(self):
        for phase in ('build','execute'):
            with self.subTest(phase=phase),self.harness() as state:
                paths,argv,_,_,exported,saved,_,_=state
                if phase=='execute':subject.main();argv[argv.index('build')]='execute';saved.clear()
                with patch('builtins.print',side_effect=OSError('INERT_OUTPUT_UNKNOWN')),self.assertRaises(OSError):subject.main()
                self.assertTrue(exported[0].exists())
                self.assertNotIn('build-receipt.json' if phase=='build' else 'execution-receipt.json',saved)

    def test_unknown_owner_recheck_performs_no_followup_query_or_read(self):
        with patch.object(subject.owned,'UNSETTLED',[object()]), \
             patch.object(subject,'inputs') as inputs, \
             patch.object(subject.owned.helpers,'git') as git, \
             patch.object(subject.owned.helpers,'verify_source') as verify, \
             patch.object(Path,'read_bytes') as read:
            with self.assertRaises(subject.owned.Unsettled):subject.recheck({},'inert','inert',Path('inert'))
            for effect in (inputs,git,verify,read):effect.assert_not_called()

    def test_mocked_final_save_failure_is_unaccepted_even_if_bytes_exist(self):
        for phase in ('build','execute'):
            with self.subTest(phase=phase),self.harness() as state:
                paths,argv,git,verify,exported,saved,commands,output=state
                if phase=='execute':subject.main();argv[argv.index('build')]='execute';saved.clear()
                name='build-receipt.json' if phase=='build' else 'execution-receipt.json'
                save=subject.owned.save
                def failed(path,filename,raw,mode=0o600):
                    save(path,filename,raw,mode)
                    if filename==name:raise OSError('INERT_FINAL_CLOSE_UNKNOWN')
                with patch.object(subject.owned,'save',side_effect=failed),self.assertRaises(OSError):subject.main()
                self.assertTrue(exported[0].exists());self.assertEqual(saved[-1],name)
                self.assertTrue((paths['artifacts']/name).exists())
                # Full bytes may survive an inherited save/close error. They
                # are NOT accepted: main raised, no known-zero CLI terminal.
                self.assertTrue(output.call_args.args[0].startswith('PROVISIONAL_'))


if __name__ == '__main__':
    unittest.main()
