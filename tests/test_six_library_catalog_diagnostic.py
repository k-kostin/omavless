"""Finite source diagnostics only; synthetic iterator, never a guest/catalog read."""
import ast
import os
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

from tests.six_library_static_closure import probe


class Entries:
    def __init__(self, names=(), fault=None):
        self.items = iter(SimpleNamespace(name=name) for name in names)
        self.fault = fault
        self.next_calls = self.close_calls = 0

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.close_calls += 1
        return False

    def __next__(self):
        self.next_calls += 1
        if self.fault is not None:
            raise self.fault
        return next(self.items)


class CatalogDiagnosticTests(unittest.TestCase):
    def run_names(self, entries, *, scan_error=None, delay=None):
        obj, events, output = probe.Sources({}), probe.Events(), []
        with patch.object(probe, 'EVENTS', events), patch.object(probe, 'FINAL_SCOPE', obj), \
             patch.object(probe.os, 'write', side_effect=lambda fd, raw: (output.append(raw), len(raw))[1]), \
             patch.object(probe.os, 'scandir', side_effect=scan_error, return_value=entries) as scan:
            if delay is not None:
                original = events.catalog_before
                events.catalog_before = lambda phase: (original(phase), delay(phase, obj))[0]
            try:
                value = obj.names(7)
            except BaseException as error:
                events.failure(error)
                with self.assertRaises(RuntimeError):
                    obj.names(7)
                self.assertEqual(scan.call_count, 1 if events.catalog_phase != 'catalog_iterator_open' or delay is None else 0)
                return obj, events, b''.join(output), error
            return obj, events, b''.join(output), value

    def test_all_name_predicates_reject_at_exact_finite_subboundary(self):
        for names, phase in [([b'private'], 'catalog_name_type'),
                             (['private value'], 'catalog_name_shape'),
                             (['.'], 'catalog_name_shape'),
                             (['same', 'same'], 'catalog_name_duplicate'),
                             (['n'+str(i) for i in range(4097)], 'catalog_name_cap')]:
            obj, events, output, error = self.run_names(Entries(names))
            self.assertIsInstance(error, RuntimeError)
            self.assertEqual(obj.state, 'refused')
            self.assertEqual(events.catalog_phase, phase)
            self.assertIn(('T3_SIX_LIBRARY_STATIC_CATALOG_FAILED_AT_V1 '+phase+'\n').encode(), output)
            self.assertIn(b'T3_SIX_LIBRARY_STATIC_EXCEPTION_V1 RuntimeError\n', output)
            self.assertNotIn(b'private', output)

    def test_iterator_open_and_next_errors_emit_only_literal_exception_category(self):
        for phase in ('catalog_iterator_open', 'catalog_next_entry'):
            obj, events, output, error = self.run_names(
                Entries(fault=PermissionError('PRIVATE_DETAIL')),
                scan_error=PermissionError('PRIVATE_DETAIL') if phase == 'catalog_iterator_open' else None)
            self.assertIsInstance(error, PermissionError)
            self.assertEqual(obj.state, 'refused')
            self.assertEqual(events.catalog_phase, phase)
            self.assertNotIn(b'PRIVATE_DETAIL', output)
            self.assertIn(b'T3_SIX_LIBRARY_STATIC_EXCEPTION_V1 PermissionError\n', output)

    def test_valid_4096_catalog_preserves_original_cap_and_only_one_before_event(self):
        entries = Entries(['n'+str(i) for i in range(4096)])
        obj, events, output, value = self.run_names(entries)
        self.assertEqual(len(value), 4096)
        self.assertEqual(value, tuple(sorted(value)))
        self.assertEqual(events.count, 1)
        self.assertEqual(output, b'T3_SIX_LIBRARY_STATIC_BEFORE_V1 catalog_names\n')
        self.assertEqual(entries.next_calls, 4097)
        self.assertEqual(entries.close_calls, 1)
        self.assertEqual(obj.state, 'ready')

    def test_delayed_latch_prevents_next_original_iterator_operation(self):
        for phase in ('catalog_iterator_open', 'catalog_next_entry', 'catalog_entry_name'):
            obj, events = probe.Sources({}), probe.Events()
            entries = Entries(['name'])
            original = events.catalog_before
            def delayed(p):
                original(p)
                if p == phase:
                    obj.deadline = 0
            with patch.object(probe, 'EVENTS', events), patch.object(probe.os, 'write', side_effect=lambda fd,raw:len(raw)), \
                 patch.object(events, 'catalog_before', side_effect=delayed), \
                 patch.object(probe.os, 'scandir', return_value=entries) as scan:
                with self.assertRaises(RuntimeError):
                    obj.names(7)
            self.assertEqual(obj.state, 'refused')
            self.assertEqual(scan.call_count, int(phase != 'catalog_iterator_open'))
            self.assertEqual(entries.next_calls, int(phase == 'catalog_entry_name'))

    def test_exception_classes_exact_type_whitelist_never_class_name_or_message(self):
        class PRIVATE_TYPE(RuntimeError):
            pass
        for error in [cls('PRIVATE_DETAIL') for cls in probe.EXCEPTION_CLASSES] + [PRIVATE_TYPE('PRIVATE_DETAIL'), KeyboardInterrupt()]:
            events = probe.Events()
            events.last, events.catalog_phase = 'catalog_names', 'catalog_next_entry'
            with patch.object(probe, 'FINAL_SCOPE', None), patch.object(probe.os,'write',side_effect=lambda fd,raw:len(raw)) as output:
                events.failure(error)
                events.failure(error)
            output.assert_called_once()
            raw = output.call_args.args[1]
            expected = probe.EXCEPTION_CLASSES.get(type(error), 'OtherBaseException')
            self.assertTrue(raw.endswith(('T3_SIX_LIBRARY_STATIC_EXCEPTION_V1 '+expected+'\n').encode()))
            self.assertNotIn(b'PRIVATE', raw)
            self.assertLessEqual(len(raw), 256)

    def test_terminal_write_unknown_short_bool_float_throw_and_late_no_retry(self):
        for fault in ('short','bool','float','throw','late','expired','success'):
            obj, events = probe.Sources({}), probe.Events()
            events.last, events.catalog_phase = 'catalog_names', 'catalog_name_shape'
            def write(fd, raw):
                if fault == 'throw': raise OSError('PRIVATE')
                if fault == 'late': obj.deadline = 0
                return {'short':0,'bool':True,'float':float(len(raw))}.get(fault,len(raw))
            if fault == 'expired': obj.deadline = 0
            with patch.object(probe,'FINAL_SCOPE',obj), patch.object(probe.os,'write',side_effect=write) as output:
                if fault == 'success':
                    events.failure(RuntimeError('PRIVATE'))
                else:
                    with self.assertRaises((RuntimeError,OSError)):
                        events.failure(RuntimeError('PRIVATE'))
                count = output.call_count
                events.failure(RuntimeError('PRIVATE'))
                with self.assertRaises(RuntimeError): events.catalog_before('catalog_next_entry')
                self.assertEqual(output.call_count,count)
                self.assertEqual(count,int(fault != 'expired'))
            self.assertTrue(events.sealed and events.reported)

    def test_unknown_subboundary_and_non_catalog_error_not_promoted(self):
        events = probe.Events()
        with self.assertRaises(RuntimeError): events.catalog_before('PRIVATE_PHASE')
        events.last = 'package_desc'
        with patch.object(probe,'FINAL_SCOPE',None), patch.object(probe.os,'write',side_effect=lambda fd,raw:len(raw)) as output:
            events.failure(RuntimeError('PRIVATE'))
        self.assertEqual(output.call_args.args[1],b'T3_SIX_LIBRARY_STATIC_FAILED_AT_V1 package_desc\n')

    def test_fixed_exception_encoder_has_no_inspection_formatting_or_effects(self):
        tree = ast.parse(Path(probe.__file__).read_text())
        events = next(node for node in tree.body if isinstance(node,ast.ClassDef) and node.name == 'Events')
        failure = next(node for node in events.body if isinstance(node,ast.FunctionDef) and node.name == 'failure')
        calls = [node.func.attr if isinstance(node.func,ast.Attribute) else node.func.id if isinstance(node.func,ast.Name) else ''
                 for node in ast.walk(failure) if isinstance(node,ast.Call)]
        self.assertFalse(set(calls) & {'str','repr','open','stat','fstat','scandir','read','pread','exec','waitid','waitpid','kill'})


if __name__ == '__main__':
    unittest.main()
