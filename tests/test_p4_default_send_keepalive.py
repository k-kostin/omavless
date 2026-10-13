"""Inert keepalive parser/source/runner controls. No engine/compiler invocation."""
from contextlib import contextmanager
import hashlib
import json
from pathlib import Path
import unittest
from unittest.mock import patch

from tests import test_p4_default_residue_runner as baseline
from tests import test_p4_ordinary_toolchain as ordinary

ROOT = Path(__file__).parent / 'fixtures/p4_awg_peer'
subject = ordinary.load('p4_inert_keepalive_runner', 'run_default_send_keepalive.py')


def values():
    value = {'case': 'keepalive', 'network_fds': 'false'}
    value.update({field: 'true' for field in subject.receipts.TRUE})
    value.update(scheduled_ns=10_000_000_000, keepalive_ns=10_005_000_000,
                 consume_ns=1_000_000, receive_ns=2_000_000, quiet_ns=1_501_000_000,
                 teardown_ns=3_000_000, body_ns=11_520_000_000)
    return value


def events(value=None):
    value = values() if value is None else value
    selected = subject.receipts.SELECTORS['keepalive']
    output = '    default_send_keepalive_test.go:300: p4_keepalive_receipt ' + ' '.join(
        key + '=' + str(item) for key, item in value.items()) + '\n'
    sequence = ordinary.native_events()
    for event in sequence:
        if 'Test' in event: event['Test'] = selected
    sequence[2]['Output'] = '=== RUN   ' + selected + '\n'
    sequence[3]['Output'] = output
    sequence[4]['Output'] = '--- PASS: ' + selected + ' (11.52s)\n'
    sequence[5]['Elapsed'], sequence[7]['Elapsed'] = 11.52, 11.53
    return sequence


