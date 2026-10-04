"""Finite static-boundary transcripts: no readelf, candidate, guest or process."""
import unittest
from unittest.mock import patch
from tests.encoder_static_boundaries import probe
from tests.test_encoder_boundary_provenance import capture_fixture, MANIFEST


class EventsTests(unittest.TestCase):
    def run_capture(self, sources, base, helper, events, writes):
        def writer(fd, raw):
            self.assertEqual(fd, 2)
            writes.append(raw)
            return len(raw)
        with patch.object(probe, 'EVENTS', events), patch.object(probe, 'Sources', return_value=sources), patch.object(probe.os, 'write', side_effect=writer):
            return probe.capture(base, helper, helper.owned, MANIFEST)

    def test_success_transcript_is_only_finite_before_labels(self):
        sources,base,helper=capture_fixture(); events=probe.Events(); writes=[]
        self.run_capture(sources,base,helper,events,writes)
        self.assertFalse(events.sealed)
        labels=[r.decode().removeprefix('T3_ENCODER_STATIC_BEFORE_V1 ').strip() for r in writes]
        self.assertTrue(set(labels)<=probe.PHASES)
        self.assertLess(labels.index('package_desc'),labels.index('package_parse'))
        self.assertLess(labels.index('before_readelf'),labels.index('readelf_decode'))
        self.assertEqual(labels[-1],'receipt')
        self.assertNotIn('/usr/',b''.join(writes).decode())

    def test_package_failure_seals_no_next_package_tool_or_query(self):
        sources,base,helper=capture_fixture(); events=probe.Events(); writes=[]
        sources.take.side_effect=OSError('private-sensitive-exception')
        with self.assertRaises(OSError): self.run_capture(sources,base,helper,events,writes)
        self.assertTrue(events.sealed); self.assertEqual(events.last,'package_desc')
        with patch.object(probe,'EVENTS',events), patch.object(probe,'Sources') as later:
            with self.assertRaises(RuntimeError): probe.capture(base,helper,helper.owned,MANIFEST)
            later.assert_not_called()
        base.command.assert_not_called(); self.assertEqual(sources.take.call_count,1)
        with patch.object(probe.os,'write',side_effect=lambda _,raw:writes.append(raw) or len(raw)):
            events.failure(); events.failure()
        self.assertEqual(writes[-1],b'T3_ENCODER_STATIC_FAILED_AT_V1 package_desc\n')
        self.assertEqual(sum(b'FAILED_AT' in r for r in writes),1)
        self.assertNotIn(b'private',b''.join(writes))

    def test_unknown_readelf_no_later_decode_recheck_or_event(self):
        sources,base,helper=capture_fixture(); events=probe.Events(); writes=[]
        base.command.side_effect=RuntimeError('unknown')
        with self.assertRaises(RuntimeError): self.run_capture(sources,base,helper,events,writes)
        self.assertEqual(events.last,'before_readelf'); self.assertTrue(events.sealed)
        helper.decode_readelf.assert_not_called()
        self.assertEqual(sources.recheck.call_count,1)

    def test_queued_identity_failure_before_next_object_or_command(self):
        sources,base,helper=capture_fixture(); events=probe.Events(); writes=[]
        helper.canonical_public.side_effect=OSError('unknown')
        with self.assertRaises(OSError): self.run_capture(sources,base,helper,events,writes)
        self.assertEqual(events.last,'canonical_queue')
        self.assertNotIn(probe.CANDIDATE,[c.args[0] for c in sources.take.call_args_list])
        base.command.assert_not_called()

    def test_short_unknown_or_cancel_event_write_never_retried(self):
        for result in (0,OSError('unknown'),KeyboardInterrupt()):
            events=probe.Events()
            with patch.object(probe.os,'write',side_effect=result if isinstance(result,BaseException) else None,return_value=result) as write:
                with self.assertRaises(BaseException): events.before('package_desc')
                with self.assertRaises(RuntimeError): events.before('package_files')
                events.failure()
            self.assertTrue(events.sealed); write.assert_called_once()

    def test_event_unknown_enum_count_and_byte_bounds_before_write(self):
        for kind in ('unknown','count','bytes'):
            events=probe.Events()
            if kind=='count': events.count=2048
            if kind=='bytes': events.total=130816
            with patch.object(probe.os,'write') as write:
                with self.assertRaises(RuntimeError): events.before('/private' if kind=='unknown' else 'source_read')
                events.failure()
                write.assert_not_called()
            self.assertTrue(events.sealed)

    def test_failed_phase_write_unknown_single_attempt(self):
        events=probe.Events()
        with patch.object(probe.os,'write',side_effect=OSError('unknown')) as write:
            with self.assertRaises(OSError): events.failure()
            events.failure()
            with self.assertRaises(RuntimeError): events.before('output')
            write.assert_called_once()

    def test_main_failure_write_error_cannot_echo_private_exception_context(self):
        import ast
        from pathlib import Path
        from unittest.mock import Mock
        tree=ast.parse(Path(probe.__file__).read_text())
        guard=ast.Module(body=[tree.body[-1]],type_ignores=[])
        events=probe.Events()
        namespace=dict(vars(probe), __name__='__main__', EVENTS=events,
                       main=Mock(side_effect=RuntimeError('PRIVATE_NOT_FOR_OUTPUT')))
        with patch.object(probe.os,'umask'),patch.object(probe.os,'write',side_effect=OSError('PRIVATE_WRITE')) as write:
            with self.assertRaises(SystemExit) as raised:
                exec(compile(guard,'<synthetic-main-guard>','exec'),namespace)
        self.assertEqual(raised.exception.code,1)
        self.assertTrue(raised.exception.__suppress_context__)
        write.assert_called_once_with(2,b'T3_ENCODER_STATIC_FAILED_AT_V1 entry\n')


if __name__=='__main__': unittest.main()
