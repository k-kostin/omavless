#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Offline, unpublished pairing of a reviewed RC app, DNS package and frontend.

The two caller hashes are reviewed inputs, not signatures or build provenance.
No build, download, installation, service, VPN, tag or publication action occurs.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import selectors
import stat
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


release = module('dns_pair_release', ROOT / 'packaging/release/build-candidate.py')
pair = module('dns_pair_frontend', ROOT / 'packaging/release/pair-frontend.py')
inspection = module('dns_pair_inspection', ROOT / 'tests/installed_native_package.py')
stage = module('dns_pair_stage', ROOT / 'tests/dns_broker_host/package/stage.py')

SHA = r'[0-9a-f]{64}'
COMMIT = r'[0-9a-f]{40}'
DNS_MEMBER = {
    'mihomo': 'usr/lib/omavless-dns/mihomo',
    'omavless-dns-broker': 'usr/lib/omavless-dns/omavless-dns-broker',
    'package-guard': 'usr/lib/omavless-dns/package-guard',
    'omavless-dns-broker.service': 'usr/lib/systemd/system/omavless-dns-broker.service',
    'omavless-dns.hook': 'usr/share/libalpm/hooks/omavless-dns.hook',
    'omavless-dns.install': '.INSTALL',
    'source-receipt.json': 'usr/share/omavless-dns/source-receipt.json',
    'corresponding-source.tar.xz': 'usr/share/omavless-dns/corresponding-source.tar.xz',
    'mihomo.LICENSE': 'usr/share/licenses/omavless-dns/mihomo.LICENSE',
    'sing-tun.LICENSE': 'usr/share/licenses/omavless-dns/sing-tun.LICENSE',
    'omavless.LICENSE': 'usr/share/licenses/omavless-dns/omavless.LICENSE',
}
REVIEWED = 'usr/share/omavless-dns/reviewed-inputs.json'
METADATA = {'.BUILDINFO', '.MTREE', '.PKGINFO', REVIEWED}


def member(path, name, limit=8192, digest=False):
    """Bound extraction and reject subprocess failures without exposing payloads."""
    argv = ['/usr/bin/bsdtar', '-xOf', str(path), name]
    process = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                               stderr=subprocess.DEVNULL)
    hasher = hashlib.sha256()
    first = bytearray()
    content = bytearray()
    total = 0
    end = time.monotonic() + 30
    try:
        with selectors.DefaultSelector() as poll:
            poll.register(process.stdout, selectors.EVENT_READ)
            while True:
                remaining = end - time.monotonic()
                if remaining <= 0 or not poll.select(remaining):
                    raise ValueError('member timeout')
                chunk = os.read(process.stdout.fileno(), 65536)
                if not chunk:
                    break
                total += len(chunk)
                if total > limit:
                    raise ValueError('member bound')
                hasher.update(chunk)
                if digest:
                    if len(first) < 20:
                        first.extend(chunk[:20 - len(first)])
                else:
                    content.extend(chunk)
        if process.wait(timeout=max(0.01, end - time.monotonic())) != 0:
            raise ValueError('member extraction')
        return (hasher.hexdigest(), bytes(first)) if digest else bytes(content)
    finally:
        if process.poll() is None:
            process.kill()
        process.wait()
        process.stdout.close()


def metadata(raw):
    return json.loads(raw, object_pairs_hook=pair.unique_object)


def package_values(raw):
    values = {}
    for line in raw.decode('utf-8', 'strict').splitlines():
        if ' = ' not in line:
            continue
        key, value = line.split(' = ', 1)
        values.setdefault(key, []).append(value)
    for key in ('pkgname', 'pkgver', 'arch'):
        if len(values.get(key, [])) != 1:
            raise ValueError('package metadata')
    return values


def expected_members():
    files = set(DNS_MEMBER.values()) | METADATA
    directories = {name[:index + 1] for name in files for index, char in enumerate(name)
                   if char == '/'}
    return files | directories


