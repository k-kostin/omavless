#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Opt-in real-core experiment, NEVER an installed DNS authorization workaround.

New user/network/PID namespaces; fresh synthetic DIRECT-only config; copied core
without file capabilities; stub resolvectl records verbs and has no DNS effects.
The normal test suite tests guards with mocks and never starts this experiment.
No private store, host controller, provider traffic, sudo or system bus calls.
"""
import hashlib
import http.client
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import stat
import subprocess
import sys
import tempfile
import time

import dns_tun_namespace_probe as ns

CORE = Path('/usr/bin/mihomo')
VERBS = ('domain', 'default-route', 'dns', 'revert')
FACTS = {
    'isolated', 'initial_dns_calls', 'same_tun_reload_kept_process',
    'same_tun_reload_kept_index', 'same_tun_reload_no_dns_calls',
    'same_tun_reload_mode_applied',
    'changed_tun_reload_reconfigured_dns', 'shutdown_reverted_dns',
    'cleanup_core_exited',
}


def require(value):
    if not value:
        raise RuntimeError('isolated_core_probe_refused')


def guard(original_net, original_user, original_pid):
    # Check BEFORE any core launch, device access or fixture creation.
    require(ns.namespace('net') != original_net)
    require(ns.namespace('user') != original_user)
    require(ns.namespace('pid') != original_pid)
    require(os.geteuid() == 0)
    require({item['ifname'] for item in ns.links()} == {'lo'})


def config(root, device, mode):
    require(device in ('ovdnsprobe0', 'ovdnsprobe1') and mode in ('direct', 'rule'))
    # auto-route=false avoids unnecessary route work, but DOES NOT disable
    # resolved calls. An unchanged TUN section is required for reuse on reload.
    return ('mode: ' + mode + '\nlog-level: silent\nipv6: false\n'
            'external-controller-unix: ' + json.dumps(str(root / 'control.sock')) + '\n'
            'tun:\n  enable: true\n  device: ' + device + '\n'
            '  stack: system\n  auto-route: false\n  auto-detect-interface: false\n'
            '  dns-hijack: []\ndns:\n  enable: false\n'
            'proxies: []\nproxy-groups: []\nrules:\n  - MATCH,DIRECT\n')


def calls(root):
    path = root / 'calls'
    if not path.exists():
        return []
    require(path.stat().st_size <= 1024)
    verbs = path.read_text().splitlines()
    require(all(verb in VERBS for verb in verbs))
    return verbs


def index(device):
    entries = [item['ifindex'] for item in ns.links() if item['ifname'] == device]
    require(len(entries) == 1 and type(entries[0]) is int)
    return entries[0]


def request(root, method, target, payload=None):
    # Only the scratch controller, not the installed runtime/controller.
    connection = http.client.HTTPConnection('localhost', timeout=1)
    connection.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    connection.sock.settimeout(1)
    try:
        connection.sock.connect(str(root / 'control.sock'))
        body = None if payload is None else json.dumps(payload).encode()
        connection.request(method, target, body, {'Content-Type': 'application/json'})
        response = connection.getresponse()
        require(200 <= response.status < 300)
        body = response.read(8193)
        require(len(body) <= 8192)
        return body
    finally:
        connection.close()


def wait_for(predicate, child, seconds=8):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        require(child.poll() is None)
        try:
            if predicate():
                return
        except (OSError, RuntimeError, http.client.HTTPException):
            pass
        time.sleep(0.05)
    raise RuntimeError('isolated_core_probe_timeout')


def ready(root):
    request(root, 'GET', '/version')
    return calls(root) == ['domain', 'default-route', 'dns']


def stop(child):
    if child.poll() is None:
        child.send_signal(signal.SIGTERM)
        try:
            child.wait(timeout=4)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait(timeout=2)
    return child.poll() is not None


def core_digest():
    metadata = CORE.lstat()
    require(stat.S_ISREG(metadata.st_mode) and metadata.st_uid == 0
            and not metadata.st_mode & 0o022 and metadata.st_size <= 128 * 1024 * 1024)
    return hashlib.sha256(CORE.read_bytes()).hexdigest()


def experiment(root, expected_digest):
    binary = root / 'mihomo'
    shutil.copyfile(CORE, binary)  # deliberately NOT copy2: no capabilities/xattrs
    binary.chmod(0o700)
    require(not os.listxattr(binary))
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    require(digest == expected_digest)
    stub = root / 'resolvectl'
    stub.write_text('#!' + sys.executable + '\n'
                    'import os, sys\n'
                    'from pathlib import Path\n'
                    'verbs = ("domain", "default-route", "dns", "revert")\n'
                    'if len(sys.argv) < 3 or sys.argv[1] not in verbs: sys.exit(2)\n'
                    'if sys.argv[2] not in ("ovdnsprobe0", "ovdnsprobe1"): sys.exit(2)\n'
                    'with open(Path(__file__).with_name("calls"), "a") as f:\n'
                    '    f.write(sys.argv[1] + "\\n")\n')
    stub.chmod(0o700)
    path = root / 'config.yaml'
    path.write_text(config(root, 'ovdnsprobe0', 'direct'))
    environment = {
        'PATH': str(root), 'HOME': str(root), 'XDG_RUNTIME_DIR': str(root),
        'DBUS_SYSTEM_BUS_ADDRESS': 'unix:path=' + str(root / 'absent-system-bus'),
        'DBUS_SESSION_BUS_ADDRESS': 'unix:path=' + str(root / 'absent-session-bus'),
        'LANG': 'C',
    }
    child = subprocess.Popen([str(binary), '-d', str(root), '-f', str(path)],
                             env=environment, stdin=subprocess.DEVNULL,
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    facts = {'isolated': True}
    try:
        wait_for(lambda: ready(root), child)
        original_index = index('ovdnsprobe0')
        original_pid = child.pid
        facts['initial_dns_calls'] = True
        path.write_text(config(root, 'ovdnsprobe0', 'rule'))
        request(root, 'PUT', '/configs?force=true', {'path': str(path)})
        # Finite evidence window, not a universal assertion about future work.
        time.sleep(1)
        facts['same_tun_reload_kept_process'] = child.poll() is None and child.pid == original_pid
        facts['same_tun_reload_kept_index'] = index('ovdnsprobe0') == original_index
        facts['same_tun_reload_no_dns_calls'] = calls(root) == ['domain', 'default-route', 'dns']
        facts['same_tun_reload_mode_applied'] = json.loads(request(root, 'GET', '/configs')).get('mode') == 'rule'
        path.write_text(config(root, 'ovdnsprobe1', 'rule'))
        request(root, 'PUT', '/configs?force=true', {'path': str(path)})
        wait_for(lambda: calls(root) == ['domain', 'default-route', 'dns', 'revert',
                                       'domain', 'default-route', 'dns'], child)
        index('ovdnsprobe1')
        facts['changed_tun_reload_reconfigured_dns'] = True
    finally:
        facts['cleanup_core_exited'] = stop(child)
    facts['shutdown_reverted_dns'] = calls(root) == [
        'domain', 'default-route', 'dns', 'revert', 'domain', 'default-route', 'dns', 'revert']
    return {'facts': facts, 'coreSha256': digest}


def child(original_net, original_user, original_pid, expected_digest):
    guard(original_net, original_user, original_pid)
    os.umask(0o077)
    with tempfile.TemporaryDirectory(prefix='omavless-core-dns-isolated.') as directory:
        return experiment(Path(directory), expected_digest)


def project(raw, expected_facts=FACTS):
    require(len(raw) <= 2048)
    def unique(items):
        result = {}
        for key, value in items:
            require(key not in result)
            result[key] = value
        return result
    data = json.loads(raw, object_pairs_hook=unique)
    require(type(data) is dict and set(data) == {'facts', 'coreSha256'})
    facts, digest = data['facts'], data['coreSha256']
    require(type(facts) is dict and set(facts) == expected_facts)
    require(all(type(value) is bool for value in facts.values()))
    require(type(digest) is str and len(digest) == 64
            and all(c in '0123456789abcdef' for c in digest))
    return data


def main():
    if len(sys.argv) == 6 and sys.argv[1] == '--isolated-child':
        try:
            print(json.dumps(child(*sys.argv[2:]), sort_keys=True))
            return 0
        except Exception:
            print('{"isolated_core_probe_failed":true}')
            return 1
    if len(sys.argv) != 1 or os.geteuid() == 0:
        print('{"unprivileged_parent_required":true}')
        return 2
    try:
        digest = core_digest()  # host-root ownership checked before UID remapping
        result = subprocess.run([
            '/usr/bin/unshare', '--user', '--map-root-user', '--net', '--pid',
            '--fork', '--kill-child=SIGKILL', '--', sys.executable,
            str(Path(__file__).resolve()), '--isolated-child',
            ns.namespace('net'), ns.namespace('user'), ns.namespace('pid'), digest,
        ], stdin=subprocess.DEVNULL, capture_output=True, timeout=30, check=False)
        require(result.returncode == 0)
        data = project(result.stdout)
        print(json.dumps(data, sort_keys=True))
        return 0 if all(data['facts'].values()) else 1
    except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired):
        print('{"isolated_core_probe_unavailable":true}')
        return 1


if __name__ == '__main__':
    sys.exit(main())
