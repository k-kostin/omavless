# SPDX-License-Identifier: MIT
import json
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('dns_readback',
    Path(__file__).with_name('native_dns_readback.py'))
dns = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dns)


def frame(kind, value):
    return json.dumps({'type': kind, 'data': value}).encode()


class DnsReadbackTests(unittest.TestCase):
    def good(self):
        return dns.project(frame('a(sb)', [['.', True]]),
            frame('a(iay)', [[2, [198, 18, 0, 2]]]), frame('b', True))

    def test_fixed_fixture_matches(self):
        self.assertTrue(all(self.good().values()))

    def test_empty_properties_are_not_success(self):
        result = dns.project(frame('a(sb)', []), frame('a(iay)', []), frame('b', False))
        self.assertTrue(result['available'])
        self.assertFalse(result['rootRoutingDomain'])
        self.assertFalse(result['fixedTunDns'])
        self.assertFalse(result['dnsDefaultRoute'])

    def test_extra_private_data_does_not_escape_or_match(self):
        result = dns.project(frame('a(sb)', [['.', True], ['private.invalid', False]]),
            frame('a(iay)', [[2, [10, 42, 43, 44]]]), frame('b', True))
        self.assertFalse(result['rootRoutingDomain'])
        self.assertFalse(result['fixedTunDns'])
        self.assertNotIn('private', json.dumps(result))
        self.assertNotIn('42', json.dumps(result))

    def test_boolean_numeric_confusion_is_rejected(self):
        for raw in (frame('b', 1), frame('b', 'true'), frame('b', None)):
            with self.subTest(raw=raw), self.assertRaises(dns.Unavailable):
                dns.project(frame('a(sb)', []), frame('a(iay)', []), raw)
        with self.assertRaises(dns.Unavailable):
            dns.project(frame('a(sb)', []), frame('a(iay)', [[2, [True, 0, 0, 1]]]), frame('b', True))

    def test_invalid_json_never_echoes_input(self):
        for raw in (b'password=secret', b'\xff', b'{"data":0,"data":1}', b'NaN',
                    b'{}{}', b' ' * (dns.LIMIT+1)):
            with self.subTest(raw=raw[:20]), self.assertRaises(dns.Unavailable) as error:
                dns.property_data(raw, 'b')
            self.assertEqual(str(error.exception), 'dns_readback_unavailable')

    def test_wrong_property_types_and_shapes_refuse(self):
        for domains in ([['.', 1]], ['.'], [['a'*254, False]], [None], [['\ud800',False]]):
            with self.assertRaises(dns.Unavailable):
                dns.project(frame('a(sb)', domains), frame('a(iay)', []), frame('b', True))
        for rows in ([[2,[1]]], [[10,[0]*4]], [[1,[0]*4]], [[2,[256]*4]], [None]):
            with self.assertRaises(dns.Unavailable):
                dns.project(frame('a(sb)', []), frame('a(iay)', rows), frame('b', True))

    def test_link_path_is_typed_and_bounded(self):
        self.assertEqual(dns.link_path(frame('o', ['/org/freedesktop/resolve1/link/_318'])),
            '/org/freedesktop/resolve1/link/_318')
        for value in ('shell command', ['/tmp/socket'], ['/org/freedesktop/resolve1/link/../x'],
                      ['/org/freedesktop/resolve1/link/'+'a'*256], []):
            with self.assertRaises(dns.Unavailable): dns.link_path(frame('o', value))

    def test_link_inventory_missing_multiple_or_invalid(self):
        self.assertIsNone(dns.tun_index(b'[]'))
        def link(index): return {'ifindex': index, 'linkinfo': {'info_kind':'tun'}}
        for rows in ([link(True)], [link(0)], [link(2**31)], [link(1),link(2)], [None]):
            with self.assertRaises(dns.Unavailable): dns.tun_index(json.dumps(rows).encode())

    def fake(self, changed=False):
        calls = []
        def command(argv, timeout, limit):
            self.assertGreater(timeout, 0)
            self.assertLessEqual(timeout, 5)
            calls.append(argv)
            if argv[0] == '/usr/bin/ip':
                index = 20 if changed and len(calls)>1 else 18
                return json.dumps([{'ifindex':index,'linkinfo':{'info_kind':'tun'}}]).encode()
            if 'GetLink' in argv: return frame('o', ['/org/freedesktop/resolve1/link/_318'])
            return {'Domains':frame('a(sb)', [['.',True]]), 'DNS':frame('a(iay)',dns.FIXED_DNS),
                'DefaultRoute':frame('b',True)}[argv[-1]]
        return command, calls

    def test_collector_has_only_fixed_read_commands(self):
        command, calls = self.fake()
        self.assertTrue(all(dns.observe(command).values()))
        self.assertEqual(len(calls), 6)
        self.assertFalse(any('sudo' in x or 'pkexec' in x for call in calls for x in call))
        self.assertEqual([c[-1] for c in calls if 'get-property' in c], list(dns.PROPERTIES))

    def test_changed_interface_refuses(self):
        self.assertEqual(dns.observe(self.fake(changed=True)[0]), {'available':False})

    def test_command_failure_returns_no_raw_error(self):
        def fail(*args, **kwargs): raise RuntimeError('https://private.invalid/key')
        self.assertEqual(dns.observe(fail), {'available':False})

    def test_no_link_needs_no_dbus_call(self):
        calls=[]
        def empty(argv, **kwargs): calls.append(argv); return b'[]'
        self.assertEqual(dns.observe(empty), {'available':True,'managedLinkPresent':False})
        self.assertEqual(len(calls), 1)

    def test_total_deadline_is_not_extended(self):
        with patch.object(dns.time,'monotonic', side_effect=[0,6]):
            self.assertEqual(dns.observe(self.fake()[0]), {'available':False})

    def test_human_observation_not_inferred(self):
        self.assertEqual(dns.classify({'lastKnownActual':'connected'}, self.good(),'none'),
            'authorization_not_observed')
        self.assertEqual(dns.classify({}, self.good(),'accepted'), 'accepted_dns_matches')
        self.assertEqual(dns.classify({}, self.good(),'delayed-accepted'), 'accepted_dns_matches')

    def test_cancellation_does_not_mean_success(self):
        missing = dict(self.good(), fixedTunDns=False)
        self.assertEqual(dns.classify({'lastKnownActual':'connected'},missing,'cancelled'),
            'connected_with_unconfirmed_dns_after_cancel')
        self.assertEqual(dns.classify({},self.good(),'cancelled'),'cancelled_but_dns_matches')
        self.assertEqual(dns.classify({},missing,'cancelled'),'cancelled_dns_not_ready')
        self.assertEqual(dns.classify({},missing,'accepted'),'accepted_dns_not_ready')
        self.assertEqual(dns.classify({}, {'available':False},'cancelled'),'dns_readback_unavailable')
        self.assertEqual(dns.classify({}, {'available':True,'managedLinkPresent':False},'cancelled'),
            'managed_link_absent')


if __name__ == '__main__':
    unittest.main()