def inspect_dns_archive(path, version, architecture, source):
    """Inspect the fixed production-name payload, not an experimental provider."""
    if path.suffixes[-3:] != ['.pkg', '.tar', '.zst']:
        raise ValueError('DNS archive type')
    names = inspection.capture(['/usr/bin/bsdtar', '-tf', str(path)], limit=16384).decode().splitlines()
    if len(names) != len(set(names)) or set(names) != expected_members():
        raise ValueError('DNS members')
    inspection.validate_listing(inspection.capture(
        ['/usr/bin/bsdtar', '-tvf', str(path)], limit=32768))
    pkg = package_values(member(path, '.PKGINFO', 16384))
    if (pkg['pkgname'] != ['omavless-dns'] or pkg['pkgver'] != [version + '-1']
            or pkg['arch'] != [architecture]):
        raise ValueError('DNS package identity')
    receipt = metadata(member(path, DNS_MEMBER['source-receipt.json']))
    manifest = metadata(member(path, REVIEWED))
    if (not isinstance(receipt, dict) or set(receipt) != stage.RELEASE_RECEIPT_KEYS
            or receipt.get('schema') != 1 or receipt.get('architecture') != architecture
            or receipt.get('omavless_commit') != source
            or receipt.get('package_flavor') != 'release'
            or receipt.get('broker_feature') != 'release-package'
            or receipt.get('mihomo_commit') != stage.PINNED_MIHOMO
            or receipt.get('mihomo_tag') != 'v1.19.31'
            or receipt.get('sing_tun_commit') != stage.PINNED_SING_TUN
            or receipt.get('sing_tun_tag') != 'v0.4.24'
            or receipt.get('patch_sha256') != stage.PINNED_PATCHES
            or receipt.get('go_build_tags') != 'with_gvisor'
            or receipt.get('go_dependency_mode') != 'vendor'
            or not isinstance(receipt.get('go_binary_sha256'), str)
            or not re.fullmatch(SHA, receipt['go_binary_sha256'])
            or not isinstance(receipt.get('cargo_lock_sha256'), str)
            or not re.fullmatch(SHA, receipt['cargo_lock_sha256'])
            or not isinstance(receipt.get('sha256'), dict)
            or set(receipt['sha256']) != set(stage.PAIR_FILES)
            or not isinstance(manifest, dict)
            or set(manifest) != {'schema', 'package', 'architecture', 'source_revision', 'sha256'}
            or manifest['schema'] != 3 or manifest['package'] != 'omavless-dns'
            or manifest['architecture'] != architecture or manifest['source_revision'] != source
            or not isinstance(manifest['sha256'], dict)
            or set(manifest['sha256']) != set(DNS_MEMBER)):
        raise ValueError('DNS receipt')
    binaries = {}
    for name, location in DNS_MEMBER.items():
        expected = manifest['sha256'][name]
        if not isinstance(expected, str) or not re.fullmatch(SHA, expected):
            raise ValueError('DNS digest shape')
        if name in ('mihomo', 'omavless-dns-broker', 'corresponding-source.tar.xz'):
            actual, first = member(path, location, 256 * 1024 * 1024, digest=True)
        else:
            raw = member(path, location, 65536)
            actual, first = hashlib.sha256(raw).hexdigest(), b''
        if actual != expected or (name in receipt['sha256'] and receipt['sha256'][name] != actual):
            raise ValueError('DNS payload digest')
        if name in ('mihomo', 'omavless-dns-broker'):
            machine = 62 if architecture == 'x86_64' else 183
            if first[:6] != b'\x7fELF\x02\x01' or int.from_bytes(first[18:20], 'little') != machine:
                raise ValueError('DNS binary architecture')
            binaries[name] = actual
    # Package scriptlet, hook and guard must be the committed reviewed source.
    for name, source_path in {
        'omavless-dns.install': 'packaging/dns/omavless-dns.install',
        'omavless-dns.hook': 'packaging/dns/omavless-dns.hook',
        'package-guard': 'tests/dns_broker_host/package/package-guard',
    }.items():
        committed = release.git(ROOT, 'show', source + ':' + source_path)
        if hashlib.sha256(committed).hexdigest() != manifest['sha256'][name]:
            raise ValueError('DNS privileged source mismatch')
    unit = release.git(ROOT, 'show', source + ':tests/dns_broker_host/omavless-dns-broker.service')
    old = b'ExecStart=/usr/lib/omavless/omavless-dns-broker --serve'
    if unit.count(old) != 1:
        raise ValueError('DNS unit source')
    unit = unit.replace(old, b'ExecStart=/usr/lib/omavless-dns/omavless-dns-broker --serve')
    unit = unit.replace(b'# REVIEW-ONLY opt-in candidate. Not installed or enabled by normal packages.\n',
                        b'# Installation alone does not enroll, enable, or start this service.\n')
    if hashlib.sha256(unit).hexdigest() != manifest['sha256']['omavless-dns-broker.service']:
        raise ValueError('DNS unit mismatch')
    return {'source': source, 'version': version + '-1', 'architecture': architecture,
            'coreSha256': binaries['mihomo'], 'brokerSha256': binaries['omavless-dns-broker'],
            'receiptSha256': hashlib.sha256(member(path, DNS_MEMBER['source-receipt.json'])).hexdigest()}


