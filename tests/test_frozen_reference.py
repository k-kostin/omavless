# SPDX-License-Identifier: MIT
"""Pure fixture-reader tests; no archived backend, host, VPN or network calls."""
import importlib.util
import io
import json
import os
import subprocess
import sys
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('frozen_reference', Path(__file__).resolve().parents[1] / 'tools/frozen_reference.py')
reference = importlib.util.module_from_spec(spec)
spec.loader.exec_module(reference)
recorder_spec = importlib.util.spec_from_file_location('record_frozen_reference', Path(__file__).resolve().parents[1] / 'tools/record_frozen_reference.py')
recorder = importlib.util.module_from_spec(recorder_spec)
with patch.dict(sys.modules, frozen_reference=reference):
    recorder_spec.loader.exec_module(recorder)


class FrozenReferenceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.tool, self.args, self.payload = 'synthetic_parity.py', ['request'], b'{"synthetic":true}'
        self.key = reference.request_key(self.args, self.payload)
        self.path = reference.fixture_path(self.tool, self.key, self.root)
        self.path.parent.mkdir()
        self.value = dict(schemaVersion=1, referenceCommit=reference.REFERENCE,
                          tool=self.tool, requestSha256=self.key, inputBytes=len(self.payload),
                          argumentCount=1, returncode=0, stdout='{"accepted":true}\n', stderr='')
        self.write(self.value)

    def write(self, value):
        self.path.write_text(json.dumps(value), encoding='utf-8')

    def replay(self):
        return reference.replay(self.tool, self.args, self.payload, self.root)

    def test_valid_independent_reply(self):
        self.assertEqual(self.replay(), self.value)

    def test_changed_bytes_arguments_or_tool_do_not_reuse_an_answer(self):
        for tool, args, data in [('other.py', self.args, self.payload),
                                 (self.tool, ['response'], self.payload),
                                 (self.tool, self.args, self.payload + b' ')]:
            with self.assertRaises(ValueError):
                reference.replay(tool, args, data, self.root)

    def test_provenance_bounds_envelope_and_types_fail_closed(self):
        for field, invalid in [('schemaVersion', True), ('referenceCommit', 'b' * 40),
                               ('tool', 'other.py'), ('requestSha256', 'c' * 64),
                               ('inputBytes', True), ('inputBytes', 0), ('argumentCount', False),
                               ('argumentCount', 2), ('returncode', True), ('returncode', 255),
                               ('stdout', []), ('stderr', {})]:
            with self.subTest(field=field, invalid=invalid):
                self.write(dict(self.value, **{field: invalid}))
                with self.assertRaises(ValueError):
                    self.replay()
        self.write(dict(self.value, unexpected='value'))
        with self.assertRaises(ValueError):
            self.replay()

    def test_duplicate_fields_and_non_objects_refused(self):
        for text in ('[]', 'null', '{"schemaVersion":1,"schemaVersion":1}'):
            self.path.write_text(text)
            with self.assertRaises((ValueError, TypeError)):
                self.replay()

    def test_symlink_and_oversized_fixture_refused(self):
        self.path.unlink()
        target = self.root / 'other.json'
        target.write_text(json.dumps(self.value))
        self.path.symlink_to(target)
        with self.assertRaises(ValueError):
            self.replay()
        self.path.unlink()
        self.write(self.value)
        with patch.object(reference, 'LIMIT', 32):
            with self.assertRaises(ValueError):
                self.replay()

    def test_request_and_tool_bounds(self):
        with patch.object(reference, 'LIMIT', 8):
            with self.assertRaises(ValueError):
                reference.request_key([], b'x' * 9)
        for name in ('../backend.py', '/backend.py', 'unsafe;name.py', 'private\n.py'):
            with self.assertRaises(ValueError):
                reference.fixture_path(name, self.key, self.root)
        for key in ('../outside', 'a' * 63, 'A' * 64):
            with self.assertRaises(ValueError):
                reference.fixture_path(self.tool, key, self.root)

    def test_missing_fixture_error_never_echoes_private_input_or_falls_back(self):
        marker = b'vless://synthetic.invalid?password=synthetic-private-canary'
        class Input:
            buffer = io.BytesIO(marker)
        with patch.dict(os.environ, {}, clear=True), \
             patch('subprocess.Popen') as process, \
             patch.object(reference.sys, 'stdin', Input()), \
             patch.object(reference.sys, 'stdout', io.StringIO()) as stdout, \
             patch.object(reference.sys, 'stderr', io.StringIO()) as stderr:
            self.assertEqual(reference.main('missing_parity.py', []), 2)
            self.assertEqual(stdout.getvalue(), '')
            self.assertNotIn(marker.decode(), stderr.getvalue())
            self.assertEqual(stderr.getvalue(), 'Frozen reference unavailable or invalid; review the independent fixture.\n')
            process.assert_not_called()

    def test_same_bytes_have_stable_key_and_framed_args_are_distinct(self):
        self.assertEqual(reference.request_key(self.args, self.payload), self.key)
        self.assertNotEqual(reference.request_key(['ab', 'c'], b'd'),
                            reference.request_key(['a', 'bc'], b'd'))


