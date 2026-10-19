# SPDX-License-Identifier: MIT
"""Offline source fixture preparation; no API/auth/build/VM or old fixtures."""
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('source_fixture', ROOT / 'packaging/release/source-fixture.py')
FIXTURE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(FIXTURE)


class SourceFixtureTests(unittest.TestCase):
    def setUp(self):
        cache = Path.home() / '.cache/ovt'
        cache.mkdir(mode=0o700, parents=True, exist_ok=True)
        self.tmp = tempfile.TemporaryDirectory(prefix='source-prep-test-', dir=cache)
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.repo = self.base / 'repository'
        self.repo.mkdir()
        FIXTURE.git(self.repo, 'init', '-q')
        FIXTURE.git(self.repo, 'config', 'user.name', 'Fixture')
        FIXTURE.git(self.repo, 'config', 'user.email', 'fixture@example.invalid')
        (self.repo / 'plain').write_text('ordinary source\n')
        (self.repo / 'executable').write_text('#!/bin/sh\nexit 0\n')
        (self.repo / 'executable').chmod(0o755)
        FIXTURE.git(self.repo, 'add', '.')
        FIXTURE.git(self.repo, 'commit', '-qm', 'first')
        self.first = FIXTURE.git(self.repo, 'rev-parse', 'HEAD').decode().strip()

    def test_real_exact_checkout_is_clean_private_and_no_admission_is_claimed(self):
        # Even a restrictive caller umask must not produce source600/700.
        previous = os.umask(0o077)
        try:
            result = FIXTURE.prepare(self.repo, self.first, self.base)
            self.assertEqual(os.umask(0o077), 0o077)
        finally:
            os.umask(previous)
        self.assertEqual(result['sourceCommit'], self.first)
        self.assertEqual(result['regularFiles'], 2)
        self.assertEqual(result['modes'], '644/755')
        self.assertFalse(result['sourceImportsOrBuilds'])
        self.assertTrue(result['bytecodeAbsent'])
        self.assertIn('NOT RUN', result['admission'])
        self.assertEqual(list(Path(result['distribution']).iterdir()), [])
        self.assertEqual(Path(result['source']).parent.stat().st_mode & 0o777, 0o700)
        self.assertEqual(FIXTURE.git(self.repo, 'rev-parse', 'HEAD').decode().strip(), self.first)

    def test_selected_merged_object_need_not_be_the_clone_default_head(self):
        (self.repo / 'later').write_text('selected later object\n')
        FIXTURE.git(self.repo, 'add', '.')
        FIXTURE.git(self.repo, 'commit', '-qm', 'later')
        later = FIXTURE.git(self.repo, 'rev-parse', 'HEAD').decode().strip()
        FIXTURE.git(self.repo, 'update-ref', 'refs/remotes/origin/development', later)
        FIXTURE.git(self.repo, 'checkout', '--detach', self.first)
        result = FIXTURE.prepare(self.repo, later, self.base)
        self.assertEqual(result['sourceCommit'], later)
        self.assertEqual((Path(result['source']) / 'later').read_text(), 'selected later object\n')
        self.assertEqual(FIXTURE.git(self.repo, 'rev-parse', 'HEAD').decode().strip(), self.first)

    def test_missing_commit_and_unsafe_parent_refuse_without_new_workspace(self):
        before = set(self.base.iterdir())
        for sha in ('HEAD', '0' * 40):
            with self.subTest(sha=sha), self.assertRaises((ValueError, OSError)):
                # CalledProcessError is deliberately handled below as a Git refusal.
                try:
                    FIXTURE.prepare(self.repo, sha, self.base)
                except FIXTURE.subprocess.SubprocessError as error:
                    raise ValueError('Git refusal') from error
        self.assertEqual(set(self.base.iterdir()), before)
        unsafe = self.base / 'unsafe'
        unsafe.mkdir()
        unsafe.chmod(0o777)
        with self.assertRaises(ValueError):
            FIXTURE.prepare(self.repo, self.first, unsafe)
        self.assertEqual(list(unsafe.iterdir()), [])
        link = self.base / 'redirect'
        link.symlink_to(self.base, target_is_directory=True)
        with self.assertRaises(ValueError):
            FIXTURE.prepare(self.repo, self.first, link)

    def test_source_bytecode_or_nonregular_members_refuse(self):
        cache = self.repo / '__pycache__'
        cache.mkdir()
        (cache / 'candidate.pyc').write_bytes(b'synthetic bytecode')
        FIXTURE.git(self.repo, 'add', '.')
        FIXTURE.git(self.repo, 'commit', '-qm', 'forbidden bytecode')
        sha = FIXTURE.git(self.repo, 'rev-parse', 'HEAD').decode().strip()
        with self.assertRaisesRegex(ValueError, 'fresh before admission'):
            FIXTURE.prepare(self.repo, sha, self.base)
        # A separate source graph control, not reuse of the refused output.
        (self.repo / 'redirect').symlink_to('plain')
        FIXTURE.git(self.repo, 'add', '.')
        FIXTURE.git(self.repo, 'commit', '-qm', 'forbidden source link')
        sha = FIXTURE.git(self.repo, 'rev-parse', 'HEAD').decode().strip()
        with self.assertRaisesRegex(ValueError, 'regular files only'):
            FIXTURE.prepare(self.repo, sha, self.base)

    def test_git_drops_ambient_overrides_and_authentication(self):
        with patch.dict(os.environ, {'GIT_DIR': '/nonexistent', 'GIT_CONFIG_COUNT': '1',
                                    'GIT_CONFIG_KEY_0': 'alias.status', 'GIT_CONFIG_VALUE_0': '!exit 99',
                                    'GH_TOKEN': 'SYNTHETIC_UNUSED_AUTH'}):
            result = FIXTURE.prepare(self.repo, self.first, self.base)
        self.assertEqual(result['sourceCommit'], self.first)
        self.assertEqual(result['localDistributionCommitConfig'],
                         ['-c', 'user.name=OmaVLESS', '-c', 'user.email=devnull@example.invalid'])
        with patch.object(FIXTURE.subprocess, 'check_output', return_value=b'') as command:
            FIXTURE.git(self.repo, 'status', '--porcelain=v1')
        env = command.call_args.kwargs['env']
        self.assertEqual(env['GIT_NO_LAZY_FETCH'], 'true')
        self.assertEqual(env['GIT_ALLOW_PROTOCOL'], 'file')
        self.assertEqual(env['GIT_TERMINAL_PROMPT'], '0')
        self.assertNotIn('GH_TOKEN', env)
        self.assertNotIn('GIT_DIR', env)

    def test_missing_promisor_commit_never_invokes_local_transport(self):
        # Entirely inert: the only configured transport is an owned local
        # upload-pack witness which records invocation and immediately refuses.
        witness = self.base / 'upload-pack-witness'
        witness.write_text('#!/bin/sh\nprintf invoked > "$0.called"\nexit 1\n')
        witness.chmod(0o755)
        FIXTURE.git(self.repo, 'config', 'remote.origin.url', str(self.repo))
        FIXTURE.git(self.repo, 'config', 'remote.origin.promisor', 'true')
        FIXTURE.git(self.repo, 'config', 'remote.origin.partialclonefilter', 'blob:none')
        FIXTURE.git(self.repo, 'config', 'remote.origin.uploadpack', str(witness))
        before = {str(path.relative_to(self.repo)): path.read_bytes()
                  for path in self.repo.rglob('*') if path.is_file()}
        with self.assertRaises(FIXTURE.subprocess.SubprocessError):
            FIXTURE.prepare(self.repo, '1' * 40, self.base)
        self.assertFalse(witness.with_suffix('.called').exists())
        self.assertEqual(set(self.base.iterdir()), {self.repo, witness})
        self.assertEqual(before, {str(path.relative_to(self.repo)): path.read_bytes()
                                  for path in self.repo.rglob('*') if path.is_file()})


if __name__ == '__main__':
    unittest.main()
