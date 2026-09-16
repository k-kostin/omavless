#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Offline developer assembly: reuse an accepted package with a newer frontend.

No build, download, install, service, store, tag or publication operation.
The supplied package checksum is a reviewed input, not a signature or build proof.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


release = module('pair_release', ROOT / 'packaging/release/build-candidate.py')
inspection = module('pair_inspection', ROOT / 'tests/installed_native_package.py')

# These are NOT runtime/build/package inputs in the accepted tree. Fail closed
# for every other path, including new root files, templates, .cargo, crates,
# Cargo.lock, toolchain, systemd, Arch packaging and payload licenses/notices.
FRONTEND_FILES = frozenset(('README.md', 'AGENTS.md', 'CONTRIBUTING.md', 'DEVELOPMENT_ROADMAP.md',
                            'manifest.json', 'preview.png', 'backend.sh', 'install.sh'))
FRONTEND_DIRS = ('plugin/', 'docs/', 'tests/', 'skills/', 'packaging/release/')


def runtime_tree(root, commit):
    if not re.fullmatch(r'[0-9a-f]{40}', commit):
        raise ValueError('source identity')
    entries = []
    for record in release.git(root, 'ls-tree', '-rz', '--full-tree', commit).split(b'\0'):
        if not record:
            continue
        meta, raw_path = record.split(b'\t', 1)
        path = raw_path.decode('utf-8', 'strict')
        if path in FRONTEND_FILES or path.startswith(FRONTEND_DIRS):
            continue
        entries.append(record)
    return b'\0'.join(entries)


def equivalent_inputs(root, runtime_source, frontend_source):
    before, after = runtime_tree(root, runtime_source), runtime_tree(root, frontend_source)
    if before != after:
        raise ValueError('runtime inputs changed')
    release.git(root, 'merge-base', '--is-ancestor', runtime_source, frontend_source)
    return hashlib.sha256(before).hexdigest()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate release field')
        result[key] = value
    return result


def bootstrap_pins(root, source, version, architecture, package_sha, runtime_source):
    path = 'plugin/runtime-release.json'
    if not release.git(root, 'ls-tree', source, '--', path):
        return 'absent'
    raw = release.git(root, 'show', source + ':' + path)
    if len(raw) > 8192:
        raise ValueError('release bounds')
    record = json.loads(raw, object_pairs_hook=unique_object)
    if (not isinstance(record, dict) or set(record) != {'schemaVersion', 'version', 'packages'}
            or type(record['schemaVersion']) is not int or record['schemaVersion'] != 1
            or record['version'] != version or not isinstance(record['packages'], dict)
            or not set(record['packages']) <= {'aarch64', 'x86_64'}):
        raise ValueError('release shape')
    for value in record['packages'].values():
        if (not isinstance(value, dict) or set(value) != {'sha256', 'sourceCommit'}
                or not isinstance(value['sha256'], str) or not re.fullmatch(r'[0-9a-f]{64}', value['sha256'])
                or not isinstance(value['sourceCommit'], str) or not re.fullmatch(r'[0-9a-f]{40}', value['sourceCommit'])):
            raise ValueError('release pin')
    if not record['packages']:
        return 'empty'
    if record['packages'].get(architecture) != {'sha256':package_sha, 'sourceCommit':runtime_source}:
        raise ValueError('package pin mismatch')
    return 'matched'


def assemble(root, output, package, expected, package_sha):
    release.checked_source(root, expected)
    version = release.version(root, stable=True)
    if not re.fullmatch(r'[0-9a-f]{64}', package_sha):
        raise ValueError('package hash')
    # Reuse the existing no-symlink/owner/mode/256-MiB bounded inspector. Check
    # the reviewed archive hash BEFORE any decompression or package parsing.
    owner = package.lstat().st_uid
    if owner not in (0, os.getuid()):
        raise ValueError('package owner')
    original = inspection.fingerprint(package, owner)
    if original[0] != package_sha:
        raise ValueError('package hash mismatch')
    info = inspection.inspect_archive(package)
    architecture = os.uname().machine
    if architecture not in ('aarch64', 'x86_64') or info['version'] != version + '-1':
        raise ValueError('package version')
    input_sha = equivalent_inputs(root, info['source'], expected)
    pins = bootstrap_pins(root, expected, version, architecture, package_sha, info['source'])
    inspection.safe_parents(output, os.getuid())
    out_info = output.lstat()
    if (not stat.S_ISDIR(out_info.st_mode) or out_info.st_uid != os.getuid()
            or output == root or root in output.parents or any(output.iterdir())):
        raise ValueError('unsafe output')
    output.chmod(0o700)
    package_name = f'omavless-{version}-1-{architecture}.pkg.tar.zst'
    package_copy = output / package_name
    with package.open('rb') as src, package_copy.open('xb') as dst:
        total = 0
        while chunk := src.read(65536):
            total += len(chunk)
            if total > inspection.ARCHIVE_LIMIT:
                raise ValueError('package copy bound')
            dst.write(chunk)
    package_copy.chmod(0o600)
    if release.digest(package_copy) != package_sha:
        raise ValueError('package changed')
    frontend = output / f'omavless-{version}-frontend.tar.xz'
    epoch = int(release.git(root, 'show', '-s', '--format=%ct', expected))
    release.frontend(root, frontend, expected, version, epoch)
    release.checked_source(root, expected)
    if inspection.fingerprint(package, owner) != original:
        raise ValueError('package changed')
    artifacts = {package_name:package_sha, frontend.name:release.digest(frontend)}
    identity = {'schemaVersion':1, 'kind':'native-frontend-pair', 'version':version,
        'runtimeSourceCommit':info['source'], 'frontendSourceCommit':expected,
        'runtimeInputTreeSha256':input_sha, 'runtimeInputsEquivalent':True,
        'architecture':architecture, 'binarySha256':info['binary'],
        'bootstrapPins':pins, 'publishedDownloadVerified':False,
        'provenance':'reused-reviewed-package; original build evidence required',
        'publication':'unpublished-candidate', 'artifacts':dict(artifacts)}
    record = output / 'frontend-pair.json'
    record.write_text(json.dumps(identity, indent=2) + '\n')
    artifacts[record.name] = release.digest(record)
    (output / 'SHA256SUMS').write_text(''.join(f'{sha}  {name}\n' for name, sha in sorted(artifacts.items())))
    return identity


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('package', type=Path)
    parser.add_argument('frontend_commit')
    parser.add_argument('package_sha256')
    args = parser.parse_args()
    try:
        assemble(ROOT, args.output, args.package, args.frontend_commit, args.package_sha256)
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError, inspection.Refused):
        parser.exit(2, 'Frontend pairing refused or failed; nothing was installed or published.\n')
    print('Offline frontend pair assembled; publication and installation are not verified.')


if __name__ == '__main__':
    main()
