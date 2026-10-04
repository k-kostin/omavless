"""Inert characterization of #619's pre-inventory diagnostic information gap."""
import unittest
from unittest.mock import Mock, patch

from tests.decoder_live_mapping import bridge, lifecycle, probe


class MappingBoundaryGap(unittest.TestCase):
    def transcript(self, error, second_child=False):
        base, copies, events = Mock(ENV={}), Mock(), []
        copies.inventory.side_effect = [[], error] if second_child else error
        session = lifecycle.Session()
        labels = Mock(breadcrumb=events.append)
        with patch.object(session, 'spawn', side_effect=[Mock(pid=123), Mock(pid=124)]), \
             patch.object(session, 'anchor'), patch.object(session, 'ready'), \
             patch.object(session, 'shutdown') as shutdown, \
             patch.object(probe, 'create'), patch('builtins.open', return_value=Mock()):
            with self.assertRaises(type(error)):
                probe.observe(base, copies, labels, session)
        self.assertTrue(session.sealed)
        self.assertEqual(copies.inventory.call_count, 2 if second_child else 1)
        shutdown.assert_not_called()
        copies.close.assert_not_called()
        self.assertEqual(events[-1], 'before_initial_maps')
        # No action is allowed after the failed operation merely to diagnose it.
        with patch.object(lifecycle.os, 'waitid') as wait, \
             patch.object(lifecycle.os, 'waitpid') as reap, \
             patch.object(lifecycle.os, 'kill') as signal:
            with self.assertRaises(lifecycle.Refused):
                session.perform(copies.inventory, Mock(), 0)
            wait.assert_not_called(); reap.assert_not_called(); signal.assert_not_called()
        return events

    def test_unrelated_initial_mapping_failures_have_identical_before_labels(self):
        cases = [OSError('synthetic read failure'),
                 bridge.Refused('mapped_copy_identity'),
                 bridge.UnknownMapping('/usr/lib/synthetic-unadmitted.so')]
        transcripts = [self.transcript(error) for error in cases]
        self.assertTrue(all(value == transcripts[0] for value in transcripts))

    def test_initial_bus_and_resolved_failure_have_identical_before_labels(self):
        self.assertEqual(self.transcript(OSError('synthetic'), False),
                         self.transcript(OSError('synthetic'), True))


if __name__ == '__main__':
    unittest.main()
