"""Pure fixed typed-property controls; no system bus or VM calls."""
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import Mock, patch, call

ROOT = Path(__file__).resolve().parents[1]
SUPPORT = ROOT / 'crates/omavless-netguard/tests/support'
spec = importlib.util.spec_from_file_location('typed_manager', SUPPORT / 'typed_manager_properties.py')
typed = importlib.util.module_from_spec(spec)
spec.loader.exec_module(typed)


class TypedProperties(unittest.TestCase):
    def original(self):
        return typed.load_original((SUPPORT / 'namespace_filter_guest_guard.py').read_bytes())

    def test_only_definitions_and_exact_immutable_source_are_loaded(self):
        original = self.original()
        self.assertEqual(original['__name__'], 'fixed_property_query_not_main')
        self.assertEqual(original['STAGE'], typed.STAGE)
        self.assertFalse(original['UNCERTAIN'])
        with self.assertRaises(RuntimeError):
            typed.load_original((SUPPORT / 'namespace_filter_guest_guard.py').read_bytes() + b'\n')

    def test_complete_fixed_readonly_argv_and_exact_typed_empty_reply(self):
        expected = ['/usr/bin/busctl', '--system', 'get-property', 'org.freedesktop.systemd1',
                    '/org/freedesktop/systemd1/unit/omavless_2dk1_2dtyped_2dorder_2dfilter_2dfixture_2eservice',
                    'org.freedesktop.systemd1.Service', 'ExecCondition', 'ExecStartPre',
                    'ExecStartPost', 'ExecReload', 'ExecStop', 'ExecStopPost',
                    'EnvironmentFiles', 'SystemCallFilter']
        requires_argv = [*expected[:5], 'org.freedesktop.systemd1.Unit', 'Requires']
        for dependencies in typed.REQUIRES_EXPECTED:
            command = Mock(side_effect=[typed.EXPECTED, dependencies])
            typed.fixed_query({'UNCERTAIN': False, 'command': command})
            self.assertEqual(command.call_args_list, [call(expected), call(requires_argv)])
        self.assertEqual(typed.EXPECTED, b'a(sasbttttuii) 0\n'*6 + b'a(sb) 0\n(bas) false 0\n')

    def test_missing_wrong_type_count_polarity_extra_tokens_or_bytes_refuse(self):
        for data in (b'', typed.EXPECTED[:-1], typed.EXPECTED + b'\n',
                     typed.EXPECTED.replace(b'a(sb)', b'as'),
                     typed.EXPECTED.replace(b'false', b'true'),
                     typed.EXPECTED.replace(b'a(sb) 0', b'a(sb) 1'),
                     typed.EXPECTED.replace(b'a(sasbttttuii) 0\n', b'', 1),
                     typed.EXPECTED + b'PRIVATE_SENTINEL'):
            with self.subTest(data=data), self.assertRaises(RuntimeError):
                typed.fixed_query({'UNCERTAIN': False, 'command': Mock(return_value=data)})

    def test_existing_or_new_uncertainty_prevents_acceptance_and_retry(self):
        command = Mock(return_value=typed.EXPECTED)
        with self.assertRaises(RuntimeError):
            typed.fixed_query({'UNCERTAIN': True, 'command': command})
        command.assert_not_called()
        original = self.original()
        later = Mock()
        child = Mock(pid=123)
        with patch.dict(original, {'OwnedProcess': Mock(return_value=child)}), \
             patch.object(original['os'], 'waitid', side_effect=ChildProcessError()) as wait, \
             patch.object(original['os'], 'waitpid') as reap:
            # Direct real await function closes over the original module dict.
            with self.assertRaises(original['Refused']):
                original['await_child'](child)
            self.assertTrue(original['UNCERTAIN'])
            original['command'] = later
            with self.assertRaises(RuntimeError):
                typed.fixed_query(original)
            wait.assert_called_once()
            reap.assert_not_called()
            later.assert_not_called()

    def test_command_failure_is_not_a_typed_empty_reply(self):
        with self.assertRaises(OSError):
            typed.fixed_query({'UNCERTAIN': False, 'command': Mock(side_effect=OSError())})

    def test_requires_rejects_missing_duplicate_extra_or_malformed_dependency(self):
        for value in (b'', b'as 0\n', b'as 1 "sysinit.target"\n',
                      b'as 2 "system.slice" "system.slice"\n',
                      b'as 2 "system.slice" "unknown.target"\n',
                      b'as 3 "sysinit.target" "system.slice" "extra.service"\n',
                      b'as 2 "sysinit.target" "system.slice"',
                      b'as 2 "sysinit.target" "system.slice"\n\n',
                      b'a(sb) 2 "sysinit.target" "system.slice"\n'):
            with self.subTest(value=value), self.assertRaises(RuntimeError):
                typed.fixed_query({'UNCERTAIN': False,
                                   'command': Mock(side_effect=[typed.EXPECTED, value])})

    def test_first_query_failure_never_queries_requires_and_second_uncertainty_refuses(self):
        command = Mock(return_value=b'')
        with self.assertRaises(RuntimeError):
            typed.fixed_query({'UNCERTAIN': False, 'command': command})
        command.assert_called_once_with(typed.ARGV)
        namespace = {'UNCERTAIN': False}
        def query(args):
            if args == typed.ARGV: return typed.EXPECTED
            namespace['UNCERTAIN'] = True
            return next(iter(typed.REQUIRES_EXPECTED))
        namespace['command'] = query
        with self.assertRaises(RuntimeError): typed.fixed_query(namespace)


if __name__ == '__main__':
    unittest.main()
