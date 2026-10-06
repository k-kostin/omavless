"""Pure in-memory protocol tests: no TLS context, socket, key or filesystem."""

import base64
import io
import struct
import unittest
from unittest.mock import patch

import peer


class Stream:
    def __init__(self, data, chunk=1):
        self.input = io.BytesIO(data)
        self.output = bytearray()
        self.chunk = chunk

    def read(self, count):
        return self.input.read(min(count, self.chunk))

    def write(self, data):
        self.output.extend(data)


def query():
    return struct.pack('!6H', 0, 0x100, 1, 0, 0, 0) + peer.QUESTION + b'\x00\x01\x00\x01'


def encoded(data):
    return base64.urlsafe_b64encode(data).rstrip(b'=')


class Protocol(unittest.TestCase):
    def test_vless_fixed_targets(self):
        for address, port in (peer.HTTP_TARGET, peer.DOH_TARGET):
            frame = b'\x00' + bytes(16) + b'\x00\x01' + struct.pack('!H', port) + b'\x01' + address
            self.assertEqual(peer.vless(Stream(frame), bytes(16)), (address, port))
            for length in range(len(frame)):
                with self.assertRaises(peer.Refused):
                    peer.vless(Stream(frame[:length]), bytes(16))

    def test_vless_closed_vocabulary(self):
        frame = b'\x00' + bytes(16) + b'\x00\x01\x00\x50\x01' + peer.HTTP_TARGET[0]
        for offset in (0, 1, 17, 18, 19, 20, 21, 22, 25):
            bad = bytearray(frame)
            bad[offset] ^= 1
            with self.assertRaises(peer.Refused):
                peer.vless(Stream(bad), bytes(16))

    def test_http_success(self):
        stream = Stream(b'GET /k1 HTTP/1.1\r\nHost: 192.0.2.80\r\n\r\n')
        peer.answer_http(stream)
        self.assertEqual(bytes(stream.output), peer.response(peer.BODY))

    def test_http_refusals(self):
        for data in (
            b'POST /k1 HTTP/1.1\r\n\r\n', b'GET /k1 HTTP/1.0\r\n\r\n',
            b'GET /k1 HTTP/1.1\r\nHost: wrong\r\n\r\n',
            b'GET /wrong HTTP/1.1\r\nHost: 192.0.2.80\r\n\r\n',
            b'GET /k1 HTTP/1.1\r\nHost: 192.0.2.80\r\nHost: wrong\r\n\r\n',
            b'GET /k1 HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n',
            b'GET /k1 HTTP/1.1\r\nContent-Length: 1\r\n\r\nx',
            b'GET /k1 HTTP/1.1\r\n folded: value\r\n\r\n',
            b'GET /k1 HTTP/1.1\r\nX: \x00\r\n\r\n', b'x' * 4097,
        ):
            with self.assertRaises(peer.Refused):
                peer.answer_http(Stream(data, 1024))

    def test_dns_answer_exact(self):
        answer = peer.dns_answer(encoded(query()))
        self.assertEqual(struct.unpack('!6H', answer[:12]), (0, 0x8180, 1, 1, 0, 0))
        self.assertEqual(answer[12:12 + len(peer.QUESTION) + 4], query()[12:])
        self.assertEqual(answer[-4:], peer.HTTP_TARGET[0])

    def test_dns_refusals(self):
        bad_queries = [query() + b'x', query()[:-1], b'', query() + bytes(11)]
        for offset in (2, 3, 5, 7, 9, 11, 12, len(query()) - 3, len(query()) - 1):
            changed = bytearray(query())
            changed[offset] ^= 1
            bad_queries.append(bytes(changed))
        for data in bad_queries:
            with self.assertRaises(peer.Refused):
                peer.dns_answer(encoded(data))
        for value in (b'!', b'A', b'A' * 513, encoded(query()) + b'=', b'\xff'):
            with self.assertRaises(peer.Refused):
                peer.dns_answer(value)

    def test_doh_finite_cases(self):
        request = b'GET /dns-query?dns=' + encoded(query()) + b' HTTP/1.1\r\nHost: 1.1.1.1\r\n\r\n'
        for mode in peer.MODES:
            stream = Stream(request)
            peer.answer_doh(stream, mode)
            output = bytes(stream.output)
            if mode == 'doh-close':
                self.assertEqual(output, b'')
            elif mode.startswith('redirect-'):
                self.assertIn(b'302 Found', output)
                self.assertIn(b'1.1.1.1/refused', output)
            elif mode == 'doh-malformed':
                self.assertTrue(output.endswith(b'\r\n\r\n\x00'))
            else:
                self.assertTrue(output.endswith(peer.dns_answer(encoded(query()))))

    def test_unknown_mode_refused_before_input_or_socket(self):
        with patch.object(peer.os, 'getuid', side_effect=AssertionError('unexpected effect')):
            with self.assertRaises(peer.Refused):
                peer.serve('not-a-mode')

    def test_wire_budget_and_deadline(self):
        class Socket:
            def settimeout(self, value):
                self.timeout = value

            def recv(self, count):
                return b'x' * count

            def sendall(self, data):
                pass

        with patch.object(peer.time, 'monotonic', return_value=10):
            expired = peer.Wire(Socket(), 10)
            with self.assertRaises(peer.Refused):
                expired.read(1)
            wire = peer.Wire(Socket(), 11)
            for _ in range(4):
                self.assertEqual(len(wire.read(16384)), 16384)
            with self.assertRaises(peer.Refused):
                wire.read(1)
            wire.write(bytes(65536))
            with self.assertRaises(peer.Refused):
                wire.write(b'x')


if __name__ == '__main__':
    unittest.main()
