"""Inert literal stream/worker controls. No sockets, threads or children start."""
import ast
import importlib.util
import os
from pathlib import Path
import stat
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

SOURCE=Path(__file__).with_name('streams.py')
spec=importlib.util.spec_from_file_location('retained_streams',SOURCE)
s=importlib.util.module_from_spec(spec);spec.loader.exec_module(s)

class Session:pass
class Child:pass
class Images:pass
class Controller:pass


class Controls(unittest.TestCase):
    def fixture(self):
        value=s.Streams.__new__(s.Streams);owner=Session();core=Child();clock=[0.0]
        owner.kind='inner';owner.isolated=True;owner.sealed=False;owner.retained=[]
        owner.anchors={'core':{'child':core,'state':'mapped'}};owner.live=Mock();owner.available=Mock()
        def within(deadline):
            if clock[0]>=deadline:raise RuntimeError('private deadline')
        owner.within=Mock(side_effect=within);owner.local_deadline=Mock(return_value=8.0)
        images=Images();images.owner=owner;images.executable=Mock()
        control=Controller();control.owner=owner;control.core=core;control.sealed=False
        value.owner,value.core,value.images,value.control=owner,core,images,control
        value.ownership=SimpleNamespace(Session=Session,OwnedProcess=Child,clock=Mock(side_effect=lambda:clock[0]))
        value.deadline=8.0;value.sealed=False
        value.failed=value.stopping=value.worker_done=value.witness_done=value.finished=False
        value.exact_ack=value.selected_eof_verified=False
        value.held=[];value.identities={};value.clients=[];value.peers=[];value.accepted=[];value.peer_eof=set();value.transferred={}
        value.worker_thread=Mock();value.worker_thread.is_alive.return_value=True
        value.worker_thread.join.return_value=None
        return value,clock

    def sock(self,number=71):
        value=Mock();value.fileno.return_value=number
        value.getsockname.return_value=('127.0.0.1',19092)
        value.getpeername.return_value=s.PROXY
        value.send.side_effect=lambda raw:len(raw);value.close.return_value=None
        return value

    def metadata(self,fd):
        return SimpleNamespace(st_dev=1,st_ino=fd,st_mode=stat.S_IFSOCK|0o777,st_uid=0,st_gid=0,st_nlink=1)

    def hold(self,value,stream):
        with patch.object(s.os,'fstat',side_effect=self.metadata):value.retained(stream)
        return stream

    def test_constructor_requires_exact_isolated_core_session_before_socket(self):
        value,clock=self.fixture()
        with patch.object(s.socket,'socket') as created:
            with self.assertRaises(s.Refused):s.Streams(SimpleNamespace(sealed=False),value.ownership,
                value.images,SimpleNamespace(Images=Images),value.control,SimpleNamespace(Controller=Controller))
            created.assert_not_called()

    def test_constructor_creates_only_fixed_listener_and_literal_worker_without_running(self):
        value,clock=self.fixture();listener=self.sock();thread=Mock();thread.is_alive.return_value=True
        with patch.object(s.os,'getpid',return_value=1),patch.object(s.os,'geteuid',return_value=0), \
             patch.object(s.os,'getegid',return_value=0),patch.object(s.os,'fstat',side_effect=self.metadata), \
             patch.object(s.socket,'socket',return_value=listener) as created, \
             patch.object(s.threading,'Thread',return_value=thread) as threaded:
            actual=s.Streams(value.owner,value.ownership,value.images,SimpleNamespace(Images=Images),
                             value.control,SimpleNamespace(Controller=Controller))
        created.assert_called_once_with(s.socket.AF_INET,s.socket.SOCK_STREAM)
        listener.bind.assert_called_once_with(s.ECHO);listener.listen.assert_called_once_with(2)
        self.assertEqual(threaded.call_args.kwargs['target'],actual.worker)
        thread.start.assert_called_once();thread.join.assert_not_called();listener.close.assert_not_called()

    def test_late_created_socket_retained_before_metadata_no_second_io(self):
        value,clock=self.fixture();stream=self.sock()
        def created(*args):clock[0]=8.0;return stream
        with patch.object(s.socket,'socket',side_effect=created) as op,patch.object(s.os,'fstat') as metadata:
            with self.assertRaises(s.Refused):value.created()
            clock[0]=0.0
            with self.assertRaises(s.Refused):value.created()
            op.assert_called_once();metadata.assert_not_called()
        self.assertEqual(value.held,[stream]);stream.close.assert_not_called();self.assertTrue(value.owner.sealed)

    def test_every_full_single_write_rechecks_socket_core_and_image_before_send(self):
        value,clock=self.fixture();stream=self.hold(value,self.sock());events=[]
        value.images.executable.side_effect=lambda *args:events.append('image')
        def sent(raw):self.assertEqual(events[-1],'image');events.append('send');return len(raw)
        stream.send.side_effect=sent
        with patch.object(s.os,'fstat',side_effect=self.metadata):value.write(stream,s.PAYLOAD)
        self.assertEqual(stream.send.call_count,1);value.owner.live.assert_called_once_with(value.core)
        stream.close.assert_not_called();self.assertFalse(value.sealed)

    def test_short_alias_throw_or_late_write_no_retry_close_or_read(self):
        for variant in ('short','bool','float','throw','late'):
            value,clock=self.fixture();stream=self.hold(value,self.sock())
            def sent(raw):
                if variant=='throw':raise OSError('private')
                if variant=='late':clock[0]=8.0
                return {'short':len(raw)-1,'bool':True,'float':float(len(raw))}.get(variant,len(raw))
            stream.send.side_effect=sent
            with patch.object(s.os,'fstat',side_effect=self.metadata):
                with self.assertRaises(s.Refused):value.write(stream,s.PAYLOAD)
                clock[0]=0.0
                with self.assertRaises(s.Refused):value.write(stream,s.PAYLOAD)
            self.assertEqual(stream.send.call_count,1);stream.recv.assert_not_called();stream.close.assert_not_called()
            self.assertTrue(value.sealed and value.owner.sealed)

    def test_changed_original_socket_core_or_image_precedes_write(self):
        for variant in ('fd','metadata','core','live','image'):
            value,clock=self.fixture();stream=self.hold(value,self.sock())
            if variant=='fd':stream.fileno.return_value=72
            if variant=='core':value.control.core=Child()
            if variant=='live':value.owner.live.side_effect=RuntimeError('private')
            if variant=='image':value.images.executable.side_effect=RuntimeError('private')
            def metadata(fd):
                actual=self.metadata(fd)
                if variant=='metadata':actual.st_ino+=1
                return actual
            with patch.object(s.os,'fstat',side_effect=metadata):
                with self.assertRaises(s.Refused):value.write(stream,s.PAYLOAD)
            stream.send.assert_not_called();stream.close.assert_not_called();self.assertTrue(value.owner.sealed)

    def test_tunnel_uses_only_literal_connect_and_strict_bounded_header(self):
        for variant in ('known','bad_status','eof','oversize','wrong_peer'):
            value,clock=self.fixture();stream=self.sock();stream.getsockname.return_value=('127.0.0.1',42000)
            raw=b'HTTP/1.1 200 Connection established\r\n\r\n'
            if variant=='bad_status':raw=b'HTTP/1.1 500 Refused\r\n\r\n'
            if variant=='eof':raw=b''
            if variant=='oversize':raw=b'x'*4097
            if variant=='wrong_peer':stream.getpeername.return_value=('192.0.2.1',19090)
            stream.recv.side_effect=[bytes([byte]) for byte in raw]+[b'']
            with patch.object(s.socket,'socket',return_value=stream),patch.object(s.os,'fstat',side_effect=self.metadata):
                if variant=='known':self.assertIs(value.tunnel(),stream)
                else:
                    with self.assertRaises(s.Refused):value.tunnel()
            stream.connect.assert_called_once_with(s.PROXY);stream.close.assert_not_called()
            if variant!='wrong_peer':stream.send.assert_called_once_with(s.CONNECT)

    def test_wrong_echo_bytes_or_eof_are_terminal_no_resend(self):
        for raw in (b'wrong',b''):
            value,clock=self.fixture();stream=self.hold(value,self.sock());value.clients=[stream]
            stream.recv.return_value=raw
            with patch.object(s.os,'fstat',side_effect=self.metadata):
                with self.assertRaises(s.Refused):value.echo(stream)
                with self.assertRaises(s.Refused):value.echo(stream)
            stream.send.assert_called_once_with(s.PAYLOAD);stream.close.assert_not_called()

    def test_witness_order_exact_replies_survivor_identity_and_no_public_ids(self):
        value,clock=self.fixture();first,second=self.sock(71),self.sock(72)
        first.getsockname.return_value=('127.0.0.1',42001);second.getsockname.return_value=('127.0.0.1',42002)
        target,other,observed=object(),object(),object();events=[]
        def tunnel():
            stream=(first,second)[len(value.clients)];value.clients.append(stream);return stream
        value.tunnel=Mock(side_effect=tunnel);value.echo=Mock(side_effect=lambda stream:events.append(('echo',stream)))
        value.selected_eof=Mock(side_effect=lambda stream:events.append(('eof',stream)))
        value.control.discover_for_ports=Mock(side_effect=[[target,other],[observed]])
        def closed(selected,**kwargs):
            self.assertIs(selected,target);events.append(('close',kwargs))
            return 'changed' if kwargs else 'closed' if len([x for x in events if x[0]=='close'])==2 else 'missing'
        value.control.close_witness=Mock(side_effect=closed);value.control.same_private_target=Mock(return_value=True)
        receipt=value.witness()
        self.assertEqual([item[0] for item in events],['echo','echo','close','echo','echo','close','eof','echo','close'])
        self.assertTrue(all(type(flag) is bool and flag for flag in receipt.values()))
        value.control.same_private_target.assert_called_once_with(other,observed)
        self.assertTrue(value.witness_done);first.close.assert_not_called();second.close.assert_not_called()

    def test_wrong_close_or_survivor_result_seals_before_any_positive_finish(self):
        for variant in ('wrong','exact','replay','survivor'):
            value,clock=self.fixture();first,second=self.sock(71),self.sock(72)
            first.getsockname.return_value=('127.0.0.1',42001);second.getsockname.return_value=('127.0.0.1',42002)
            value.tunnel=Mock(side_effect=[first,second]);value.echo=Mock();value.selected_eof=Mock()
            value.control.discover_for_ports=Mock(side_effect=[[object(),object()],[object()]])
            replies=['changed','closed','missing'];index={'wrong':0,'exact':1,'replay':2}.get(variant)
            if index is not None:replies[index]='wrong'
            value.control.close_witness=Mock(side_effect=replies)
            value.control.same_private_target=Mock(return_value=variant!='survivor')
            with self.assertRaises(s.Refused):value.witness()
            with self.assertRaises(s.Refused):value.finish_positive()
            value.worker_thread.join.assert_not_called();first.close.assert_not_called();second.close.assert_not_called()
            self.assertFalse(value.witness_done)

    def test_selected_eof_or_reset_only_post_ack_and_late_never_accepts(self):
        for variant in ('eof','reset','live','late'):
            value,clock=self.fixture();stream=self.hold(value,self.sock())
            value.clients=[stream,self.sock(72)];value.exact_ack=True
            def received(maximum):
                if variant=='late':clock[0]=8.0
                if variant=='reset':raise ConnectionResetError()
                return b'x' if variant=='live' else b''
            stream.recv.side_effect=received
            with patch.object(s.os,'fstat',side_effect=self.metadata):
                if variant in ('eof','reset'):value.selected_eof(stream)
                else:
                    with self.assertRaises(s.Refused):value.selected_eof(stream)
            stream.close.assert_not_called()

    def test_eof_without_exact_ack_or_wrong_selected_socket_never_reads(self):
        for variant in ('no_ack','wrong_socket','already_seen'):
            value,clock=self.fixture();first=self.hold(value,self.sock());second=self.hold(value,self.sock(72))
            value.clients=[first,second];value.exact_ack=variant!='no_ack'
            value.selected_eof_verified=variant=='already_seen'
            with self.assertRaises(s.Refused):value.selected_eof(second if variant=='wrong_socket' else first)
            first.recv.assert_not_called();second.recv.assert_not_called();self.assertTrue(value.owner.sealed)

    def test_worker_first_bad_event_preserves_all_sockets_no_retry_or_cleanup(self):
        for variant in ('foreign','throw','late','duplicate'):
            value,clock=self.fixture();value.listener=self.hold(value,self.sock())
            def selected(*args):
                if variant=='throw':raise RuntimeError('private')
                if variant=='late':clock[0]=8.0
                return ([object()] if variant=='foreign' else [value.listener,value.listener] if variant=='duplicate' else [],[],[])
            with patch.object(s.select,'select',side_effect=selected) as op:value.worker()
            op.assert_called_once();self.assertTrue(value.failed and value.owner.sealed)
            value.listener.accept.assert_not_called();value.listener.close.assert_not_called();self.assertFalse(value.worker_done)

    def test_worker_late_accept_retains_peer_no_metadata_read_or_close(self):
        value,clock=self.fixture();value.listener=self.hold(value,self.sock());peer=self.sock(72)
        def accepted():clock[0]=8.0;return peer,('127.0.0.1',42000)
        value.listener.accept.side_effect=accepted
        with patch.object(s.select,'select',return_value=([value.listener],[],[])),patch.object(s.os,'fstat') as metadata:
            value.worker()
        self.assertIn(peer,value.held);metadata.assert_not_called();peer.close.assert_not_called()
        self.assertTrue(value.failed and value.owner.sealed)

    def test_worker_known_two_peer_bytes_and_eof_return_without_any_close(self):
        value,clock=self.fixture();value.listener=self.hold(value,self.sock())
        first,second=self.sock(72),self.sock(73)
        value.listener.accept.side_effect=[(first,('127.0.0.1',42001)),(second,('127.0.0.1',42002))]
        first.recv.side_effect=second.recv.side_effect=[s.PAYLOAD,b'']
        sequence=iter([[value.listener],[value.listener],[first,second],[first,second],[]])
        def selected(*args):
            ready=next(sequence)
            if not ready:value.stopping=True
            return ready,[],[]
        with patch.object(s.select,'select',side_effect=selected),patch.object(s.os,'fstat',side_effect=self.metadata):
            value.worker()
        self.assertTrue(value.worker_done);self.assertFalse(value.failed or value.sealed)
        self.assertEqual(value.peer_eof,{id(first),id(second)});self.assertEqual(len(value.accepted),2)
        for peer in (first,second):peer.send.assert_called_once_with(s.PAYLOAD);peer.close.assert_not_called()
        value.listener.close.assert_not_called()

    def test_worker_extra_peer_byte_budget_or_wrong_address_seals_retains_no_close(self):
        for variant in ('third','bytes','address'):
            value,clock=self.fixture();value.listener=self.hold(value,self.sock());peer=self.hold(value,self.sock(72))
            value.peers=[peer];value.transferred[id(peer)]=4*len(s.PAYLOAD)
            if variant=='third':value.peers.append(self.hold(value,self.sock(73)))
            peer.recv.return_value=b'x'
            value.listener.accept.return_value=(self.sock(74),('192.0.2.1',42000))
            ready=[peer] if variant=='bytes' else [value.listener]
            with patch.object(s.select,'select',return_value=(ready,[],[])) as op, \
                 patch.object(s.os,'fstat',side_effect=self.metadata):value.worker()
            op.assert_called_once();self.assertTrue(value.failed and value.owner.sealed)
            for held in value.held:held.close.assert_not_called()
            peer.send.assert_not_called()

    def test_finish_requires_ack_and_normal_worker_return_before_server_close(self):
        for variant in ('known','no_witness','worker_live','worker_error','late_close'):
            value,clock=self.fixture();value.clients=[self.hold(value,self.sock(index)) for index in (71,72)]
            value.peers=[self.hold(value,self.sock(index)) for index in (73,74)]
            value.listener=self.hold(value,self.sock(75));value.witness_done=variant!='no_witness'
            def joined(**kwargs):
                value.worker_done=True
                value.worker_thread.is_alive.return_value=variant=='worker_live'
                value.failed=variant=='worker_error'
            value.worker_thread.join.side_effect=joined
            if variant=='late_close':value.clients[0].close.side_effect=lambda:clock.__setitem__(0,8.0)
            with patch.object(s.os,'fstat',side_effect=self.metadata):
                if variant=='known':
                    self.assertEqual(value.finish_positive(),{'positive_sockets_closed':True,
                        'owned_echo_thread_returned_without_error':True});self.assertTrue(value.finished)
                else:
                    with self.assertRaises(s.Refused):value.finish_positive()
            if variant in ('no_witness','late_close'):value.worker_thread.join.assert_not_called()
            for peer in value.peers+[value.listener]:
                if variant=='known':peer.close.assert_called_once()
                else:peer.close.assert_not_called()
            if variant=='late_close':value.clients[1].close.assert_not_called()

    def test_invalid_clock_address_and_source_no_legacy_cleanup_or_sendall(self):
        for bad in (float('inf'),float('nan'),True,0):
            value,clock=self.fixture();value.ownership.clock.return_value=bad;value.ownership.clock.side_effect=None
            with patch.object(s.socket,'socket') as op:
                with self.assertRaises(s.Refused):value.created()
                op.assert_not_called()
        for bad in (('127.0.0.1',True),('127.0.0.1',1.0),('192.0.2.1',80),['127.0.0.1',80]):
            with self.assertRaises(s.Refused):s.address(bad)
        for node in ast.walk(ast.parse(SOURCE.read_text())):
            if isinstance(node,ast.Try):self.assertEqual(node.finalbody,[])
            if isinstance(node,ast.With):self.fail('implicit failure cleanup')
            if isinstance(node,ast.Call):
                name=node.func.id if isinstance(node.func,ast.Name) else node.func.attr if isinstance(node.func,ast.Attribute) else ''
                self.assertNotIn(name,{'sendall','kill','waitpid','waitid','poll','unlink','rmdir','system','exec','eval'})


if __name__=='__main__':unittest.main()
