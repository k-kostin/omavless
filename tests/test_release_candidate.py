# SPDX-License-Identifier: MIT
"""Offline release-assembly and sandboxed installer tests, never a live install."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('release_candidate', ROOT / 'packaging/release/build-candidate.py')
RELEASE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RELEASE)


class ReleaseCandidateTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='omavless-release-')
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.repo = self.base / 'source'
        self.repo.mkdir()
        members = (*RELEASE.FILES, 'Cargo.toml', 'packaging/arch/', 'packaging/systemd/')
        for name in members:
            source, destination = ROOT / name, self.repo / name
            if destination.exists():
                continue
            destination.parent.mkdir(parents=True, exist_ok=True)
            if source.is_dir():
                shutil.copytree(source, destination)
            else:
                shutil.copyfile(source, destination)
        (self.repo / '.gitignore').write_text('private-local.txt\n')
        # Historical RC assembly remains independently tested after version bumps.
        (self.repo / 'Cargo.toml').write_text('[workspace.package]\nversion = "0.8.0-rc.1"\n')
        (self.repo / 'private-local.txt').write_text('synthetic-private-canary')
        self.git('init', '-q')
        self.commit()
        self.output = self.base / 'output'
        self.output.mkdir()

    def git(self, *args):
        return subprocess.check_output(['git', '-C', str(self.repo), *args], stderr=subprocess.DEVNULL)

    def commit(self):
        self.git('add', '.')
        self.git('-c', 'user.name=Synthetic', '-c', 'user.email=synthetic@example.invalid',
                 'commit', '-qm', 'fixture')
        self.sha = self.git('rev-parse', 'HEAD').decode().strip()

    def archive(self, name='frontend.tar.xz', stable=False):
        target = self.output / name
        RELEASE.frontend(self.repo, target, self.sha, RELEASE.version(self.repo, stable), 1)
        return target

    def test_native_source_manifest_version_and_lock_are_coherent(self):
        self.assertEqual(RELEASE.version(ROOT, stable=True), '0.8.2')
        lock = tomllib.loads((ROOT / 'Cargo.lock').read_text())
        versions = {p['version'] for p in lock['package'] if p['name'].startswith('omavless-')}
        self.assertEqual(versions, {RELEASE.version(ROOT, stable=True)})
        self.assertEqual(json.loads((ROOT / 'manifest.json').read_text())['version'], RELEASE.version(ROOT, stable=True))
        with self.assertRaises(ValueError):
            RELEASE.version(ROOT)  # Explicit --stable is still required.
        for invalid in ('0.8.0', '0.8.0-rc.0', '0.8.0-rc.1;false', '0.8.0-beta.1'):
            (self.repo / 'Cargo.toml').write_text(f'[workspace.package]\nversion = "{invalid}"\n')
            with self.assertRaises(ValueError):
                RELEASE.version(self.repo)

    def test_archive_is_deterministic_native_only_and_pins_frontend_version(self):
        first, second = self.archive(), self.archive('second.tar.xz')
        self.assertEqual(RELEASE.digest(first), RELEASE.digest(second))
        with tarfile.open(first) as archive:
            names = archive.getnames()
            self.assertTrue(all(m.isfile() and m.uid == m.gid == 0 and m.mtime == 1 for m in archive))
            self.assertFalse(any(name.endswith('.py') or '/skills/' in name or '/tests/' in name
                                 or 'private-local' in name or name.endswith('/uninstall.sh') for name in names))
            manifest = json.load(archive.extractfile('omavless-frontend/manifest.json'))
            self.assertEqual(manifest['version'], RELEASE.version(self.repo))
            for name in ('install.sh', 'install-frontend.sh', 'backend.sh'):
                self.assertEqual(archive.getmember('omavless-frontend/' + name).mode, 0o755)
            self.assertEqual(archive.extractfile('omavless-frontend/plugin/Panel.qml').read(),
                             (self.repo / 'plugin/Panel.qml').read_bytes())
            self.assertEqual(archive.extractfile('omavless-frontend/install-frontend.sh').read(),
                             (ROOT / 'install.sh').read_bytes())

    def test_stable_mode_requires_exact_bounded_stable_version(self):
        for valid in ('0.8.0', '0.8.1', '1.2.34'):
            (self.repo / 'Cargo.toml').write_text(f'[workspace.package]\nversion = "{valid}"\n')
            self.assertEqual(RELEASE.version(self.repo, stable=True), valid)
            with self.assertRaises(ValueError):
                RELEASE.version(self.repo)
        for invalid in ('0.8.0-rc.1', '0.8.0-beta.1', '0.8.0+private', '0.8.0;false',
                        'v0.8.0', '0.8', '1' * 33 + '.0.0'):
            (self.repo / 'Cargo.toml').write_text(f'[workspace.package]\nversion = "{invalid}"\n')
            with self.assertRaises(ValueError):
                RELEASE.version(self.repo, stable=True)

    def test_stable_mode_refuses_rc_before_packaging(self):
        with patch.object(RELEASE.subprocess, 'run', wraps=subprocess.run) as effects:
            with self.assertRaises(ValueError):
                RELEASE.assemble(self.repo, self.output, Path('/usr/bin/true'), self.sha, stable=True)
            self.assertTrue(effects.called)
            self.assertTrue(all(call.args[0][0] == 'git' for call in effects.call_args_list))
        self.assertEqual(list(self.output.iterdir()), [])

    def test_symlink_inside_allowed_tree_is_rejected(self):
        (self.repo / 'plugin/unsafe.qml').symlink_to('/etc/passwd')
        self.commit()
        with self.assertRaises(ValueError):
            self.archive()

    def test_exact_source_dirty_and_output_refusals_before_packaging(self):
        RELEASE.checked_source(self.repo, self.sha)
        for wrong in ('0' * 40, ';false', ''):
            with self.assertRaises(ValueError):
                RELEASE.assemble(self.repo, self.output, Path('/usr/bin/true'), wrong)
        (self.repo / 'uncommitted').write_text('synthetic')
        with self.assertRaises(ValueError):
            RELEASE.assemble(self.repo, self.output, Path('/usr/bin/true'), self.sha)
        (self.repo / 'uncommitted').unlink()
        link = self.base / 'output-link'
        link.symlink_to(self.output)
        for unsafe in (self.repo, link, Path('relative')):
            with self.assertRaises(ValueError):
                RELEASE.assemble(self.repo, unsafe, Path('/usr/bin/true'), self.sha)
        sentinel = self.output / 'sentinel'
        sentinel.write_text('preserved')
        with self.assertRaises(ValueError):
            RELEASE.assemble(self.repo, self.output, Path('/usr/bin/true'), self.sha)
        self.assertEqual(list(self.output.iterdir()), [sentinel])

    def installer_env(self, stable=False):
        with tarfile.open(self.archive(stable=stable)) as archive:
            archive.extractall(self.output, filter='data')
        self.frontend = self.output / 'omavless-frontend'
        self.bin = self.base / 'bin'
        self.bin.mkdir()
        self.home = self.base / 'home with spaces'
        self.home.mkdir()
        self.trace = self.base / 'trace'
        env = {'HOME': str(self.home), 'PATH': str(self.bin), 'TEST_TRACE': str(self.trace),
               'TEST_OWNER': 'rust', 'LC_ALL': 'C.UTF-8'}
        for tool in ('dirname', 'mkdir', 'mktemp', 'cp', 'chmod', 'mv', 'rm', 'sleep', 'jq'):
            (self.bin / tool).symlink_to('/usr/bin/' + tool)
        self.stub('omavless', '''
[ "$#" = 2 ] && [ "$1" = plugin ] && [ "$2" = target ] || exit 99
printf 'owner-read\n' >> "$TEST_TRACE"
printf '%s\n' "$TEST_OWNER"
''')
        self.stub('omarchy', '''
printf '%s:%s\n' "$1" "$2" >> "$TEST_TRACE"
case "$1:$2" in
 plugin:validate) exit 0 ;;
 plugin:list) printf '[{"id":"kdk.omavless"}]\n' ;;
 plugin:enable) exit 0 ;;
 *) exit 99 ;;
esac
''')
        return env

    def stub(self, name, code):
        path = self.bin / name
        path.write_text('#!/bin/sh\n' + code)
        path.chmod(0o700)

    def invoke_installer(self, env, *args):
        return subprocess.run(['/bin/bash', str(self.frontend / 'install.sh'), *args],
                              env=env, capture_output=True, text=True, timeout=15)

    def test_fresh_and_existing_frontend_install_have_no_python_or_activation(self):
        self.check_frontend_install(stable=False)

    def test_stable_fresh_and_existing_frontend_install_without_python(self):
        (self.repo / 'Cargo.toml').write_text('[workspace.package]\nversion = "0.8.0"\n')
        self.commit()
        self.check_frontend_install(stable=True)

    def check_frontend_install(self, stable):
        env = self.installer_env(stable)
        target = self.home / '.config/omarchy/plugins/kdk.omavless'
        for fresh in (True, False):
            if not fresh:
                (target / 'backend.py').write_text('synthetic legacy remnant')
                (target / 'uninstall.sh').write_text('synthetic legacy remnant')
                self.trace.unlink()
            result = self.invoke_installer(env)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(list(target.rglob('*.py')), [])
            self.assertFalse((target / 'uninstall.sh').exists())
            self.assertEqual(json.loads((target / 'manifest.json').read_text())['version'], RELEASE.version(self.repo, stable))
            calls = self.trace.read_text().splitlines()
            self.assertEqual(calls.count('owner-read'), 2)
            self.assertEqual('plugin:enable' in calls, fresh)
            self.assertNotIn('cutover', '\n'.join(calls))
            self.assertIn('file picker missing', result.stdout)

    def test_frontend_missing_legacy_unknown_owner_and_extra_args_do_not_install(self):
        env = self.installer_env()
        target = self.home / '.config/omarchy/plugins/kdk.omavless'
        for owner in ('legacy', 'unknown', 'rust\nlegacy'):
            env['TEST_OWNER'] = owner
            self.assertNotEqual(self.invoke_installer(env).returncode, 0)
            self.assertFalse(target.exists())
        (self.bin / 'omavless').unlink()
        self.assertNotEqual(self.invoke_installer(env).returncode, 0)
        self.assertFalse(target.exists())
        for args in [('--legacy',), ('--native-only',), (';false',)]:
            self.assertEqual(self.invoke_installer(env, *args).returncode, 2)
            self.assertFalse(target.exists())

    def test_real_offline_pair_integrity_and_provenance(self):
        self.check_offline_pair(stable=False)

    def test_real_stable_pair_integrity_and_inspector(self):
        (self.repo / 'Cargo.toml').write_text('[workspace.package]\nversion = "0.8.0"\n')
        self.commit()
        self.check_offline_pair(stable=True)

    def check_offline_pair(self, stable):
        if os.geteuid() == 0 or not all(shutil.which(t) for t in ('makepkg', 'fakeroot', 'bsdtar', 'readelf', 'zstd')):
            self.skipTest('non-root Arch packaging tools required')
        RELEASE.assemble(self.repo, self.output, Path('/usr/bin/true'), self.sha, stable=stable)
        result = subprocess.run(['sha256sum', '--check', 'SHA256SUMS'], cwd=self.output,
                                capture_output=True, timeout=10)
        self.assertEqual(result.returncode, 0)
        identity = json.loads((self.output / 'release-candidate.json').read_text())
        self.assertEqual(identity['sourceCommit'], self.sha)
        self.assertEqual(identity['provenance'], 'caller-supplied-prebuilt')
        self.assertEqual(identity['publication'], 'unpublished-candidate')
        self.assertEqual(identity['version'], '0.8.0' if stable else '0.8.0-rc.1')
        self.assertEqual(identity['binarySha256'], hashlib.sha256(Path('/usr/bin/true').read_bytes()).hexdigest())
        self.assertEqual(len(identity['artifacts']), 2)
        for name, sha in identity['artifacts'].items():
            self.assertEqual(RELEASE.digest(self.output / name), sha)
        spec = importlib.util.spec_from_file_location('candidate_package_gate', ROOT / 'tests/installed_native_package.py')
        gate = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(gate)
        archive, = self.output.glob('*.pkg.tar.zst')
        # Temporary test roots live below /tmp; production archive inspection
        # retains its stricter trusted-parent rule. No install method is called.
        with patch.object(gate, 'safe_parents'):
            checked = gate.inspect_archive(archive)
        self.assertEqual(checked['source'], self.sha)
        self.assertEqual(checked['binary'], identity['binarySha256'])
        self.assertEqual(checked['version'], '0.8.0-1' if stable else '0.8.0rc1-1')
        frontend, = self.output.glob('*-frontend.tar.xz')
        with tarfile.open(frontend) as payload:
            self.assertEqual(json.load(payload.extractfile('omavless-frontend/manifest.json'))['version'],
                             identity['version'])
            self.assertFalse(any(n.endswith('.py') or '/skills/' in n or '/tests/' in n
                                 for n in payload.getnames()))
        self.assertEqual(self.git('status', '--porcelain'), b'')


if __name__ == '__main__':
    unittest.main()
