#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Opt-in developer core test; no installed replacement or real DNS effects.

Usage: python3 tests/dns_core_ownership_probe.py /absolute/test/core SHA256
Runs only in fresh user/net/PID namespaces. Never use as a production launcher.
"""
import hashlib
import fcntl
import json
import os
import platform
from pathlib import Path
import shutil
import stat
import struct
import subprocess
import sys
import tempfile
import time

import dns_core_namespace_probe as base
import dns_tun_fd_policy as fd_policy

FACTS = {
    'isolated', 'omitted_keeps_legacy', 'false_keeps_legacy',
    'true_suppresses_setup_and_close', 'changed_tun_has_no_dns_calls',
    'true_to_false_restores_core_writer', 'false_to_true_releases_core_writer',
    'controller_confirms_flag', 'all_owned_children_exited',
    'fd_false_reverts_on_close', 'fd_true_suppresses_close',
    'restricted_fd_core_ready', 'restricted_fd_core_clean_close',
}


class UnsupportedCore(RuntimeError):
    pass


def validate_source(path, digest):
    base.require(path.is_absolute() and len(digest) == 64
                 and all(c in '0123456789abcdef' for c in digest))
    metadata = path.lstat()
    base.require(stat.S_ISREG(metadata.st_mode) and metadata.st_uid in (0, os.getuid())
                 and not metadata.st_mode & 0o022 and 0 < metadata.st_size <= 128 * 1024 * 1024)
    base.require(hashlib.sha256(path.read_bytes()).hexdigest() == digest)


def config(root, device, disabled):
    base.require(disabled is None or type(disabled) is bool)
    text = base.config(root, device, 'direct')
    if disabled is not None:
        text = text.replace('tun:\n', 'tun:\n  disable-system-dns: '
                            + ('true' if disabled else 'false') + '\n', 1)
    return text


def flag(root):
    value = json.loads(base.request(root, 'GET', '/configs')).get('tun', {})
    value = value.get('disable-system-dns')
    if type(value) is not bool:
        raise UnsupportedCore('unsupported_dns_ownership_capability')
    return value


def wait_ready(root, child, fd=None):
    def ready():
        base.request(root, 'GET', '/version')
        base.index('ovdnsprobe0')
        tun = json.loads(base.request(root, 'GET', '/configs')).get('tun', {})
        return tun.get('enable') is True and (fd is None or tun.get('file-descriptor') == fd)
    base.wait_for(ready, child)


def stable_calls(root, expected):
    # Bounded observation only, not a claim about arbitrary delayed OS work.
    time.sleep(1)
    return base.calls(root) == expected


def stop_cleanly(child):
    # SIGKILL/early SIGTERM absence of DNS calls does not exercise core Close.
    return base.stop(child) and child.returncode == 0


def launch(root, binary, disabled, fd=None, restricted=False):
    base.require(fd is None or (type(fd) is int and fd > 2))
    base.require(type(restricted) is bool and (not restricted or fd is not None))
    (root / 'resolvectl').write_text(
        '#!' + sys.executable + '\nfrom pathlib import Path\nimport sys\n'
        'if len(sys.argv)<3 or sys.argv[1] not in '
        '("domain","default-route","dns","revert") or sys.argv[2] not in '
        '("ovdnsprobe0","ovdnsprobe1"): sys.exit(2)\n'
        'with open(Path(__file__).with_name("calls"), "a") as f: f.write(sys.argv[1]+"\\n")\n')
    (root / 'resolvectl').chmod(0o700)
    path = root / 'config.yaml'
    text = config(root, 'ovdnsprobe0', disabled)
    if fd is not None:
        text = text.replace('tun:\n', 'tun:\n  file-descriptor: ' + str(fd) + '\n', 1)
    path.write_text(text)
    command = [str(binary), '-d', str(root), '-f', str(path)]
    if restricted:
        filter_path = root / 'ioctl-filter.bpf'
        filter_path.write_bytes(fd_policy.encode(platform.machine()))
        wrapper = root / 'restricted_exec.py'
        wrapper.write_text(
            'import os, sys\nfrom pathlib import Path\n'
            'sys.path.insert(0, ' + repr(str(Path(__file__).resolve().parent)) + ')\n'
            'from dns_tun_authority_probe import dropped_capabilities\n'
            'dropped_capabilities()\n'
            'Path(' + repr(str(root / 'capabilities-checked')) + ').write_bytes(b"checked")\n'
            'os.execve(' + repr(str(binary)) + ', ' + repr(command) + ', dict(os.environ))\n')
        command = ['/usr/bin/setpriv', '--bounding-set=-all', '--inh-caps=-all',
                   '--ambient-caps=-all', '--no-new-privs', '--seccomp-filter',
                   str(filter_path), '--', sys.executable, str(wrapper)]
    return subprocess.Popen(command, env={
        'PATH': str(root), 'HOME': str(root), 'XDG_RUNTIME_DIR': str(root), 'LANG': 'C',
        'DBUS_SYSTEM_BUS_ADDRESS': 'unix:path=' + str(root / 'absent-system-bus'),
        'DBUS_SESSION_BUS_ADDRESS': 'unix:path=' + str(root / 'absent-session-bus'),
    }, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        pass_fds=() if fd is None else (fd,))


def reload(root, device, disabled):
    path = root / 'config.yaml'
    path.write_text(config(root, device, disabled))
    base.request(root, 'PUT', '/configs?force=true', {'path': str(path)})


def experiment(root, source, digest):
    binary = root / 'candidate'
    shutil.copyfile(source, binary)
    binary.chmod(0o700)
    base.require(not os.listxattr(binary))
    base.require(hashlib.sha256(binary.read_bytes()).hexdigest() == digest)
    facts = {'isolated': True, 'controller_confirms_flag': True, 'all_owned_children_exited': True}
    setup = ['domain', 'default-route', 'dns']
    for name, disabled in [('omitted', None), ('false', False), ('true', True)]:
        case = root / name
        case.mkdir(mode=0o700)
        child = launch(case, binary, disabled)
        try:
            wait_ready(case, child)
            facts['controller_confirms_flag'] &= flag(case) == bool(disabled)
            if not disabled:
                base.wait_for(lambda: base.calls(case) == setup, child)
            else:
                base.require(stable_calls(case, []))
                reload(case, 'ovdnsprobe1', True)
                base.wait_for(lambda: base.index('ovdnsprobe1') > 0, child)
                facts['changed_tun_has_no_dns_calls'] = stable_calls(case, []) and flag(case)
        finally:
            facts['all_owned_children_exited'] &= stop_cleanly(child)
        facts[{'omitted': 'omitted_keeps_legacy', 'false': 'false_keeps_legacy',
               'true': 'true_suppresses_setup_and_close'}[name]] = stable_calls(
                   case, [] if disabled else setup + ['revert'])
        base.require({item['ifname'] for item in base.ns.links()} == {'lo'})

    case = root / 'toggle'
    case.mkdir(mode=0o700)
    child = launch(case, binary, True)
    try:
        wait_ready(case, child)
        base.require(flag(case) and stable_calls(case, []))
        reload(case, 'ovdnsprobe0', False)
        base.wait_for(lambda: base.calls(case) == setup, child)
        facts['true_to_false_restores_core_writer'] = not flag(case)
        reload(case, 'ovdnsprobe0', True)
        base.wait_for(lambda: base.calls(case) == setup + ['revert'], child)
        facts['false_to_true_releases_core_writer'] = flag(case) and stable_calls(case, setup + ['revert'])
    finally:
        facts['all_owned_children_exited'] &= stop_cleanly(child)
    facts['true_suppresses_setup_and_close'] &= stable_calls(case, setup + ['revert'])
    base.require({item['ifname'] for item in base.ns.links()} == {'lo'})
    for disabled, restricted in ((False, False), (True, False), (True, True)):
        case = root / ('fd-restricted' if restricted else 'fd-true' if disabled else 'fd-false')
        case.mkdir(mode=0o700)
        fd = os.open('/dev/net/tun', os.O_RDWR | os.O_CLOEXEC | os.O_NONBLOCK)
        try:
            fcntl.ioctl(fd, base.ns.TUNSETIFF,
                        struct.pack('16sH22x', b'ovdnsprobe0', base.ns.FLAGS))
            # FD mode skips all core configure work. The test creator must set
            # up its own link; this is NOT a production host helper.
            for args in [('address', 'add', '198.18.0.1/30', 'dev', 'ovdnsprobe0'),
                         ('link', 'set', 'dev', 'ovdnsprobe0', 'up')]:
                subprocess.run(['/usr/bin/ip', *args], stdin=subprocess.DEVNULL,
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                               timeout=3, check=True)
            child = launch(case, binary, disabled, fd=fd, restricted=restricted)
            try:
                wait_ready(case, child, fd)
                base.require(flag(case) == disabled and stable_calls(case, []))
                if restricted:
                    facts['restricted_fd_core_ready'] = (case / 'capabilities-checked').read_bytes() == b'checked'
            finally:
                facts['all_owned_children_exited'] &= stop_cleanly(child)
            key = 'restricted_fd_core_clean_close' if restricted else 'fd_true_suppresses_close' if disabled else 'fd_false_reverts_on_close'
            facts[key] = child.returncode == 0 and stable_calls(
                case, [] if disabled else ['revert'])
        finally:
            os.close(fd)
        base.require({item['ifname'] for item in base.ns.links()} == {'lo'})
    return {'facts': facts, 'coreSha256': digest}


def child(original_net, original_user, original_pid, source, digest):
    base.guard(original_net, original_user, original_pid)
    os.umask(0o077)
    with tempfile.TemporaryDirectory(prefix='omavless-dns-owner.') as directory:
        return experiment(Path(directory), Path(source), digest)


def project(raw, digest):
    # Reuse the strict bounded, duplicate-key rejecting result decoder.
    value = base.project(raw, FACTS)
    base.require(value['coreSha256'] == digest)
    return value


def main():
    if len(sys.argv) == 7 and sys.argv[1] == '--isolated-child':
        try:
            print(json.dumps(child(*sys.argv[2:]), sort_keys=True))
            return 0
        except UnsupportedCore:
            print('{"unsupported_dns_ownership_capability":true}')
            return 1
        except Exception:
            print('{"isolated_dns_ownership_probe_failed":true}')
            return 1
    if len(sys.argv) != 3 or os.geteuid() == 0:
        print('{"unprivileged_candidate_and_digest_required":true}')
        return 2
    try:
        source, digest = Path(sys.argv[1]), sys.argv[2]
        validate_source(source, digest)
        result = subprocess.run([
            '/usr/bin/unshare', '--user', '--map-root-user', '--net', '--pid',
            '--fork', '--kill-child=SIGKILL', '--', sys.executable,
            str(Path(__file__).resolve()), '--isolated-child',
            base.ns.namespace('net'), base.ns.namespace('user'), base.ns.namespace('pid'),
            str(source), digest,
        ], stdin=subprocess.DEVNULL, capture_output=True, timeout=60, check=False)
        if result.returncode == 1 and result.stdout == b'{"unsupported_dns_ownership_capability":true}\n':
            print('{"unsupported_dns_ownership_capability":true}')
            return 1
        base.require(result.returncode == 0)
        value = project(result.stdout, digest)
        print(json.dumps(value, sort_keys=True))
        return 0 if all(value['facts'].values()) else 1
    except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired):
        print('{"isolated_dns_ownership_probe_unavailable":true}')
        return 1


if __name__ == '__main__':
    sys.exit(main())