class KeepaliveControls(unittest.TestCase):
    def verify(self, sequence):
        return subject.receipts.verify_events(subject.formats.normalize(ordinary.encoded(sequence)), 'keepalive')

    def test_exact_native_frames_and_finite_causal_positive(self):
        value = self.verify(events())
        self.assertEqual(value['scheduled_ns'], 10_000_000_000)
        self.assertEqual(value['body_ns'], 11_520_000_000)

    def test_every_observation_true_is_required_without_extra_fields(self):
        for field in subject.receipts.TRUE:
            for alteration in ('false', None):
                value = values()
                if alteration is None: del value[field]
                else: value[field] = alteration
                with self.subTest(field=field, alteration=alteration), self.assertRaises(ValueError):
                    self.verify(events(value))
        value = values(); value['private_key'] = 'secret'
        with self.assertRaises(ValueError): self.verify(events(value))

    def test_actual_default_and_causal_timing_bounds_reject_punctures(self):
        for field, bad in (('scheduled_ns', 1_000_000_000), ('keepalive_ns', 9_999_999_999),
                           ('keepalive_ns', 13_000_000_001), ('receive_ns', 2_000_000_001),
                           ('consume_ns', 3_000_000), ('quiet_ns', 1_499_999_999),
                           ('teardown_ns', 0), ('body_ns', 30_000_000_001),
                           ('body_ns', 10_000_000_000), ('keepalive_ns', True)):
            value = values(); value[field] = bad
            with self.subTest(field=field, bad=bad), self.assertRaises(ValueError):
                self.verify(events(value))

    def test_frames_order_fields_duplicates_and_elapsed_remain_closed(self):
        sequence = events()
        for position in range(8):
            with self.subTest(position=position), self.assertRaises(ValueError):
                self.verify(sequence[:position] + sequence[position+1:])
        for value in (True, 0, 10, 30.001, float('inf')):
            sequence = events(); sequence[5]['Elapsed'] = value
            with self.assertRaises(ValueError): self.verify(sequence)
        raw = ordinary.encoded(events()).replace(b'"OutputType": "frame"', b'"OutputType": "frame", "OutputType": "frame"', 1)
        with self.assertRaises(ValueError): subject.formats.normalize(raw)

    def test_source_uses_preconstruction_pure_observation_and_real_engine_paths(self):
        code = (ROOT / 'upstream-tests/default_send_keepalive_test.go').read_text()
        for literal in ('key.receive.Open(nil,', 'entry.peer.keypairs.RLock()',
                        'entry.peer.keypairs.RUnlock()', 'logger.Verbosef = l.observe',
                        'f.d = NewDevice(f.tun, b, logger)', 'Receiving keepalive packet',
                        'p4KeepaliveConsumption(server.b, empty.wire, 2)',
                        'corrupt[len(corrupt)-1] ^= 1', 'sp.timers.newHandshake.IsPending',
                        'client.close(t, closeEnd)', 'server.close(t, closeEnd)'):
            self.assertIn(literal, code)
        self.assertLess(code.index('logger.Verbosef = l.observe'), code.index('f.d = NewDevice('))
        for forbidden in ('.ValidateCounter(', '.SendKeepalive(', '.Mod(', '.Del(',
                          'net.Listen', 'net.Dial', 'unsafe.', 'reflect.', '.created =',
                          'logger.Verbosef = l.observe // After'):
            self.assertNotIn(forbidden, code)

    @contextmanager
    def harness(self, failure=None):
        with patch.object(ordinary, 'runner', subject):
            with ordinary.OrdinaryRunnerControls().harness(failure) as state:
                fixture, _ = state
                commands = fixture[6]
                original = commands.side_effect
                def command(args, *rest):
                    if args[0].endswith('/test2json') and failure not in ('execute', 'execute_unknown'):
                        return 0, ordinary.encoded(events()), b''
                    return original(args, *rest)
                commands.side_effect = command
                yield state

    def test_mocked_closed_build_and_one_exact_keepalive_execute(self):
        with self.harness() as (state, _):
            paths, argv, _, _, _, saved, commands, _ = state
            argv[argv.index('reject')] = 'keepalive'
            subject.main()
            self.assertEqual(len(subject.inputs()), 11)
            self.assertEqual(saved[-1], 'build-receipt.json')
            build = json.loads((paths['artifacts'] / 'build-receipt.json').read_bytes())
            self.assertEqual((build['body_limit_seconds'], build['go_timeout_seconds'], build['supervisor_timeout_seconds']), (30, 40, 50))
            self.assertIn('-tags=p4_cookie_overlay,p4_default_residue_overlay,p4_default_keepalive_overlay', commands.call_args.args[0])
            argv[argv.index('build')] = 'execute'
            subject.main()
            self.assertEqual(saved[-1], 'execution-receipt.json')
            self.assertEqual(commands.call_args.args[0], [str(paths['artifacts'] / 'test2json'), '-p', subject.PACKAGE, '-t', str(paths['artifacts'] / 'device.test'), '-test.v=test2json', '-test.run=^TestP4DefaultSendKeepalive$', '-test.count=1', '-test.timeout=40s'])
            self.assertEqual(commands.call_args.args[3], 50)

    def test_mocked_failure_or_unknown_preserves_without_success_receipt(self):
        for failure in ('helper', 'compile', 'execute', 'execute_unknown'):
            with self.subTest(failure=failure), self.harness(failure) as (state, _):
                paths, argv, _, _, _, saved, _, _ = state
                argv[argv.index('reject')] = 'keepalive'
                if failure.startswith('execute'):
                    subject.main(); argv[argv.index('build')] = 'execute'
                with self.assertRaises((ValueError, subject.owned.Unsettled)): subject.main()
                self.assertNotIn('execution-receipt.json', saved)
                if failure in ('helper', 'compile'): self.assertNotIn('build-receipt.json', saved)
                if failure == 'execute_unknown': self.assertNotIn('execution-command-result.json', saved)

    def test_pins_keep_old_accepted_source_and_new_parser_overlay_exact(self):
        self.assertEqual(hashlib.sha256((ROOT / 'run_default_residue_ordinary.py').read_bytes()).hexdigest(), subject.BASE_RUNNER_SHA)
        self.assertEqual(hashlib.sha256((ROOT / 'default_keepalive_receipt.py').read_bytes()).hexdigest(), subject.RECEIPT_SHA)
        for name, (digest, _) in subject.OVERLAYS.items():
            self.assertEqual(hashlib.sha256((ROOT / 'upstream-tests' / name).read_bytes()).hexdigest(), digest)


if __name__ == '__main__':
    unittest.main()