class RecorderTests(unittest.TestCase):
    """No real archive/tool executes; prove the opt-in recorder's boundaries."""
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.archive = Path(self.temp.name) / 'archive'
        (self.archive / 'tools').mkdir(parents=True)
        self.tool = 'control_protocol_parity.py'
        self.source = self.archive / 'tools' / self.tool
        self.source.write_bytes(b'# synthetic archived tool, never executed\n')
        self.destination = Path(self.temp.name) / 'fixtures' / 'reply.json'
        self.environment = patch.dict(os.environ, OMAVLESS_REFERENCE_CHECKOUT=str(self.archive), OMAVLESS_RECORD_REFERENCE='1')
        self.environment.start()
        self.addCleanup(self.environment.stop)
        self.location = patch.object(recorder, 'fixture_path', return_value=self.destination)
        self.location.start()
        self.addCleanup(self.location.stop)
        self.git = patch.object(recorder.subprocess, 'check_output', side_effect=self.git_reply).start()
        self.addCleanup(patch.stopall)
        self.run = patch.object(recorder.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, b'{"accepted":true}\n', b'')).start()

    def git_reply(self, argv, **kwargs):
        command = argv[3]
        if command == 'rev-parse':
            return (reference.REFERENCE + '\n').encode()
        if command == 'status':
            return b''
        if command == 'show':
            return self.source.read_bytes()
        self.fail('unexpected Git command')

    def record(self):
        return recorder.record(self.tool, ['request'], b'{"synthetic":true}')

    def test_publish_complete_reply_and_allow_identical_repeat(self):
        value = self.record()
        self.assertEqual(json.loads(self.destination.read_bytes()), value)
        self.assertEqual(self.record(), value)
        self.assertEqual(list(self.destination.parent.glob('.record-*')), [])
        args, kwargs = self.run.call_args
        self.assertEqual(args[0], [sys.executable, str(self.source), 'request'])
        self.assertNotIn('OMAVLESS_RECORD_REFERENCE', kwargs['env'])
        self.assertEqual(kwargs['cwd'], self.archive)
        self.assertEqual(kwargs['timeout'], 45)

    def test_existing_conflict_not_overwritten(self):
        self.record()
        before = self.destination.read_bytes()
        self.run.return_value.stdout = b'{"different":true}\n'
        with self.assertRaises(ValueError):
            self.record()
        self.assertEqual(self.destination.read_bytes(), before)
        self.assertEqual(list(self.destination.parent.glob('.record-*')), [])

    def test_existing_symlink_not_followed(self):
        self.destination.parent.mkdir()
        target = Path(self.temp.name) / 'untouched'
        target.write_bytes(b'unchanged')
        self.destination.symlink_to(target)
        with self.assertRaises(ValueError):
            self.record()
        self.assertEqual(target.read_bytes(), b'unchanged')

    def test_changed_commit_or_dirty_archive_never_executes(self):
        for replies in ([b'b' * 40 + b'\n'], [(reference.REFERENCE + '\n').encode(), b' M backend.py\n']):
            self.git.side_effect = replies
            with self.assertRaises(ValueError):
                self.record()
            self.run.assert_not_called()

    def test_noncanonical_or_current_checkout_refused(self):
        for location in ('relative', str(recorder.ROOT)):
            with patch.dict(os.environ, OMAVLESS_REFERENCE_CHECKOUT=location):
                with self.assertRaises(ValueError):
                    self.record()
        self.run.assert_not_called()

    def test_unlisted_and_live_tools_never_execute(self):
        for tool in ('route_live_reference.py', 'control_protocol_probe.py', 'arbitrary.py', '../backend.py'):
            with self.assertRaises(ValueError):
                recorder.record(tool, [], b'')
        self.git.assert_not_called()
        self.run.assert_not_called()

    def test_changed_or_symlink_tool_never_executes(self):
        self.git.side_effect = [(reference.REFERENCE + '\n').encode(), b'', b'different blob']
        with self.assertRaises(ValueError):
            self.record()
        self.git.side_effect = self.git_reply
        self.source.unlink()
        self.source.symlink_to(Path(self.temp.name) / 'missing')
        with self.assertRaises(ValueError):
            self.record()
        self.run.assert_not_called()

    def test_invalid_exit_encoding_or_escaped_output_bound_not_recorded(self):
        for code, data in ((3, b''), (0, b'\xff'), (0, b'\x00' * 100)):
            self.run.return_value = subprocess.CompletedProcess([], code, data, b'')
            with patch.object(recorder, 'LIMIT', 512):
                with self.assertRaises((ValueError, UnicodeError)):
                    self.record()
            self.assertFalse(self.destination.exists())


if __name__ == '__main__':
    unittest.main()
