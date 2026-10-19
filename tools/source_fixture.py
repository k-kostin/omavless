#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Prepare an offline exact-source checkout and EMPTY distribution directory.

No source imports, exporter, build, API, credentials, VM or installed actions.
This is fixture preparation, never source-CI/bootstrap or product acceptance.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def git(root, *args):
    # A local-only fixture must not inherit hooks, auth helpers, global filters,
    # GIT_DIR overrides or automatic maintenance from the operator's shell.
    env = {'PATH': os.defpath, 'HOME': str(Path.home()), 'LC_ALL': 'C',
           'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': os.devnull}
    return subprocess.check_output(
        ['git', '-c', 'gc.auto=0', '-c', 'maintenance.auto=false',
         '-c', 'core.hooksPath=/dev/null', '-c', 'init.templateDir=',
         '-c', 'core.fsmonitor=false', '-C', str(root), *args],
        env=env, timeout=30, stderr=subprocess.DEVNULL)


def checked_parent(parent):
    if not parent.is_absolute() or parent.resolve(strict=True) != parent:
        raise ValueError('fixture parent redirected')
    for path in (parent, *parent.parents):
        info = path.lstat()
        if not stat.S_ISDIR(info.st_mode) or info.st_uid not in (0, os.getuid()) or info.st_mode & 0o022:
            raise ValueError('fixture parent unsafe')
    cache = Path.home() / '.cache/ovt'
    if (parent.stat().st_uid != os.getuid() or Path.home().resolve() not in parent.parents
            or (parent != cache and cache not in parent.parents)):
        raise ValueError('fixture must use owned HOME-backed cache')


def inspect_source(root, sha):
    if git(root, 'rev-parse', 'HEAD').decode().strip() != sha:
        raise ValueError('source head mismatch')
    if git(root, 'status', '--porcelain=v1', '--untracked-files=all'):
        raise ValueError('source checkout not clean')
    count = 0
    for item in git(root, 'ls-tree', '-rz', '--full-tree', sha).split(b'\0'):
        if not item:
            continue
        meta, name = item.split(b'\t', 1)
        mode, kind, oid = meta.decode().split()
        if kind != 'blob' or mode not in ('100644', '100755'):
            raise ValueError('source must contain regular files only')
        path = root / name.decode('utf-8', 'strict')
        info = path.lstat()
        if not stat.S_ISREG(info.st_mode) or stat.S_IMODE(info.st_mode) != int(mode, 8) & 0o777:
            raise ValueError('source file mode mismatch')
        data = path.read_bytes()
        object_id = hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data,
                                usedforsecurity=False).hexdigest()
        if object_id != oid:
            raise ValueError('source bytes mismatch')
        count += 1
    for path in root.rglob('*'):
        relative = path.relative_to(root)
        if relative.parts[0] == '.git':
            continue
        if ('__pycache__' in relative.parts or path.suffix in ('.pyc', '.pyo')
                or relative.parts[0] in ('target', 'build', '.venv') or path.is_symlink()):
            raise ValueError('source must be fresh before admission')
    return count


def prepare(repository, sha, parent):
    if not re.fullmatch('[0-9a-f]{40}', sha):
        raise ValueError('full immutable source commit required')
    if (repository.resolve(strict=True) != repository
            or git(repository, 'rev-parse', '--show-toplevel').decode().strip() != str(repository)
            or git(repository, 'rev-parse', sha + '^{commit}').decode().strip() != sha):
        raise ValueError('source repository or commit mismatch')
    checked_parent(parent)
    # Never reuse/repair an existing fixture or delete a partial/failed one.
    workspace = Path(tempfile.mkdtemp(prefix='source-fixture-', dir=parent))
    source = workspace / 'source'
    distribution = workspace / 'distribution'
    previous = os.umask(0o022)
    try:
        git(workspace, 'clone', '--no-local', '--no-checkout', str(repository), str(source))
        # A worktree HEAD may precede the selected merged object. Clone alone
        # does not advertise its remote-tracking refs; fetch the exact object.
        git(source, 'fetch', '--no-tags', 'origin', sha)
        if git(source, 'rev-parse', 'FETCH_HEAD').decode().strip() != sha:
            raise ValueError('fetched source identity mismatch')
        git(source, 'checkout', '--detach', sha)
        count = inspect_source(source, sha)
        distribution.mkdir(mode=0o700)
    finally:
        os.umask(previous)
    return {'schemaVersion': 1, 'source': str(source), 'distribution': str(distribution),
            'sourceCommit': sha, 'sourceTree': git(source, 'rev-parse', 'HEAD^{tree}').decode().strip(),
            'regularFiles': count, 'modes': '644/755', 'sourceImportsOrBuilds': False,
            'bytecodeAbsent': True, 'distributionEmpty': True,
            'localDistributionCommitConfig': ['-c', 'user.name=OmaVLESS',
                                              '-c', 'user.email=devnull@example.invalid'],
            'admission': 'NOT RUN: actual export/offline/source-CI/generated bootstrap remain required'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source_commit')
    parser.add_argument('--parent', type=Path, default=Path.home() / '.cache/ovt')
    args = parser.parse_args()
    try:
        print(json.dumps(prepare(ROOT, args.source_commit, args.parent), sort_keys=True))
    except (OSError, ValueError, subprocess.SubprocessError):
        parser.exit(2, 'Source fixture refused; no existing fixture was reused or repaired.\n')


if __name__ == '__main__':
    main()
