"""Inert controller protocol/ownership counterexamples; no actual socket/core."""
import ast
import importlib.util
import json
from pathlib import Path
import struct
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

SOURCE = Path(__file__).with_name('controller.py')
spec = importlib.util.spec_from_file_location('retained_controller', SOURCE)
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)
FIRST = '11111111-1111-4111-8111-111111111111'
SECOND = '22222222-2222-4222-8222-222222222222'


def owner():
    child = SimpleNamespace(pid=17)
    value = SimpleNamespace(sealed=False, live=Mock(), available=Mock(),
                            anchors={'core':{'child':child}})
    return value, child


def controller():
    value = c.Controller.__new__(c.Controller)
    value.owner, value.core = owner()
    value.sealed = False; value._identity = object(); value.deadline = 40.0
    value.held, value.streams = [], []
    value.directory, value.socket = 1, 2
    directory = SimpleNamespace(st_dev=3,st_ino=4,st_mode=0o40700,st_uid=1000,st_gid=1000,st_nlink=2)
    socket = SimpleNamespace(st_dev=3,st_ino=5,st_mode=0o140600,st_uid=1000,st_gid=1000,st_nlink=1)
    value.directory_identity, value.socket_identity = c.identity(directory), c.identity(socket)
    return value, directory, socket


def stream(response=b'HTTP/1.0 204 No Content\r\n\r\n'):
    value = Mock()
    value.getsockopt.return_value = struct.pack('3i',17,1000,1000)
    value.send.side_effect = lambda raw: len(raw)
    value.recv.side_effect = [response,b'']
    return value


