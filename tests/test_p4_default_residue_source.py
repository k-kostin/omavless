"""Inert P4 residue/exhaustion source and finite receipt controls, no engine run."""
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

FIXTURE = Path(__file__).parent / 'fixtures/p4_awg_peer'
spec = importlib.util.spec_from_file_location('p4_residue_receipt', FIXTURE / 'default_residue_receipt.py')
subject = importlib.util.module_from_spec(spec)
spec.loader.exec_module(subject)


class ReceiptControls(unittest.TestCase):
    def values(self, case):
        values = {'case': case, 'network_fds': 'false'}
        values.update({field: 'true' for field in subject.TRUE[case]})
        values.update(subject.COUNTS[case])
        values.update(quiet_ns=1_600_000_000 if case == 'reject' else 5_600_000_000,
                      client_zero_age_ns=540_010_000_000, server_zero_age_ns=540_020_000_000,
                      client_removed_age_ns=540_030_000_000, server_removed_age_ns=540_040_000_000,
                      teardown_ns=20_000_000, body_ns=540_100_000_000)
        if case == 'reject':
            values.update(early_age_ns=177_100_000_000, late_age_ns=181_100_000_000)
        else:
            values.update(trigger_age_ns=181_100_000_000, exhaustion_ns=106_100_000_000,
                          retry_min_ns=5_010_000_000, retry_max_ns=5_332_000_000,
                          target_min_ns=5_000_000_000, target_max_ns=5_333_000_000)
        return values

    def output(self, case, values=None):
        values = self.values(case) if values is None else values
        return '    default_residue_cases_test.go:345: p4_residue_receipt ' + ' '.join(
            key + '=' + str(value) for key, value in values.items()) + '\n'

    def events(self, case):
        selected = subject.SELECTORS[case]
        return [{'Action': 'start', 'Package': subject.PACKAGE},
                {'Action': 'run', 'Package': subject.PACKAGE, 'Test': selected},
                {'Action': 'output', 'Package': subject.PACKAGE, 'Test': selected, 'Output': '=== RUN   ' + selected + '\n'},
                {'Action': 'output', 'Package': subject.PACKAGE, 'Test': selected, 'Output': self.output(case)},
                {'Action': 'output', 'Package': subject.PACKAGE, 'Test': selected, 'Output': '--- PASS: ' + selected + ' (540.10s)\n'},
                {'Action': 'pass', 'Package': subject.PACKAGE, 'Test': selected, 'Elapsed': 540.10},
                {'Action': 'output', 'Package': subject.PACKAGE, 'Output': 'PASS\n'},
                {'Action': 'pass', 'Package': subject.PACKAGE, 'Elapsed': 540.11}]

    def encoded(self, events):
        return b''.join(json.dumps(event).encode() + b'\n' for event in events)

    def test_one_exact_causal_synthetic_receipt_each_selector(self):
        for case in subject.SELECTORS:
            result = subject.verify_events(self.encoded(self.events(case)), case)
            self.assertEqual(result['case'], case)
            self.assertEqual(result['body_ns'], 540_100_000_000)

    def test_every_boolean_and_count_must_match_exactly(self):
        for case in subject.SELECTORS:
            for field in subject.TRUE[case] | set(subject.COUNTS[case]) | {'network_fds', 'case'}:
                values = self.values(case)
                values[field] = 21 if field in subject.COUNTS[case] else 'unknown'
                with self.subTest(case=case, field=field), self.assertRaisesRegex(ValueError, '^fixed_residue_receipt_refused$'):
                    subject.receipt(self.output(case, values), case)

    def test_numeric_types_noncanonical_and_magnitude_never_pass(self):
        for case in subject.SELECTORS:
            for field in subject.TIMES[case]:
                for value in (True, None, -1, 0, '01', 'nan', 'inf', '1.5', '1e9', '9' * 17):
                    values = self.values(case)
                    values[field] = value
                    with self.subTest(case=case, field=field, value=value), self.assertRaises(ValueError):
                        subject.receipt(self.output(case, values), case)

    def test_real_default_age_callback_then_independent_removal_bounds(self):
        mutations = {'client_zero_age_ns': 180_000_000_000,
                     'server_zero_age_ns': 1_080_000_000_000,
                     'client_removed_age_ns': 539_000_000_000,
                     'server_removed_age_ns': 551_000_000_000,
                     'body_ns': 539_000_000_000, 'teardown_ns': 5_000_000_001}
        for case in subject.SELECTORS:
            for field, bad in mutations.items():
                values = self.values(case)
                values[field] = bad
                with self.subTest(case=case, field=field), self.assertRaises(ValueError):
                    subject.receipt(self.output(case, values), case)

    def test_early_late_consume_and_full_quiet_window_bounds(self):
        for field, bad in (('early_age_ns', 179_000_000_000), ('early_age_ns', 176_999_999_999),
                           ('late_age_ns', 180_999_999_999), ('late_age_ns', 183_000_000_000),
                           ('quiet_ns', 1_499_999_999), ('quiet_ns', 3_000_000_001)):
            values = self.values('reject')
            values[field] = bad
            with self.subTest(field=field, bad=bad), self.assertRaises(ValueError):
                subject.receipt(self.output('reject', values), 'reject')

    def test_actual_default_retry_exhaustion_not_ninety_or_renewed_timer(self):
        for field, bad in (('exhaustion_ns', 90_000_000_000), ('exhaustion_ns', 130_000_000_001),
                           ('retry_min_ns', 4_999_999_999), ('retry_max_ns', 8_000_000_001),
                           ('target_max_ns', 5_334_000_000), ('quiet_ns', 5_499_999_999),
                           ('trigger_age_ns', 120_000_000_000), ('client_zero_age_ns', 820_000_000_000)):
            values = self.values('exhaust')
            values[field] = bad
            with self.subTest(field=field, bad=bad), self.assertRaises(ValueError):
                subject.receipt(self.output('exhaust', values), 'exhaust')

    def test_receipt_duplicate_unknown_missing_or_private_fields_refuse(self):
        for case in subject.SELECTORS:
            output = self.output(case)
            for candidate in (output.replace(' case=', ' private_path=secret case='),
                              output.replace(' baseline=true', ''),
                              output.replace(' baseline=true', ' baseline=true baseline=true'),
                              output.replace('default_residue_cases_test.go', 'private.go'),
                              output[:-1], output + 'private diagnostic\n'):
                with self.assertRaises(ValueError):
                    subject.receipt(candidate, case)

    def test_stream_missing_duplicate_skip_fail_extra_or_wrong_selector_refuses(self):
        for case in subject.SELECTORS:
            events = self.events(case)
            candidates = [[], events[:3] + events[4:], events[:-1], events + events,
                          events[:3] + [events[3]] + events[3:], events[1:] + [events[1]]]
            candidates.extend(events + [{'Action': action, 'Package': subject.PACKAGE}]
                              for action in ('skip', 'fail', 'unknown'))
            wrong = self.events('exhaust' if case == 'reject' else 'reject')
            candidates.append(wrong)
            for candidate in candidates:
                with self.assertRaises(ValueError):
                    subject.verify_events(self.encoded(candidate), case)

    def test_stream_finite_utf8_json_duplicate_keys_and_raw_diagnostics(self):
        case = 'reject'
        raw = self.encoded(self.events(case))
        for candidate in (raw[:-1], b'\xff\n', b'{"Action":"run","Action":"pass"}\n',
                          b'{}\n' * (subject.EVENT_LIMIT + 1), b'x' * (subject.OUTPUT_LIMIT + 1),
                          b'[' * 2000 + b']' * 2000 + b'\n'):
            with self.assertRaises(ValueError):
                subject.verify_events(candidate, case)
        for output in ('private raw exception\n', 'panic: timeout\n', 'PASS\n'):
            events = self.events(case)
            events[3]['Output'] = output
            with self.assertRaises(ValueError):
                subject.verify_events(self.encoded(events), case)

    def test_case_elapsed_includes_teardown_rounding_only_and_finite570_bound(self):
        for bad in (None, True, '540.1', 0, -1, 180, 540.08, 570.001, float('inf'), float('nan')):
            events = self.events('reject')
            events[5]['Elapsed'] = bad
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                subject.verify_events(self.encoded(events), 'reject')


