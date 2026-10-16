#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Admit reviewed source validator bytes BEFORE executing candidate code."""
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys

FILES = ('packaging/release/export-distribution.py',
         'packaging/release/pair-frontend.py', 'packaging/release/build-candidate.py',
         'tests/installed_native_package.py', 'tests/human_authorization.py')


def admit(source, trusted):
    if not source.is_absolute() or source.resolve(strict=True) != source:
        raise ValueError('redirected source directory')
    if set(trusted) != set(FILES):
        raise ValueError('validator trust inventory differs')
    for name in FILES:
        expected = trusted[name]
        if not isinstance(expected, str) or not re.fullmatch('[0-9a-f]{64}', expected):
            raise ValueError('invalid validator pin')
        path = source / name
        # A matching source hash must not admit a previously planted .pyc.
        # The fresh CI source checkout has no imported source modules yet.
        if (path.parent / '__pycache__').exists() or path.with_suffix('.pyc').exists():
            raise ValueError('source bytecode cache must be absent before admission')
        for parent in path.parents:
            if parent == source:
                break
            if not stat.S_ISDIR(parent.lstat().st_mode):
                raise ValueError('redirected source parent')
        info = path.lstat()
        if not stat.S_ISREG(info.st_mode) or info.st_size > 1024 * 1024:
            raise ValueError('invalid source validator member')
        if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
            raise ValueError('source validator does not match reviewed bytes')


def main():
    if len(sys.argv) != 3:
        raise ValueError('source and distribution directories required')
    source, distribution = map(Path, sys.argv[1:])
    # This trust table belongs to the reviewed distribution, NOT its mutable
    # distribution.json or the candidate source being admitted.
    table = distribution / '.github/source-validator-pins.json'
    if not stat.S_ISREG(table.lstat().st_mode) or table.stat().st_size > 4096:
        raise ValueError('invalid validator trust table')
    trusted = json.loads(table.read_text())
    admit(source, trusted)
    # Isolated mode prevents candidate json.py/etc and PYTHONPATH/user-site
    # imports from shadowing the standard library after hashes were checked.
    os.execv(sys.executable, [sys.executable, '-I', '-B', str(source / FILES[0]),
                             'verify', str(distribution), '--source-ci'])


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, TypeError) as error:
        sys.exit('Source validator refused: ' + str(error))
