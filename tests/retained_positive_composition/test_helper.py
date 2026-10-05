"""Inert fixed pipe protocol controls; no helper, process or actual fixture."""
import ast
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

def module(name):
    spec=importlib.util.spec_from_file_location('private_'+name,Path(__file__).with_name(name+'.py'))
    value=importlib.util.module_from_spec(spec);spec.loader.exec_module(value);return value

h=module('helper');protocol=module('controller')
class Session:pass
class Child:pass
class Images:pass


class Controls(unittest.TestCase):
    def setUp(self):self.streams=[];self.fds=[]
    def tearDown(self):
        for stream in self.streams:
            if not stream.closed:stream.close()
        for fd in self.fds:os.close(fd)

    def fixture(self,started=True):
        owner=Session();owner.kind='inner';owner.sealed=False;owner.retained=[]
        owner.available=Mock();owner.within=Mock();owner.live=Mock();owner.local_deadline=Mock(return_value=5.0)
        child=Child();child.pid=17;child.returncode=None
        r,w=os.pipe();self.fds.append(r);child.stdin=io.FileIO(w,'wb');self.streams.append(child.stdin)
        r,w=os.pipe();self.fds.append(w);child.stdout=io.FileIO(r,'rb');self.streams.append(child.stdout)
        owner.roles={id(child):'host'};owner.anchors={'host':{'child':child,'state':'spawned'}};owner.zero_reaped={}
        images=Images();images.owner=owner;images.executable=Mock()
        ownership=SimpleNamespace(Session=Session,OwnedProcess=Child,clock=Mock(return_value=0.0))
        actual=os.fstat
        def metadata(fd):
            value=actual(fd)
            return SimpleNamespace(st_dev=value.st_dev,st_ino=value.st_ino,st_mode=stat.S_IFIFO|0o600,
                                   st_uid=0,st_gid=0,st_nlink=1)
        with patch.object(h.os,'fstat',side_effect=metadata):
            value=h.Helper(owner,child,ownership,images,SimpleNamespace(Images=Images),protocol)
        value.started=started
        return value,metadata

    def reply(self,final=False,**overrides):
        value={'notify_alive':True,'observer_finalized':final,'monitor_alive':not final};value.update(overrides)
        return json.dumps(value).encode()+b'\n'

    def pipes(self,value,metadata,raw):
        return patch.object(h.os,'fstat',side_effect=metadata), \
               patch.object(h.select,'select',return_value=([value.output_fd],[],[])), \
               patch.object(h.os,'read',return_value=raw)

    def zeros(self,value):
        for index,name in enumerate(('core','broker'),18):
            child=Child();child.pid=index;child.returncode=0
            value.owner.anchors[name]={'child':child,'state':'zero-reaped'}
            value.owner.zero_reaped[id(child)]=(index,0)

    def test_original_unbuffered_pipe_constructor_and_literal_ready(self):
        value,metadata=self.fixture(False);fstat,ready,read=self.pipes(value,metadata,b'fixture-ready\n')
        with fstat,ready,read,patch.object(h.os,'write') as write:
            value.ready(5.0)
        self.assertTrue(value.started);write.assert_not_called();self.assertIn(value,value.owner.retained)
        self.assertEqual(value.images.executable.call_count,2)

    def test_every_single_write_has_live_image_and_pipe_check_immediately_before(self):
        value,metadata=self.fixture();events=[]
        value.images.executable.side_effect=lambda *args:events.append('image')
        def written(fd,raw):
            self.assertEqual(events[-1],'image');events.append('write')
            self.assertEqual((fd,raw),(value.input_fd,b'snapshot\n'));return len(raw)
        fstat,ready,read=self.pipes(value,metadata,self.reply())
        with fstat,ready,read,patch.object(h.os,'write',side_effect=written) as write:
            value.snapshot(5.0)
        self.assertEqual(write.call_count,1);self.assertEqual(value.requests,1);self.assertFalse(value.sealed)

    def test_short_wrong_type_throw_or_late_write_never_reads_retries_or_closes(self):
        for variant in ('short','bool','float','throw','late'):
            value,metadata=self.fixture()
            def written(fd,raw):
                if variant=='throw':raise OSError('private')
                if variant=='late':value.ownership.clock.return_value=5.0
                return {'short':len(raw)-1,'bool':True,'float':float(len(raw))}.get(variant,len(raw))
            with patch.object(h.os,'fstat',side_effect=metadata),patch.object(h.os,'write',side_effect=written) as write, \
                 patch.object(h.os,'read') as read:
                with self.assertRaisesRegex(h.Refused,'^fixed_manager_helper_refused$'):value.snapshot(5.0)
                value.ownership.clock.return_value=0.0
                with self.assertRaises(h.Refused):value.snapshot(5.0)
                self.assertEqual(write.call_count,1);read.assert_not_called()
            self.assertTrue(value.sealed and value.owner.sealed);self.assertFalse(value.input.closed)

    def test_finish_requires_two_independent_exact_normal_zero_ledgers_before_write(self):
        for variant in ('missing','status_bool','pid_float','return_bool','phase','nonzero'):
            value,metadata=self.fixture();self.zeros(value)
            child=value.owner.anchors['broker']['child']
            if variant=='missing':value.owner.zero_reaped.pop(id(child))
            if variant=='status_bool':value.owner.zero_reaped[id(child)]=(child.pid,False)
            if variant=='pid_float':value.owner.zero_reaped[id(child)]=(float(child.pid),0)
            if variant=='return_bool':child.returncode=False
            if variant=='phase':value.owner.anchors['broker']['state']='mapped'
            if variant=='nonzero':value.owner.zero_reaped[id(child)]=(child.pid,256)
            with patch.object(h.os,'write') as write:
                with self.assertRaises(h.Refused):value.snapshot(5.0,final=True)
                write.assert_not_called()
            self.assertTrue(value.sealed and value.owner.sealed)

    def test_known_finish_allows_single_positive_eof_no_followup_request(self):
        value,metadata=self.fixture();self.zeros(value)
        fstat,ready,read=self.pipes(value,metadata,self.reply(True))
        with fstat,ready,read,patch.object(h.os,'write',side_effect=lambda fd,raw:len(raw)) as write:
            value.snapshot(5.0,final=True);value.positive_eof(5.0)
            self.assertTrue(value.input.closed and value.eof)
            with self.assertRaises(h.Refused):value.snapshot(5.0)
            self.assertEqual(write.call_count,1)
        self.assertTrue(value.owner.sealed)

    def test_bad_helper_receipt_duplicate_unsolicited_or_oversized_line_is_terminal(self):
        for raw in (self.reply(notify_alive=False),self.reply(observer_finalized=1),
                    b'{"notify_alive":true,"notify_alive":true}\n',b'{}\nunsolicited\n',
                    b'x'*(h.MAX_LINE+1),b'',self.reply()+b'\n'):
            value,metadata=self.fixture();fstat,ready,read=self.pipes(value,metadata,raw)
            with fstat,ready,read,patch.object(h.os,'write',side_effect=lambda fd,raw:len(raw)) as write:
                with self.assertRaises(h.Refused):value.snapshot(5.0)
                with self.assertRaises(h.Refused):value.snapshot(5.0)
                self.assertEqual(write.call_count,1)
            self.assertTrue(value.sealed and value.owner.sealed);self.assertFalse(value.input.closed)

    def test_premature_or_late_positive_eof_never_authorizes_followup_wait(self):
        value,metadata=self.fixture()
        with self.assertRaises(h.Refused):value.positive_eof(5.0)
        self.assertFalse(value.input.closed);self.assertTrue(value.owner.sealed)
        value,metadata=self.fixture();value.finished=True
        original=value.call
        def called(deadline,operation,*args,**kwargs):
            if operation==value.input.close:
                def closed():
                    result=operation();value.ownership.clock.return_value=5.0;return result
                return original(deadline,closed)
            return original(deadline,operation,*args,**kwargs)
        with patch.object(h.os,'fstat',side_effect=metadata),patch.object(value,'call',side_effect=called):
            with self.assertRaises(h.Refused):value.positive_eof(5.0)
        self.assertTrue(value.input.closed and value.owner.sealed);self.assertFalse(value.eof)
        value.ownership.clock.return_value=0.0
        with patch.object(value.owner,'live') as query:
            with self.assertRaises(h.Refused):value.check(5.0)
            query.assert_not_called()

    def test_deadline_nonfinite_clock_request_cap_or_unstarted_refuses_before_write(self):
        for variant in ('clock','deadline','cap','unstarted','final_type'):
            value,metadata=self.fixture()
            if variant=='clock':value.ownership.clock.return_value=float('inf')
            if variant=='cap':value.requests=h.MAX_REQUESTS
            if variant=='unstarted':value.started=False
            with patch.object(h.os,'write') as write:
                with self.assertRaises(h.Refused):value.snapshot(True if variant=='deadline' else 5.0,
                                                                final=1 if variant=='final_type' else False)
                write.assert_not_called()
            self.assertTrue(value.sealed and value.owner.sealed)

    def test_changed_original_pipe_or_lost_helper_precedes_write(self):
        for variant in ('inode','live','image'):
            value,metadata=self.fixture()
            if variant=='live':value.owner.live.side_effect=RuntimeError('private')
            if variant=='image':value.images.executable.side_effect=RuntimeError('private')
            def changed(fd):
                info=metadata(fd)
                if variant=='inode':info.st_ino+=1
                return info
            with patch.object(h.os,'fstat',side_effect=changed),patch.object(h.os,'write') as write:
                with self.assertRaises(h.Refused):value.snapshot(5.0)
                write.assert_not_called()
            self.assertTrue(value.sealed and value.owner.sealed)

    def test_no_candidate_execution_query_signal_reap_or_failure_cleanup_surface(self):
        forbidden={'exec','eval','spawn','kill','waitid','waitpid','system','unlink','rmdir','mkdir','open','flush'}
        for node in ast.walk(ast.parse(Path(__file__).with_name('helper.py').read_text())):
            if isinstance(node,ast.Call):
                name=node.func.id if isinstance(node.func,ast.Name) else node.func.attr if isinstance(node.func,ast.Attribute) else ''
                self.assertNotIn(name,forbidden)


if __name__=='__main__':unittest.main()
