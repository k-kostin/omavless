#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Opt-in synthetic packet gate in NEW user/net/PID namespaces only.

No host routes, DNS, provider, store, sudo or installed runtime. Fixed local
TCP/UDP echo peers exercise actual TUN -> core -> peer -> TUN packet paths.
"""
import concurrent.futures
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import struct
import subprocess
import sys
import tempfile

import dns_core_namespace_probe as base
import dns_core_ownership_probe as ownership

MARK = 66
PEER = '192.0.2.1'
TUN_ADDRESS = '198.18.0.1'
PAYLOAD = b'omavless-synthetic-packet-check'
FACTS = {'isolated', 'unrestricted_tcp', 'unrestricted_udp',
         'restricted_tcp', 'restricted_udp', 'core_clean_exit', 'no_dns_calls',
         'marked_route_is_tun', 'unmarked_route_is_local', 'bidirectional_tun_bytes'}


def ip(*args):
    subprocess.run(['/usr/bin/ip', *args], check=True, timeout=3,
                   stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                   stderr=subprocess.DEVNULL)


def ip_query(*args):
    result = subprocess.run(['/usr/bin/ip', '-j', *args], check=True, timeout=3,
                            stdin=subprocess.DEVNULL, capture_output=True)
    base.require(len(result.stdout) <= 8192)
    return json.loads(result.stdout)


def tun_counters():
    value = ip_query('-s', 'link', 'show', 'dev', 'ovdnsprobe0')
    base.require(len(value) == 1 and value[0]['ifname'] == 'ovdnsprobe0')
    counts = value[0]['stats64']
    result = (counts['rx']['bytes'], counts['tx']['bytes'])
    base.require(all(type(item) is int and item >= 0 for item in result))
    return result


def echo_once(server, stream):
    try:
        if stream:
            connection, _ = server.accept()
            with connection:
                connection.settimeout(3)
                data = connection.recv(len(PAYLOAD) + 1)
                base.require(data == PAYLOAD)
                connection.sendall(data)
        else:
            data, target = server.recvfrom(len(PAYLOAD) + 1)
            base.require(data == PAYLOAD)
            server.sendto(data, target)
        return True
    except (OSError, RuntimeError):
        return False


def packet_case(stream):
    kind = socket.SOCK_STREAM if stream else socket.SOCK_DGRAM
    with socket.socket(socket.AF_INET, kind) as server:
        server.settimeout(5)
        server.bind((PEER, 0))
        if stream:
            server.listen(1)
        with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
            echo = pool.submit(echo_once, server, stream)
            matched = False
            with socket.socket(socket.AF_INET, kind) as client:
                client.settimeout(4)
                # Only this synthetic client uses the namespace-only TUN table.
                # Core egress stays unmarked and reaches the local peer directly.
                client.setsockopt(socket.SOL_SOCKET, socket.SO_MARK, MARK)
                client.bind((TUN_ADDRESS, 0))
                try:
                    client.connect(server.getsockname())
                    client.sendall(PAYLOAD)
                    matched = client.recv(len(PAYLOAD) + 1) == PAYLOAD
                except OSError:
                    pass
            return echo.result(timeout=6) and matched


def experiment(root, source, digest, original_net, original_user, original_pid):
    base.guard(original_net, original_user, original_pid)
    binary = root / 'candidate'
    shutil.copyfile(source, binary)
    binary.chmod(0o700)
    base.require(not os.listxattr(binary))
    base.require(hashlib.sha256(binary.read_bytes()).hexdigest() == digest)
    fd = os.open('/dev/net/tun', os.O_RDWR | os.O_CLOEXEC | os.O_NONBLOCK)
    facts = {'isolated': True, 'core_clean_exit': True, 'no_dns_calls': True}
    try:
        fcntl.ioctl(fd, base.ns.TUNSETIFF,
                    struct.pack('16sH22x', b'ovdnsprobe0', base.ns.FLAGS))
        ip('link', 'set', 'dev', 'lo', 'up')
        ip('address', 'add', PEER + '/32', 'dev', 'lo')
        ip('address', 'add', TUN_ADDRESS + '/30', 'dev', 'ovdnsprobe0')
        ip('link', 'set', 'dev', 'ovdnsprobe0', 'up')
        # The echo address is local in this fully disposable namespace. Permit
        # its synthesized TUN reply; this is not a host sysctl or product need.
        subprocess.run(['/usr/bin/sysctl', '-q', '-w',
                        'net.ipv4.conf.ovdnsprobe0.accept_local=1',
                        'net.ipv4.conf.ovdnsprobe0.rp_filter=0'],
                       check=True, timeout=3, stdin=subprocess.DEVNULL,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        # Only a fresh namespace reaches this point. The mark rule must precede
        # local delivery so the client cannot accidentally bypass the core.
        ip('rule', 'del', 'pref', '0')
        ip('rule', 'add', 'pref', '10', 'fwmark', str(MARK), 'table', '100')
        ip('rule', 'add', 'pref', '20', 'lookup', 'local')
        ip('route', 'add', 'table', '100', 'default', 'dev', 'ovdnsprobe0')
        facts['marked_route_is_tun'] = ip_query('route', 'get', PEER, 'mark', str(MARK))[0]['dev'] == 'ovdnsprobe0'
        facts['unmarked_route_is_local'] = ip_query('route', 'get', PEER)[0]['dev'] == 'lo'
        base.require(facts['marked_route_is_tun'] and facts['unmarked_route_is_local'])
        facts['bidirectional_tun_bytes'] = True
        for restricted in (False, True):
            label = 'restricted' if restricted else 'unrestricted'
            case = root / label
            case.mkdir(mode=0o700)
            child = ownership.launch(case, binary, True, fd=fd, restricted=restricted)
            try:
                ownership.wait_ready(case, child, fd)
                base.require(ownership.flag(case))
                before = tun_counters()
                facts[label + '_tcp'] = packet_case(True)
                facts[label + '_udp'] = packet_case(False)
                after = tun_counters()
                facts['bidirectional_tun_bytes'] &= all(new > old for old, new in zip(before, after))
            finally:
                facts['core_clean_exit'] &= ownership.stop_cleanly(child)
            facts['no_dns_calls'] &= base.calls(case) == []
    finally:
        os.close(fd)
    return facts


def main():
    try:
        if len(sys.argv) == 7 and sys.argv[1] == '--isolated':
            base.guard(*sys.argv[2:5])
            source, digest = Path(sys.argv[5]), sys.argv[6]
            ownership.validate_source(source, digest)
            os.umask(0o077)
            with tempfile.TemporaryDirectory(prefix='omavless-dns-packets.') as directory:
                facts = experiment(Path(directory), source, digest, *sys.argv[2:5])
            base.require(set(facts) == FACTS)
            print(json.dumps(facts, sort_keys=True))
            return 0 if all(facts.values()) else 1
        if len(sys.argv) != 3 or os.geteuid() == 0:
            print('{"unprivileged_parent_required":true}')
            return 2
        source, digest = Path(sys.argv[1]), sys.argv[2]
        ownership.validate_source(source, digest)
        result = subprocess.run([
            '/usr/bin/unshare', '--user', '--map-root-user', '--net', '--pid',
            '--fork', '--kill-child=SIGKILL', '--', sys.executable,
            str(Path(__file__).resolve()), '--isolated',
            base.ns.namespace('net'), base.ns.namespace('user'), base.ns.namespace('pid'),
            str(source), digest,
        ], stdin=subprocess.DEVNULL, capture_output=True, timeout=40, check=False)
        base.require(result.returncode in (0, 1))
        from dns_tun_authority_probe import project
        facts = project(result.stdout, FACTS)
        print(json.dumps({'coreSha256': digest, 'facts': facts}, sort_keys=True))
        return 0 if all(facts.values()) else 1
    except Exception:
        print('{"isolated_core_packet_probe_failed":true}')
        return 1


if __name__ == '__main__':
    sys.exit(main())