def pin_state(root, frontend_source, version, arch, app_sha, dns_sha, source):
    states = [pair.bootstrap_pins(root, frontend_source, version, arch, app_sha, source)]
    raw = release.git(root, 'show', frontend_source + ':plugin/dns-release.json')
    if len(raw) > 8192:
        raise ValueError('DNS pin bound')
    record = metadata(raw)
    if (not isinstance(record, dict) or set(record) != {'schemaVersion', 'version', 'packages'}
            or type(record['schemaVersion']) is not int or record['schemaVersion'] != 1
            or record['version'] != version or not isinstance(record['packages'], dict)
            or not set(record['packages']) <= {'x86_64', 'aarch64'}):
        raise ValueError('DNS pin shape')
    for value in record['packages'].values():
        if (not isinstance(value, dict) or set(value) != {'sha256', 'sourceCommit'}
                or not isinstance(value['sha256'], str) or not re.fullmatch(SHA, value['sha256'])
                or not isinstance(value['sourceCommit'], str)
                or not re.fullmatch(COMMIT, value['sourceCommit'])):
            raise ValueError('DNS pin shape')
    if not record['packages']:
        states.append('empty')
    elif record['packages'].get(arch) == {'sha256': dns_sha, 'sourceCommit': source}:
        states.append('matched')
    else:
        raise ValueError('DNS pin mismatch')
    if len(set(states)) != 1 or states[0] == 'absent':
        raise ValueError('partial bootstrap pin')
    return states[0]


def copy_verified(src, dst, expected, limit):
    total = 0
    digest = hashlib.sha256()
    with src.open('rb') as incoming, dst.open('xb') as outgoing:
        for chunk in iter(lambda: incoming.read(65536), b''):
            total += len(chunk)
            if total > limit:
                raise ValueError('package copy bound')
            digest.update(chunk)
            outgoing.write(chunk)
    dst.chmod(0o600)
    if digest.hexdigest() != expected:
        raise ValueError('package changed during copy')


