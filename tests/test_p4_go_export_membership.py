"""Synthetic SOURCE regression only: no original FD/tree, Go or exporter main."""
import ast
import hashlib
import importlib.util
from pathlib import Path
from types import SimpleNamespace
import unittest

SOURCE=Path(__file__).parent/'research/p4-go-export/directory_membership.py'
spec=importlib.util.spec_from_file_location('p4_export_membership',SOURCE)
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)

class Controls(unittest.TestCase):
    def setUp(self):m.HELD.clear()
    def iterator(self,names,events):
        class Iterator:
            def __init__(self):self.values=iter(names)
            def __next__(self):return SimpleNamespace(name=next(self.values))
            def close(self):events.append('close')
        return Iterator()
    def adapter(self,names,events,result=0):
        iterator=self.iterator(names,events)
        def call(operation,*args):
            if operation is m.os.lseek:
                self.assertEqual(args,(123,0,m.os.SEEK_SET));events.append('seek');return result
            if operation is m.os.scandir:
                self.assertEqual(args,(123,));events.append('scan');return iterator
            return operation(*args)
        return SimpleNamespace(call=call)
    def test_reset_precedes_whole_membership_and_positive_close(self):
        for names in (['go'],['receipt.json','go']):
            events=[]
            m.root_members(self.adapter(names,events),123,set(names))
            self.assertEqual(events,['seek','scan','close'])
            self.assertEqual(len(m.HELD),1 if names==['go'] else 2)
    def test_wrong_seek_result_refuses_without_scan(self):
        for result in (None,False,True,1,-1,0.0,'0'):
            events=[]
            with self.assertRaises(m.Refused):m.root_members(self.adapter(['go'],events,result),123,{'go'})
            self.assertEqual(events,['seek']);self.assertEqual(m.HELD,[])
    def test_throw_or_sampled_late_seek_has_no_scan_retry_or_close(self):
        for late in (False,True):
            events=[]
            def call(operation,*args):
                self.assertIs(operation,m.os.lseek);events.append('seek')
                if late:raise m.Refused()
                raise OSError('synthetic adapter only')
            with self.assertRaises(m.Refused if late else OSError):
                m.root_members(SimpleNamespace(call=call),123,{'go'})
            self.assertEqual(events,['seek']);self.assertEqual(m.HELD,[])
    def test_membership_equality_and_uniqueness_stay_strict(self):
        for names in ([],['other'],['go','go'],['go','other']):
            events=[]
            with self.assertRaises(m.Refused):m.root_members(self.adapter(names,events),123,{'go'})
            self.assertEqual(events,['seek','scan'])
    def test_close_failure_has_no_second_scan_or_retry(self):
        events=[];store=self.adapter(['go'],events)
        original=store.call
        def call(operation,*args):
            if getattr(operation,'__name__',None)=='close':
                events.append('close');raise OSError('synthetic close refusal')
            return original(operation,*args)
        with self.assertRaises(OSError):m.root_members(SimpleNamespace(call=call),123,{'go'})
        self.assertEqual(events,['seek','scan','close'])
    def test_model_per_open_index_refresh_not_kernel_attestation(self):
        state={'last_index':0,'entries':['go']};events=[]
        def call(operation,*args):
            if operation is m.os.lseek:
                self.assertEqual(args,(123,0,m.os.SEEK_SET));state['last_index']=1;events.append('seek');return 0
            if operation is m.os.scandir:
                events.append('scan');return self.iterator(state['entries'][:state['last_index']],events)
            return operation(*args)
        self.assertEqual(state['entries'][:state['last_index']],[])
        m.root_members(SimpleNamespace(call=call),123,{'go'})
        self.assertEqual(events,['seek','scan','close'])
    def test_no_real_operations_entry_point_or_ambient_path(self):
        source=SOURCE.read_text();tree=ast.parse(source)
        body=source[source.index('def root_members('):]
        self.assertEqual(hashlib.sha256(body.encode()).hexdigest(),
            '92d87c721d539734bd0f806f188ef8ca0e29fc4dc7b0103cc98572c37c4cd940')
        self.assertEqual([n.module if isinstance(n,ast.ImportFrom) else n.names[0].name
            for n in tree.body if isinstance(n,(ast.Import,ast.ImportFrom))],['os'])
        self.assertNotIn('__main__',SOURCE.read_text())
        self.assertNotIn('/home/',SOURCE.read_text())
        function=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='root_members')
        direct=[n.func.attr for n in ast.walk(function) if isinstance(n,ast.Call)
            and isinstance(n.func,ast.Attribute) and isinstance(n.func.value,ast.Name)
            and n.func.value.id=='os']
        self.assertEqual(direct,[])

if __name__=='__main__':unittest.main()
