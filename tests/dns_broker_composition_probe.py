#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Actual Rust Host/held TUN/private D-Bus composition. No host DNS effects."""
import os
from pathlib import Path
import subprocess
import sys

import dns_core_namespace_probe as base
import dns_core_ownership_probe as ownership


def isolated(arguments):
    base.require(len(arguments) == 7)
    original, binary, digest, case = arguments[:4], *arguments[4:]
    base.guard(*original[:3])
    base.require(base.ns.namespace('mnt') != original[3])
    base.require(case in ('success', 'denial', 'timeout', 'drop', 'policy', 'mismatch',
                         'apply_drift', 'release_drift', 'dns_drift', 'release_dns_drift'))
    ownership.validate_source(Path(binary), digest)
    subprocess.run(['/usr/bin/mount', '--make-rprivate', '/'], check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=3)
    subprocess.run(['/usr/bin/mount', '-t', 'tmpfs', '-o', 'mode=0755,size=4m',
                    'tmpfs', '/run'], check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=3)
    subprocess.run(['/usr/bin/mount', '-t', 'proc', '-o', 'nosuid,nodev,noexec',
                    'proc', '/proc'], check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=3)
    Path('/run/omavless-dns/private').mkdir(parents=True, mode=0o700)
    environment = {'PATH': '/usr/bin', 'OMAVLESS_TEST_CASE': case}
    environment.update({'OMAVLESS_TEST_NS_' + name.upper(): value
                        for name, value in zip(('net', 'user', 'pid', 'mnt'), original)})
    result = subprocess.run([binary, '--exact',
                             'transaction::integration::actual_host_composition',
                             '--ignored', '--nocapture'], env=environment,
                            stdin=subprocess.DEVNULL, capture_output=True, timeout=30)
    base.require(result.returncode == 0)


def main():
    try:
        if len(sys.argv) == 9 and sys.argv[1] == '--isolated-child':
            isolated(sys.argv[2:])
            return 0
        base.require(len(sys.argv) == 3 and os.geteuid() != 0)
        binary, digest = sys.argv[1:]
        ownership.validate_source(Path(binary), digest)
        original = [base.ns.namespace(name) for name in ('net', 'user', 'pid', 'mnt')]
        for case in ('success', 'denial', 'timeout', 'drop', 'policy', 'mismatch',
                     'apply_drift', 'release_drift', 'dns_drift', 'release_dns_drift'):
            result = subprocess.run([
                '/usr/bin/unshare', '--user', '--map-root-user', '--net', '--pid',
                '--mount', '--propagation', 'private', '--fork', '--kill-child=SIGKILL',
                '--', sys.executable, str(Path(__file__).resolve()), '--isolated-child',
                *original, binary, digest, case,
            ], stdin=subprocess.DEVNULL, capture_output=True, timeout=40)
            base.require(result.returncode == 0)
            print(case + ': PASS', flush=True)
        return 0
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError):
        print('isolated_broker_composition_failed')
        return 1


if __name__ == '__main__':
    sys.exit(main())