class SourceControls(unittest.TestCase):
    def test_pinned_modular_support_and_supervisor_unchanged(self):
        for name, digest in (('upstream-tests/default_elapsed_rekey_test.go', '6722869be1b098603966eb0df564579789146a15c1340bc121e4ac96c5d2cb6f'),
                             ('run_default_rekey_supervisor.py', '00fa64cacdf72d65fcdd772208eca1bc4ca40ca954ef94444ff11eec65b729ec'),
                             ('run_cookie_overlay.py', '55fa8597b183a5b8deb0b0aabf25740ee12dfc221054e04e62ac017d1b86cc6e')):
            self.assertEqual(hashlib.sha256((FIXTURE / name).read_bytes()).hexdigest(), digest)

    def test_add_only_tagged_overlay_no_timer_age_or_worker_queue_substitution(self):
        sources = [(FIXTURE / 'upstream-tests' / name).read_text() for name in
                   ('default_residue_support_test.go', 'default_residue_cases_test.go')]
        for source in sources:
            self.assertTrue(source.startswith('//go:build p4_cookie_overlay && p4_default_residue_overlay\n'))
            for forbidden in ('expiredRetransmitHandshake(', 'expiredZeroKeyMaterial(', 'timersSessionDerived(',
                              'SendHandshakeInitiation(', '.handshakeAttempts.Store(', 'FlushStagedPackets(',
                              '.created =', '.zeroKeyMaterial.Mod(', '.retransmitHandshake.Mod(',
                              '.queue.handshake.c <-', 'net.Listen(', 'net.Dial(', 'os.Open(', 'os.Create('):
                self.assertNotIn(forbidden, source)
        support, cases = sources
        self.assertEqual(support.count('= NewDevice('), 1)
        self.assertIn('runtime.NumCPU() > 32', support)
        self.assertIn('p4ResiduePacketCap = 4096', support)
        self.assertIn('p4ResidueEventCap = 128', support)
        self.assertIn('len(b.held) >= 2', support)
        self.assertIn('f.closed = true', support)
        self.assertIn('case <-f.d.Wait():', support)
        self.assertIn('f.log.starts == f.log.stops', support)
        self.assertIn('len(f.client.log.events("retry")) != 19', cases)
        self.assertIn('f.cp.timers.handshakeAttempts.Load() != 19', cases)
        self.assertIn('f.ck.created, 539*time.Second', cases)
        self.assertIn('clientRemoved < clientAge', cases)
        for selector in subject.SELECTORS.values():
            self.assertEqual(cases.count('func ' + selector + '('), 1)


if __name__ == '__main__':
    unittest.main()