def assemble(root, output, app, dns, frontend_source, app_sha, dns_sha,
             architecture=None):
    release.checked_source(root, frontend_source)
    version = release.version(root)
    # Inspection is offline: the selected architecture need not be the host's.
    # Installed acceptance still defaults to (and requires) the native arch.
    arch = os.uname().machine if architecture is None else architecture
    if arch not in ('x86_64', 'aarch64') or any(not re.fullmatch(SHA, value)
                                              for value in (app_sha, dns_sha)):
        raise ValueError('input identity')
    for path, expected in ((app, app_sha), (dns, dns_sha)):
        owner = path.lstat().st_uid
        if owner not in (0, os.getuid()) or inspection.fingerprint(path, owner)[0] != expected:
            raise ValueError('input hash')
    app_info = inspection.inspect_archive(app, arch)
    if app_info['version'] != version.replace('-rc.', 'rc') + '-1':
        raise ValueError('app version')
    source = app_info['source']
    if not re.fullmatch(COMMIT, source):
        raise ValueError('app source')
    pkg = package_values(member(app, '.PKGINFO', 16384))
    binary_sha, header = member(app, 'usr/bin/omavless', 256 * 1024 * 1024, digest=True)
    machine = 62 if arch == 'x86_64' else 183
    if (binary_sha != app_info['binary'] or header[:6] != b'\x7fELF\x02\x01'
            or int.from_bytes(header[18:20], 'little') != machine):
        raise ValueError('app binary architecture')
    if (pkg['arch'] != [arch]
            or 'omavless-dns=' + app_info['version'] not in pkg.get('depend', [])
            or any('mihomo' in value for value in pkg.get('depend', []))):
        raise ValueError('app companion dependency')
    dns_info = inspect_dns_archive(dns, version.replace('-rc.', 'rc'), arch, source)
    tree_sha = pair.equivalent_inputs(root, source, frontend_source)
    pins = pin_state(root, frontend_source, version, arch, app_sha, dns_sha, source)
    inspection.safe_parents(output, os.getuid())
    info = output.lstat()
    if (not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid()
            or output == root or root in output.parents or any(output.iterdir())):
        raise ValueError('unsafe output')
    output.chmod(0o700)
    package_version = version.replace('-rc.', 'rc') + '-1'
    app_name = f'omavless-{package_version}-{arch}.pkg.tar.zst'
    dns_name = f'omavless-dns-{package_version}-{arch}.pkg.tar.zst'
    copy_verified(app, output / app_name, app_sha, inspection.ARCHIVE_LIMIT)
    copy_verified(dns, output / dns_name, dns_sha, inspection.ARCHIVE_LIMIT)
    frontend_name = f'omavless-{version}-frontend.tar.xz'
    frontend = output / frontend_name
    epoch = int(release.git(root, 'show', '-s', '--format=%ct', frontend_source))
    release.frontend(root, frontend, frontend_source, version, epoch)
    release.checked_source(root, frontend_source)
    if (inspection.fingerprint(app, app.lstat().st_uid)[0] != app_sha
            or inspection.fingerprint(dns, dns.lstat().st_uid)[0] != dns_sha):
        raise ValueError('source archive changed')
    artifacts = {app_name: app_sha, dns_name: dns_sha, frontend_name: release.digest(frontend)}
    record = {'schemaVersion': 1, 'kind': 'managed-dns-release-triple',
              'version': version, 'architecture': arch, 'runtimeSourceCommit': source,
              'frontendSourceCommit': frontend_source, 'runtimeInputTreeSha256': tree_sha,
              'runtimeInputsEquivalent': True, 'bootstrapPins': pins,
              'dnsReceiptSha256': dns_info['receiptSha256'],
              'coreSha256': dns_info['coreSha256'], 'brokerSha256': dns_info['brokerSha256'],
              'publishedDownloadVerified': False, 'publication': 'unpublished-candidate',
              'artifacts': dict(artifacts)}
    record_path = output / 'managed-dns-pair.json'
    record_path.write_text(json.dumps(record, indent=2) + '\n')
    artifacts[record_path.name] = release.digest(record_path)
    (output / 'SHA256SUMS').write_text(''.join(f'{value}  {name}\n'
                                               for name, value in sorted(artifacts.items())))
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('application_package', type=Path)
    parser.add_argument('dns_package', type=Path)
    parser.add_argument('frontend_commit')
    parser.add_argument('application_sha256')
    parser.add_argument('dns_sha256')
    parser.add_argument('--arch', choices=('x86_64', 'aarch64'),
                        help='inspect this archive architecture without executing its binaries')
    args = parser.parse_args()
    try:
        assemble(ROOT, args.output, args.application_package, args.dns_package,
                 args.frontend_commit, args.application_sha256, args.dns_sha256,
                 args.arch)
    except (OSError, ValueError, TypeError, KeyError, UnicodeError,
            subprocess.SubprocessError, inspection.Refused):
        parser.exit(2, 'Managed DNS release pairing refused; nothing installed or published.\n')
    print('Unpublished managed DNS release triple assembled; installation is not verified.')


if __name__ == '__main__':
    main()