class Controls(unittest.TestCase):
    def setUp(self):
        self.clock = patch.object(c.time,'monotonic',return_value=0.0)
        self.clock.start()

    def tearDown(self):
        self.clock.stop()

    def snapshot(self, rows):
        return json.dumps({'connections':rows}).encode()

    def row(self, identifier=FIRST, token='42'):
        return {'id':identifier,'omavlessCloseToken':token}

    def test_strict_private_identity_grammar_uniqueness_and_bounds(self):
        self.assertEqual(c.targets(self.snapshot([self.row(),self.row(SECOND,'43')])),[(FIRST,'42'),(SECOND,'43')])
        self.assertEqual(c.targets(b'{"connections":null}'),[])
        for rows in ([self.row(),self.row()], [self.row(),self.row(SECOND)], [self.row()]*129,
                     [{'id':FIRST}], [self.row(token=42)], [self.row(token='0')],
                     [self.row(token='01')], [self.row(token=str(2**64))],
                     [self.row(identifier=FIRST[:-1]+'A')], [self.row(identifier='../secret')]):
            with self.subTest(rows=rows[:2]), self.assertRaises(c.Refused):c.targets(self.snapshot(rows))
        for raw in (b'{"connections":[],"connections":[]}',b'{"connections":NaN}',
                    self.snapshot([self.row()]).replace(b'"42"',b'"42","omavlessCloseToken":"43"'),
                    b'{"connections":[],"unused":1e999}',b'x'*(c.MAX_SNAPSHOT+1)):
            with self.assertRaises((c.Refused,ValueError)):c.targets(raw)

    def test_exact_http_status_length_and_no_transfer_or_duplicate_headers(self):
        self.assertEqual(c.parse_http(b'HTTP/1.1 409 Conflict\r\nContent-Length: 0\r\n\r\n'),(409,b''))
        for raw in (b'HTTP/1.1 409\r\nContent-Length: 1\r\n\r\n',
                    b'HTTP/1.1 204\r\nTransfer-Encoding: chunked\r\n\r\n',
                    b'HTTP/1.1 204\r\nContent-Length: 0\r\nContent-Length: 0\r\n\r\n',
                    b'HTTP/2 204\r\n\r\n',b'HTTP/1.0 099\r\n\r\n',
                    b'HTTP/1.0 204\r\nBad Header: 0\r\n\r\n',
                    b'HTTP/1.0 204\r\nX: \x00\r\n\r\n',b'x'*8193+b'\r\n\r\n'):
            with self.assertRaises((c.Refused,UnicodeError)):c.parse_http(raw)

    def test_actual_check_exact_peer_directory_socket_and_owner_anchor(self):
        value,directory,sock=controller();peer=stream()
        with patch.object(c.os,'fstat',side_effect=lambda fd: directory if fd==1 else sock), \
             patch.object(c.os,'stat',side_effect=lambda name,**kw:directory if name==c.DIRECTORY else sock):
            value.check(peer)
        self.assertEqual(value.owner.live.call_count,2)
        peer.getsockopt.assert_called_once_with(c.socket.SOL_SOCKET,c.socket.SO_PEERCRED,12)

    def test_path_peer_identity_core_session_refusal_permanently_seals(self):
        for variant in ('peer_pid','peer_uid','peer_gid','dir_mode','socket_mode','socket_inode',
                        'owner_child','core_dead','socket_replaced'):
            value,directory,sock=controller();peer=stream()
            named_sock=sock
            if variant.startswith('peer_'):
                creds=[17,1000,1000];creds[{'peer_pid':0,'peer_uid':1,'peer_gid':2}[variant]]=18
                peer.getsockopt.return_value=struct.pack('3i',*creds)
            if variant=='dir_mode':directory.st_mode=0o40755
            if variant=='socket_mode':sock.st_mode=0o140666
            if variant=='socket_inode':sock.st_ino+=1
            if variant=='owner_child':value.owner.anchors['core']['child']=SimpleNamespace(pid=17)
            if variant=='core_dead':value.owner.live.side_effect=RuntimeError('private-live-error')
            if variant=='socket_replaced':
                named_sock=SimpleNamespace(**vars(sock));named_sock.st_ino+=1
            with patch.object(c.os,'fstat',side_effect=lambda fd:directory if fd==1 else sock), \
                 patch.object(c.os,'stat',side_effect=lambda name,**kw:directory if name==c.DIRECTORY else named_sock):
                with self.assertRaises(c.Refused):value.check(peer)
                self.assertTrue(value.sealed and value.owner.sealed)
                old_calls=value.owner.live.call_count
                with self.assertRaises(c.Refused):value.check(peer)
                self.assertEqual(value.owner.live.call_count,old_calls)

    def test_every_single_write_immediately_rechecks_exact_owner_and_peer(self):
        value,_,_=controller();peer=stream();events=[]
        value.check=Mock(side_effect=lambda current=None:events.append(('check',current)))
        def send(raw):
            self.assertEqual(events[-1],('check',peer));events.append(('send',None));return len(raw)
        peer.send.side_effect=send
        selected=c.Target(FIRST,'42',value._identity,value.core)
        with patch.object(c.socket,'socket',return_value=peer):
            result=value.exchange('conditional_close',selected)
        self.assertEqual(result,(204,b''));self.assertEqual(peer.send.call_count,1)
        peer.connect.assert_called_once_with('/home/core/controller.sock')
        request=peer.send.call_args.args[0]
        self.assertIn(b'POST /connections/'+FIRST.encode()+b'/close-conditional',request)
        self.assertIn(b'If-Match: "42"',request)
        peer.close.assert_called_once()

    def test_unknown_short_throw_late_send_or_reply_never_retry_close_or_followup(self):
        for variant in ('short','throw','late','reply','oversize'):
            value,_,_=controller();value.check=Mock();peer=stream();clock=[0.0]
            if variant=='short':peer.send.side_effect=lambda raw:len(raw)-1
            if variant=='throw':peer.send.side_effect=OSError('private-error')
            if variant=='late':
                def late(raw):clock[0]=4.0;return len(raw)
                peer.send.side_effect=late
            if variant=='reply':peer.recv.side_effect=OSError('private-reply')
            if variant=='oversize':peer.recv.side_effect=[b'x'*(c.MAX_REPLY+1)]
            selected=c.Target(FIRST,'42',value._identity,value.core)
            with patch.object(c.socket,'socket',return_value=peer) as created, \
                 patch.object(c.time,'monotonic',side_effect=lambda:clock[0]):
                with self.assertRaises(c.Refused):value.exchange('conditional_close',selected)
                self.assertTrue(value.sealed and value.owner.sealed)
                peer.close.assert_not_called()
                count=created.call_count
                # Restore no-op mocked check to model the actual sealed check gate.
                value.check=c.Controller.check.__get__(value)
                with self.assertRaises(c.Refused):value.exchange('capabilities')
                self.assertEqual(created.call_count,count)
            self.assertEqual(peer.send.call_count,1)
            if variant in ('short','throw','late'):peer.recv.assert_not_called()

    def test_expired_or_nonfinite_clock_never_connects_or_writes(self):
        for clock in (float('nan'),float('inf'),True,1,41.0):
            value,_,_=controller()
            with patch.object(c.time,'monotonic',return_value=clock),patch.object(c.socket,'socket') as opened:
                with self.assertRaises(c.Refused):value.exchange('snapshot')
                opened.assert_not_called()
            self.assertTrue(value.sealed and value.owner.sealed)

    def test_cross_session_same_numeric_core_refuses_before_readiness(self):
        value,_,_=controller();other,_,_=controller();other.core=value.core
        selected=c.Target(FIRST,'42',other._identity,other.core)
        with patch.object(value,'ready') as ready,patch.object(value,'exchange') as exchange:
            with self.assertRaises(c.Refused):value.close_witness(selected)
            ready.assert_not_called();exchange.assert_not_called()
        self.assertTrue(value.sealed and value.owner.sealed)
        self.assertNotIn(FIRST,repr(selected));self.assertNotIn('42',repr(selected))

    def test_exact_empty_effect_receipts_and_wrong_token_not_parent_authority(self):
        for code,expected in ((204,'closed'),(404,'missing'),(409,'changed')):
            value,_,_=controller();selected=c.Target(FIRST,'42',value._identity,value.core)
            with patch.object(value,'ready'),patch.object(value,'exchange',return_value=(code,b'')):
                self.assertEqual(value.close_witness(selected),expected)
        for code,body in ((200,b''),(404,b'generic private HTML'),(409,b'x'),(503,b'')):
            value,_,_=controller();selected=c.Target(FIRST,'42',value._identity,value.core)
            with patch.object(value,'ready'),patch.object(value,'exchange',return_value=(code,body)):
                with self.assertRaises(c.Refused):value.close_witness(selected)
            self.assertTrue(value.sealed and value.owner.sealed)

    def test_wrong_token_derived_from_bound_private_target_only(self):
        for token,expected in (('42','43'),(str(2**64-1),'1')):
            value,_,_=controller();value.check=Mock();peer=stream(b'HTTP/1.0 409 Conflict\r\n\r\n')
            selected=c.Target(FIRST,token,value._identity,value.core)
            with patch.object(c.socket,'socket',return_value=peer):value.exchange('conditional_close',selected,True)
            self.assertIn(('If-Match: "'+expected+'"').encode(),peer.send.call_args.args[0])

    def test_exact_typed_capabilities_before_snapshot_or_close(self):
        for body in (b'{"abi":1,"ready":true}',):
            value,_,_=controller()
            with patch.object(value,'exchange',return_value=(200,body)):value.ready()
        for body in (b'{"abi":true,"ready":true}',b'{"abi":1,"ready":false}',
                     b'{"abi":1,"ready":true,"private":1}',b'{"abi":1,"abi":1,"ready":true}'):
            value,_,_=controller()
            with patch.object(value,'exchange',return_value=(200,body)):
                with self.assertRaises(c.Refused):value.ready()
            self.assertTrue(value.sealed and value.owner.sealed)

    def test_no_generic_delete_shell_process_cleanup_or_source_fallback(self):
        tree=ast.parse(SOURCE.read_text())
        forbidden={'exec','eval','kill','waitpid','waitid','system','spawn','unlink','rmdir','mkdir'}
        for node in ast.walk(tree):
            if isinstance(node,ast.Call):
                name=node.func.id if isinstance(node.func,ast.Name) else node.func.attr if isinstance(node.func,ast.Attribute) else ''
                self.assertNotIn(name,forbidden)
        self.assertNotIn('DELETE ',SOURCE.read_text())
        self.assertNotIn('AF_INET',SOURCE.read_text())


if __name__ == '__main__':
    unittest.main()
