"""Inert runner/source graph guards; never call main, Go or an engine command."""
import ast
import hashlib
import importlib.util
import os
from pathlib import Path
import tempfile
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
                        'if not owned.UNSETTLED:','receipts.verify_events(out, args.case)',
                        '"actual_default_residue_nonpass_preserve"'):
            self.assertIn(literal,code)
        self.assertEqual(code.count('"tool", "test2json"'),1)
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


if __name__ == '__main__':
    unittest.main()
