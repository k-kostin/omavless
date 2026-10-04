"""Before-decision diagnostic controls; no ELF execution, mounts or guest calls."""
import json
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

from tests.decoder_mapping_boundaries import bridge, probe


class BoundaryEvents(unittest.TestCase):
    def setUp(self):
        for name, value in (('_candidate_counts', {'bus': 0, 'resolved': 0}),
                            ('_candidate_bytes', 0), ('_candidate_refused', False),
                            ('_breadcrumb_count', 0), ('_breadcrumb_refused', False)):
            change = patch.object(bridge, name, value)
            change.start(); self.addCleanup(change.stop)

    def emit(self, daemon='bus', path='/usr/lib/synthetic.so', identity=(44, 1)):
        captured = []
        with patch.object(bridge.os, 'write', side_effect=lambda fd, data: captured.append((fd, data)) or len(data)):
            bridge.mapped_candidate(daemon, path, identity)
        self.assertEqual(captured[0][0], 2)
        return json.loads(captured[0][1].removeprefix(b'T3_MAP_CANDIDATE_V1 '))

    def fixture(self):
        obj = bridge.Bridge.__new__(bridge.Bridge)
        obj.state = 'ready'
        obj.base = SimpleNamespace(UNSETTLED=[], child_status=Mock(return_value=None))
        obj._child_binding = Mock()
        obj.records = {}
        return obj

    def test_exact_typed_record_explicitly_does_not_admit(self):
        self.assertEqual(self.emit(), {
            'schema': 'public-map-candidate-before-admission-v1', 'daemon': 'bus',
            'pass': 'initial', 'path': '/usr/lib/synthetic.so', 'mapped_device': 44,
            'mapped_inode': 1, 'identity_proven': False, 'adoption': False})

    def test_each_daemon_64_count_and_65th_no_write_permanent_seal(self):
        for daemon in ('bus', 'resolved'):
            for _ in range(64): self.emit(daemon)
        self.assertEqual(bridge._candidate_counts, {'bus': 64, 'resolved': 64})
        self.assertLessEqual(bridge._candidate_bytes, 1024 * 1024)
        with patch.object(bridge.os, 'write') as write:
            for _ in range(2):
                with self.assertRaises(bridge.Refused): bridge.mapped_candidate('bus', '/usr/lib/a', (44, 1))
            write.assert_not_called()

    def test_byte_limit_refuses_before_write_and_cannot_recover(self):
        bridge._candidate_bytes = 1024 * 1024
        with patch.object(bridge.os, 'write') as write:
            with self.assertRaises(bridge.Refused): bridge.mapped_candidate('bus', '/usr/lib/a', (44, 1))
            bridge._candidate_bytes = 0
            with self.assertRaises(bridge.Refused): bridge.mapped_candidate('bus', '/usr/lib/a', (44, 1))
            write.assert_not_called()

    def test_private_deleted_noncanonical_and_bool_identity_never_echoed(self):
        cases = [('bus', '/private/secret', (44, 1)), ('bus', '/usr/lib/a (deleted)', (44, 1)),
                 ('bus', '/usr/lib/../secret', (44, 1)), ('bus', '/usr/lib/a\nsecret', (44, 1)),
                 ('bus', '/usr/lib/a', (True, 1)), ('bus', '/usr/lib/a', (44, False)),
                 ('bus', '/usr/lib/a', (44, 2**64)), ('foreign', '/usr/lib/a', (44, 1)),
                 ('bus', '/usr/lib/' + 'a' * 4096, (44, 1))]
        for args in cases:
            bridge._candidate_refused = False
            with self.subTest(args=args[:1]), patch.object(bridge.os, 'write') as write:
                with self.assertRaises(bridge.Refused): bridge.mapped_candidate(*args)
                write.assert_not_called()

    def test_short_and_unknown_write_are_terminal(self):
        for failure in (0, OSError('synthetic')):
            bridge._candidate_refused = False
            with patch.object(bridge.os, 'write', side_effect=failure if isinstance(failure, Exception)
                              else lambda *_: failure) as write:
                with self.assertRaises((bridge.Refused, OSError)): bridge.mapped_candidate('bus', '/usr/lib/a', (44, 1))
                with self.assertRaises(bridge.Refused): bridge.mapped_candidate('bus', '/usr/lib/a', (44, 1))
                self.assertEqual(write.call_count, 1)

    def test_candidate_precedes_unknown_membership_without_any_target_open(self):
        obj = self.fixture()
        text = '1000-2000 r--p 0 00:2c 1 /usr/lib/unknown.so\n'
        events = []
        with patch.object(bridge, 'breadcrumb', side_effect=events.append), \
             patch.object(bridge, 'bounded_text', return_value=text) as read, \
             patch.object(bridge, 'mapped_candidate', side_effect=lambda *args: events.append(('candidate', args))), \
             patch.object(obj, '_verify_target') as target:
            with self.assertRaises(bridge.UnknownMapping): obj.inventory(SimpleNamespace(pid=123), 100, 'initial_bus')
            self.assertEqual(events[-1], ('candidate', ('bus', '/usr/lib/unknown.so', (44, 1))))
            self.assertEqual(read.call_count, 1)
            target.assert_not_called()
            old = list(events)
            with self.assertRaises(bridge.Refused): obj.inventory(SimpleNamespace(pid=123), 100, 'initial_bus')
            self.assertEqual(events, old)

    def test_malformed_maps_stop_at_before_parse_no_candidate(self):
        obj = self.fixture(); events = []
        with patch.object(bridge, 'breadcrumb', side_effect=events.append), \
             patch.object(bridge, 'bounded_text', return_value='malformed private input'), \
             patch.object(bridge, 'mapped_candidate') as candidate:
            with self.assertRaises(bridge.Refused): obj.inventory(SimpleNamespace(pid=123), 100, 'initial_resolved')
            self.assertEqual(events[-1], 'initial_resolved_before_map_parse')
            candidate.assert_not_called()

    def test_wrong_identity_recorded_before_refusal_but_not_hashed(self):
        obj = self.fixture(); path = '/usr/lib/a'
        obj.records[path] = {'device': 44, 'inode': 2}
        with patch.object(bridge, 'breadcrumb'), \
             patch.object(bridge, 'bounded_text', return_value=f'1000-2000 r--p 0 00:2c 1 {path}\n'), \
             patch.object(bridge, 'mapped_candidate') as candidate, patch.object(obj, '_verify_target') as target:
            with self.assertRaisesRegex(bridge.Refused, 'mapped_copy_identity'):
                obj.inventory(SimpleNamespace(pid=123), 100, 'initial_bus')
            candidate.assert_called_once_with('bus', path, (44, 1)); target.assert_not_called()

    def test_final_and_shutdown_have_no_candidate_records(self):
        for context in ('final_bus', 'final_resolved', None):
            obj = self.fixture(); path = '/usr/lib/a'
            obj.records[path] = {'device': 44, 'inode': 1, 'size': 12, 'sha256': 'a' * 64}
            with patch.object(bridge, 'breadcrumb'), \
                 patch.object(bridge, 'bounded_text', return_value=f'1000-2000 r--p 0 00:2c 1 {path}\n'), \
                 patch.object(bridge, 'mapped_candidate') as candidate, patch.object(obj, '_verify_target'):
                obj.inventory(SimpleNamespace(pid=123), 100, context)
                candidate.assert_not_called()

    def test_record_write_unknown_prevents_target_and_second_maps_read(self):
        obj = self.fixture(); path = '/usr/lib/a'
        obj.records[path] = {'device': 44, 'inode': 1}
        with patch.object(bridge, 'breadcrumb'), \
             patch.object(bridge, 'bounded_text', return_value=f'1000-2000 r--p 0 00:2c 1 {path}\n') as read, \
             patch.object(bridge.os, 'write', side_effect=OSError('synthetic')) as write, \
             patch.object(obj, '_verify_target') as target:
            with self.assertRaises(OSError): obj.inventory(SimpleNamespace(pid=123), 100, 'initial_bus')
            with self.assertRaises(bridge.Refused): obj.inventory(SimpleNamespace(pid=123), 100, 'initial_bus')
            self.assertEqual(write.call_count, 1); self.assertEqual(read.call_count, 1)
            target.assert_not_called()

    def test_maximum_public_path_records_fit_byte_budget_and_phase_cap_unchanged(self):
        path = '/usr/lib/' + 'a' * (4096 - len('/usr/lib/'))
        for daemon in ('bus', 'resolved'):
            for _ in range(64): self.emit(daemon, path, (2**64 - 1, 2**64 - 1))
        self.assertLess(bridge._candidate_bytes, 1024 * 1024)
        with patch.object(bridge.os, 'write', side_effect=lambda _, data: len(data)) as write:
            for _ in range(128): bridge.breadcrumb('before_copy')
            with self.assertRaises(bridge.Refused): bridge.breadcrumb('before_copy')
            self.assertEqual(write.call_count, 128)

    def test_fixed_probe_supplies_contexts_not_caller_argv(self):
        base, copies, session = Mock(ENV={}), Mock(), Mock(retained=[])
        session.perform.side_effect = lambda fn, *args: fn(*args)
        session.spawn.side_effect = [SimpleNamespace(pid=123), SimpleNamespace(pid=124)]
        with patch.object(probe, 'create'), patch('builtins.open', return_value=Mock()):
            probe.observe(base, copies, Mock(), session)
        self.assertEqual([call.args[2] for call in copies.inventory.call_args_list],
                         ['initial_bus', 'initial_resolved', 'final_bus', 'final_resolved'])


if __name__ == '__main__': unittest.main()
