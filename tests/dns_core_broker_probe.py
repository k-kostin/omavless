#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Opt-in actual core/Rust/kernel channel test, NEVER a host DNS writer.

Private user/net/PID/mount namespaces and a fresh tmpfs /run. Fixed synthetic
TUN, no provider, no system bus, no DNS or routes. Ready here means mock-channel
acknowledgement only, not actual DNS readiness or installed acceptance.
"""
import hashlib
import http.client
import json
import os
from pathlib import Path
import select
import shutil
import signal
import subprocess
import sys
import tempfile
import time

import dns_core_namespace_probe as base
import dns_core_ownership_probe as ownership

FACTS = {
    'isolated', 'rust_admits_core_tun', 'pending_not_ready',
    'ack_projects_ready', 'release_sequence_completed',
    'refusal_not_ready', 'loss_stops_core', 'proof_holds_after_core_exit',
    'last_proof_close_removes_tun', 'no_core_dns_calls', 'all_children_exited',
}


def guard(original):
    base.require(len(original) == 4)
    base.guard(*original[:3])
    base.require(base.ns.namespace('mnt') != original[3])


def receive(child, expected):
    deadline = time.monotonic() + 8
    result = bytearray()
    while time.monotonic() < deadline and len(result) < 64:
        if select.select([child.stdout], [], [], max(0, deadline - time.monotonic()))[0]:
            byte = os.read(child.stdout.fileno(), 1)
            base.require(byte)
            result.extend(byte)
            if byte == b'\n':
                base.require(bytes(result) == expected.encode('ascii') + b'\n')
                return
    raise RuntimeError('fixture_response_unavailable')


def command(child, value):
    base.require(value in ('ready', 'reject', 'drop_channel', 'drop_proof'))
    child.stdin.write(value.encode('ascii') + b'\n')
    child.stdin.flush()


def readiness(root):
    try:
        value = json.loads(base.request(root, 'GET', '/configs')).get('tun', {})
        return value.get('omavless-dns-ready') is True
    except (OSError, RuntimeError, http.client.HTTPException):
        return False


def disable_tun(root):
    # Exercise actual listener.Close through its synchronous controller reload.
    # Upstream's controller can be ready before main registers signal.Notify;
    # early SIGTERM is therefore not a deterministic normal-close experiment.
    path = root / 'config.yaml'
    text = path.read_text()
    base.require(text.count('  enable: true\n') == 1)
    path.write_text(text.replace('  enable: true\n', '  enable: false\n', 1))
    base.request(root, 'PUT', '/configs?force=true', {'path': str(path)})


def stop_core(child):
    base.stop(child)
    return child.returncode in (0, -signal.SIGTERM)


def launch(root, core):
    # Fixed conversion of an existing synthetic config, never imported YAML.
    config = ownership.config(root, 'ovdnsprobe0', True).replace(
        'device: ovdnsprobe0', 'device: Meta').replace(
        'tun:\n', 'tun:\n  omavless-dns-broker: true\n', 1)
    path = root / 'config.yaml'
    path.write_text(config)
    (root / 'resolvectl').write_text(
        '#!' + sys.executable + '\nfrom pathlib import Path\n'
        'with open(Path(__file__).with_name("calls"), "a") as f: f.write("dns\\n")\n')
    (root / 'resolvectl').chmod(0o700)
    return subprocess.Popen([str(core), '-d', str(root), '-f', str(path)], env={
        'PATH': str(root), 'HOME': str(root), 'XDG_RUNTIME_DIR': str(root), 'LANG': 'C',
        'DBUS_SYSTEM_BUS_ADDRESS': 'unix:path=/run/absent-system-bus',
        'DBUS_SESSION_BUS_ADDRESS': 'unix:path=/run/absent-session-bus',
    }, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def copied(source, target, digest):
    shutil.copyfile(source, target)
    target.chmod(0o700)
    base.require(not os.listxattr(target))
    base.require(hashlib.sha256(target.read_bytes()).hexdigest() == digest)
    return target


def experiment(root, core_source, core_digest, fixture_source, fixture_digest, original):
    # Recheck before the mount as well as entry: no broad host /run mutation.
    guard(original)
    subprocess.run(['/usr/bin/mount', '--make-rprivate', '/'], check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=3)
    subprocess.run(['/usr/bin/mount', '-t', 'tmpfs', '-o', 'mode=0755,size=1m',
                    'tmpfs', '/run'], check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=3)
    subprocess.run(['/usr/bin/mount', '-t', 'proc', '-o', 'nosuid,nodev,noexec',
                    'proc', '/proc'], check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=3)
    broker_dir = Path('/run/omavless-dns')
    broker_dir.mkdir(mode=0o700)
    core = copied(core_source, root / 'core', core_digest)
    fixture = copied(fixture_source, root / 'fixture', fixture_digest)
    facts = dict.fromkeys(FACTS, True)
    for mode in ('success', 'reject', 'loss'):
        case = root / mode
        case.mkdir(mode=0o700)
        server = subprocess.Popen([str(fixture), '--fixture', mode, *original],
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.DEVNULL, env={'PATH': '/usr/bin'}, bufsize=0)
        child = None
        try:
            receive(server, 'fixture_ready')
            child = launch(case, core)
            receive(server, 'proof_admitted')
            base.require(child.poll() is None and base.index('Meta') > 0)
            facts['pending_not_ready'] &= not readiness(case)
            if mode == 'reject':
                command(server, 'reject')
                receive(server, 'lease_rejected')
                facts['refusal_not_ready'] &= not readiness(case)
                facts['all_children_exited'] &= stop_core(child)
            else:
                command(server, 'ready')
                receive(server, 'lease_ready')
                base.wait_for(lambda: readiness(case), child)
                if mode == 'success':
                    disable_tun(case)
                    receive(server, 'lease_released')
                    facts['all_children_exited'] &= stop_core(child)
                else:
                    command(server, 'drop_channel')
                    receive(server, 'channel_lost')
                    child.wait(timeout=5)
                    # Loss is deliberately NOT a clean DNS release. Depending
                    # on main's signal-handler startup, self-SIGTERM can be
                    # handled or terminate directly; both must stop the core.
                    facts['loss_stops_core'] &= child.returncode in (0, -signal.SIGTERM)
            facts['proof_holds_after_core_exit'] &= base.index('Meta') > 0
            command(server, 'drop_proof')
            receive(server, 'proof_dropped')
            server.wait(timeout=3)
            facts['all_children_exited'] &= server.returncode == 0
            facts['last_proof_close_removes_tun'] &= {
                item['ifname'] for item in base.ns.links()} == {'lo'}
            facts['no_core_dns_calls'] &= base.calls(case) == []
        finally:
            if child is not None:
                base.stop(child)
            base.stop(server)
            server.stdin.close()
            server.stdout.close()
    return {'coreSha256': core_digest, 'facts': facts}


def child(arguments):
    base.require(len(arguments) == 8)
    original = arguments[:4]
    guard(original)
    with tempfile.TemporaryDirectory(prefix='omavless-core-broker-') as directory:
        return experiment(Path(directory), Path(arguments[4]), arguments[5],
                          Path(arguments[6]), arguments[7], original)


def main():
    try:
        if len(sys.argv) == 10 and sys.argv[1] == '--isolated-child':
            result = child(sys.argv[2:])
        else:
            base.require(len(sys.argv) == 5 and os.geteuid() != 0)
            core, digest, fixture, fixture_digest = sys.argv[1:]
            ownership.validate_source(Path(core), digest)
            ownership.validate_source(Path(fixture), fixture_digest)
            original = [base.ns.namespace(name) for name in ('net', 'user', 'pid', 'mnt')]
            process = subprocess.run([
                '/usr/bin/unshare', '--user', '--map-root-user', '--net', '--pid',
                '--mount', '--propagation', 'private', '--fork', '--kill-child=SIGKILL',
                '--', sys.executable, str(Path(__file__).resolve()), '--isolated-child',
                *original, core, digest, fixture, fixture_digest,
            ], stdin=subprocess.DEVNULL, capture_output=True, timeout=60, check=False)
            base.require(process.returncode in (0, 1))
            result = base.project(process.stdout, FACTS)
            base.require(result['coreSha256'] == digest)
            base.require(process.returncode == (0 if all(result['facts'].values()) else 1))
        print(json.dumps(result, sort_keys=True))
        return 0 if all(result['facts'].values()) else 1
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError):
        print('{"isolated_core_broker_probe_failed":true}')
        return 1


if __name__ == '__main__':
    sys.exit(main())
