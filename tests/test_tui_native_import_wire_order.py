"""Bounded ordering controls for two real client's independent RPC workers."""
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('native_wire', ROOT / 'tests/test-tui-native-import-wire.py')
WIRE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(WIRE)
READ = WIRE.METHODS[:4]
CLASSIFY = ['system.hello', 'imports.classify', 'system.hello']
SAVE = CLASSIFY + ['system.hello', 'plugin.action']


def interleavings(left, right):
    if not left or not right:
        yield left + right
        return
    for suffix in interleavings(left[1:], right):
        yield left[:1] + suffix
    for suffix in interleavings(left, right[1:]):
        yield right[:1] + suffix


class NativeWireOrderingTests(unittest.TestCase):
    def test_all_bounded_per_worker_interleavings_preserve_guards_and_cycles(self):
        count = 0
        for calls in interleavings(SAVE, READ * 2):
            WIRE.require_import_trace(READ + calls, SAVE)
            count += 1
        self.assertEqual(count, 1287)

    def test_original_global_adjacency_refuses_a_valid_interleaving(self):
        calls = READ + CLASSIFY + ['system.hello', 'capabilities.get',
                                  'system.hello', 'ui.snapshot',
                                  'runtime.observation', 'plugin.action']
        with self.assertRaisesRegex(AssertionError, 'Save fixed pre-hello'):
            WIRE.require(calls[-2] == 'system.hello', 'Save fixed pre-hello')
        WIRE.require_import_trace(calls, SAVE)

    def test_missing_foreground_calls_extra_hellos_and_resend_still_refuse(self):
        for index in range(len(SAVE)):
            with self.subTest(missing=index), self.assertRaises(AssertionError):
                WIRE.require_import_trace(READ + SAVE[:index] + SAVE[index + 1:], SAVE)
        for suffix in (['system.hello'], ['plugin.action'], ['connection.connect']):
            with self.subTest(extra=suffix), self.assertRaises(AssertionError):
                WIRE.require_import_trace(READ + SAVE + suffix, SAVE)

    def test_reordered_reads_and_foreground_do_not_pass_as_background_noise(self):
        for calls in (READ[:1] + READ[2:3] + READ[1:2] + READ[3:] + SAVE,
                      READ + ['system.hello', 'plugin.action'] + CLASSIFY,
                      READ + CLASSIFY + ['plugin.action', 'system.hello']):
            with self.subTest(calls=calls), self.assertRaises(AssertionError):
                WIRE.require_import_trace(calls, SAVE)

    def test_partial_observation_relaxes_only_background_completion(self):
        WIRE.require_import_trace(READ + SAVE + READ[:2], SAVE, partial_read=True)
        with self.assertRaises(AssertionError):
            WIRE.require_import_trace(READ + SAVE + READ[:2], SAVE)
        with self.assertRaises(AssertionError):
            WIRE.require_import_trace(READ + CLASSIFY, SAVE, partial_read=True)


if __name__ == '__main__':
    unittest.main()
