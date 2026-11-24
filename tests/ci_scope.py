#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Select the PR Rust gate from exact committed trees; uncertainty means full."""
import os
from pathlib import Path
import re
import subprocess
import sys

# Only developer instructions, never user Help, compiled data or test/CI code.
# Remove a member here if it becomes a build/package input.
DEVELOPER_MARKDOWN = frozenset({
    'AGENTS.md', 'CONTRIBUTING.md',
    'docs/development/AGENT_GUIDE.md',
    'docs/development/AGENT_OPERATING_WORKFLOW.md',
    'docs/development/EXECUTION_POLICY.md',
    'docs/development/FEATURE_RESEARCH.md',
    'docs/development/README.md',
    'docs/roadmap/DEVELOPMENT_WORKFLOW.md',
    'docs/roadmap/ACCEPTANCE_ENVIRONMENTS.md',
})
SHA = re.compile(r'[0-9a-f]{40}')


def git(root, *args):
    env = {'PATH': os.defpath, 'HOME': str(Path.home()), 'LC_ALL': 'C',
           'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': os.devnull,
           'GIT_NO_LAZY_FETCH': 'true', 'GIT_TERMINAL_PROMPT': '0'}
    return subprocess.check_output(
        ['git', '-c', 'gc.auto=0', '-c', 'maintenance.auto=false',
         '-c', 'core.hooksPath=/dev/null', '-C', str(root), *args],
        env=env, timeout=30, stderr=subprocess.DEVNULL)


def rust_required(root, base, head):
    try:
        if not SHA.fullmatch(base) or not SHA.fullmatch(head):
            return True, 'invalid_commit_identity'
        if (git(root, 'rev-parse', '--show-toplevel').decode().strip() != str(root)
                or git(root, 'rev-parse', 'HEAD').decode().strip() != head
                or git(root, 'rev-parse', base + '^{commit}').decode().strip() != base):
            return True, 'checkout_identity_mismatch'
        # Direct tree comparison includes changes since the branch diverged.
        # A merge-base diff could hide newer product code in the event base.
        diff = git(root, 'diff', '--raw', '--no-abbrev', '-z', '--no-renames',
                   '--no-ext-diff', base, head, '--')
        if not diff or len(diff) > 65536 or not diff.endswith(b'\0'):
            return True, 'empty_or_unbounded_diff'
        records = diff[:-1].split(b'\0')
        if len(records) % 2:
            return True, 'invalid_diff'
        for metadata, name in zip(records[::2], records[1::2]):
            old_mode, new_mode, old_oid, new_oid, status = metadata.decode('ascii').split()
            if (old_mode not in (':100644', ':000000')
                    or new_mode not in ('100644', '000000')
                    or not SHA.fullmatch(old_oid) or not SHA.fullmatch(new_oid)
                    or status not in ('A', 'D', 'M')
                    or name.decode('utf-8') not in DEVELOPER_MARKDOWN):
                return True, 'outside_developer_markdown'
        return False, 'developer_markdown_only'
    except (OSError, ValueError, UnicodeError, subprocess.SubprocessError):
        return True, 'metadata_unavailable'


if __name__ == '__main__':
    required, reason = (rust_required(Path.cwd().resolve(), *sys.argv[1:])
                        if len(sys.argv) == 3 else (True, 'invalid_arguments'))
    print('rust_required=' + str(required).lower())
    print('reason=' + reason)
