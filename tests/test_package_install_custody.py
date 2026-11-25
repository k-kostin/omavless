# SPDX-License-Identifier: MIT
"""Real byte-stream/copy/hash/consumer process, with root admission modelled.

No sudo or pacman is run. Root UID/cache and the terminal are substituted in a
private fixture; actual root protection remains an installed acceptance gate.
"""
import hashlib
import os
import pty
import re
from pathlib import Path
import shlex
import subprocess
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]


class PackageInstallCustodyTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="package-custody-", dir=Path.home() / ".cache/ovt")
        self.addCleanup(temporary.cleanup)
        self.base = Path(temporary.name)
        self.cache = self.base / "root-cache"
        self.cache.mkdir(mode=0o700)
        self.app = self.base / "app.pkg.tar.zst"
        self.dns = self.base / "dns.pkg.tar.zst"
        self.app.write_bytes(b"original application package")
        self.dns.write_bytes(b"original DNS package")
        self.expected = [hashlib.sha256(p.read_bytes()).hexdigest() for p in (self.app, self.dns)]
        self.sizes = [p.stat().st_size for p in (self.app, self.dns)]
        self.consumer = self.base / "pacman-fixture"
        self.consumer.write_text("#!/bin/bash\nset -euo pipefail\n"
            + "printf poison > " + shlex.quote(str(self.app)) + "\n"
            + "[[ $1 == -U && $2 == -- ]]\n"
            + "[[ $3 != " + shlex.quote(str(self.app)) + " ]]\n"
            + "[[ $(stat -c '%a' -- \"$3\") == 400 ]]\n"
            + "printf '%s  %s\\n%s  %s\\n' " + self.expected[0] + ' "$3" '
            + self.expected[1] + ' "$4" | sha256sum --check --status -\n'
            + "touch " + shlex.quote(str(self.base / "consumed")) + "\n")
        self.consumer.chmod(0o700)

    def root_program_model(self):
        source = (ROOT / "plugin/install-package-pair.sh").read_text()
        self.assertIn('[[ $# -ge 1 && $EUID -eq 0 ]]', source)
        self.assertIn("== 0:700", source)
        source = source.replace('caller_uid=${OMAVLESS_INSTALL_UID:-}', 'caller_uid=${OMAVLESS_INSTALL_UID:-' + str(os.getuid()) + '}')
        # Substitute admission only. Copy limits, hashes, modes, protected-path
        # handoff and cleanup execute the production script in real subprocesses.
        source = source.replace('$EUID -eq 0', '$EUID -eq ' + str(os.getuid()))
        source = source.replace('for parent in / /var /var/cache;',
                                'for parent in ' + shlex.quote(str(self.cache)) + ';')
        source = source.replace('stat -c \'%u\' -- "$parent") == 0 ]]',
                                'stat -c \'%u\' -- "$parent") == ' + str(os.getuid()) + ' ]]')
        source = source.replace('== 0:700', '== ' + str(os.getuid()) + ':700')
        source = source.replace('== 0:400:1', '== ' + str(os.getuid()) + ':400:1')
        source = source.replace('stage=/var/cache/$token', 'stage=' + shlex.quote(str(self.cache)) + '/$token')
        source = source.replace('-p /var/cache ', '-p ' + shlex.quote(str(self.cache)) + ' ')
        return source.replace('/usr/bin/pacman', shlex.quote(str(self.consumer))).replace('</dev/tty', '</dev/null')

    def prepare(self):
        app, dns = self.app.read_bytes(), self.dns.read_bytes()
        result = subprocess.run(['bash', '-c', self.root_program_model(), 'fixture',
            'prepare', str(len(app)), self.expected[0], str(len(dns)), self.expected[1]],
            input=app + dns, capture_output=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        token = result.stdout.decode().strip()
        self.assertRegex(token, r'^omavless-packages\.[A-Za-z0-9]{8}$')
        return token

    def finish(self, token, action='install'):
        args = [action, token]
        if action in ('install', 'discard'):
            args += [str(self.sizes[0]), self.expected[0],
                     str(self.sizes[1]), self.expected[1]]
        master, slave = pty.openpty()
        try:
            return subprocess.run(['bash', '-c', self.root_program_model(), 'fixture', *args],
                stdin=slave, capture_output=True, timeout=10)
        finally:
            os.close(slave)
            os.close(master)

    def test_preparation_never_submits_and_terminal_phase_uses_protected_pair(self):
        token = self.prepare()
        stage = self.cache / token
        self.assertFalse((self.base / 'consumed').exists())
        self.assertEqual(len(list(stage.iterdir())), 3)
        result = self.finish(token)
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        self.assertTrue((self.base / 'consumed').exists())
        self.assertFalse(stage.exists())
        self.assertNotEqual(self.finish(token).returncode, 0)

    def test_known_prepared_pair_can_be_discarded_but_claim_cannot_be_replayed(self):
        token = self.prepare()
        stage = self.cache / token
        (stage / 'ready').rename(stage / 'claimed')
        for action in ('install', 'discard'):
            self.assertNotEqual(self.finish(token, action).returncode, 0)
            self.assertTrue(stage.exists())
        self.assertFalse((self.base / 'consumed').exists())
        fresh = self.prepare()
        self.assertEqual(self.finish(fresh, 'discard').returncode, 0)
        self.assertFalse((self.cache / fresh).exists())

    def test_unfinished_foreign_or_mismatched_preparation_cannot_install_or_discard(self):
        token = self.prepare()
        stage = self.cache / token
        ready = (stage / 'ready').read_bytes()
        for action in ('install', 'discard'):
            args = [action, token, str(self.sizes[0]), self.expected[0],
                    str(self.sizes[1]), self.expected[1]]
            foreign = subprocess.run(['bash', '-c', self.root_program_model(), 'fixture', *args],
                env={**os.environ, 'OMAVLESS_INSTALL_UID': str(os.getuid() + 1)},
                capture_output=True, timeout=10)
            self.assertNotEqual(foreign.returncode, 0)
            self.assertEqual((stage / 'ready').read_bytes(), ready)
            self.assertFalse((stage / 'claimed').exists())
        (stage / 'ready').rename(stage / '.ready-pending')
        for action in ('install', 'discard'):
            self.assertNotEqual(self.finish(token, action).returncode, 0)
            self.assertFalse((stage / 'claimed').exists())
        (stage / '.ready-pending').rename(stage / 'ready')
        self.assertEqual(self.finish(token, 'discard').returncode, 0)

    def test_install_discard_race_has_one_winner(self):
        token = self.prepare()
        args = [token, str(self.sizes[0]), self.expected[0],
                str(self.sizes[1]), self.expected[1]]
        master, slave = pty.openpty()
        try:
            children = [subprocess.Popen(['bash', '-c', self.root_program_model(), 'fixture',
                action, *args], stdin=slave, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                for action in ('install', 'discard')]
            for child in children:
                child.communicate(timeout=10)
            self.assertEqual(sorted(child.returncode for child in children), [0, 1])
            self.assertFalse((self.cache / token).exists())
        finally:
            os.close(slave)
            os.close(master)

    def test_delayed_loser_cannot_reclaim_after_winner_cleanup(self):
        token = self.prepare()
        waiting, release = self.base / 'waiting', self.base / 'release'
        model = self.root_program_model()
        anchor = '  # One-way claim:'
        self.assertIn(anchor, model)
        hook = '  touch ' + shlex.quote(str(waiting)) + '; while [[ ! -e ' + shlex.quote(str(release)) + ' ]]; do sleep .01; done\n'
        model = model.replace(anchor, hook + anchor)
        args = ['discard', token, str(self.sizes[0]), self.expected[0],
                str(self.sizes[1]), self.expected[1]]
        child = subprocess.Popen(['bash', '-c', model, 'fixture', *args],
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            deadline = time.monotonic() + 5
            while not waiting.exists() and time.monotonic() < deadline:
                time.sleep(.01)
            self.assertTrue(waiting.exists())
            self.assertEqual(self.finish(token).returncode, 0)
            release.touch()
            child.communicate(timeout=5)
            self.assertNotEqual(child.returncode, 0)
            self.assertFalse((self.cache / token).exists())
        finally:
            release.touch()
            if child.poll() is None:
                child.terminate()
                child.communicate(timeout=5)

    def test_cleanup_failure_cannot_reopen_claim_and_preserves_consumer_result(self):
        token = self.prepare()
        stage = self.cache / token
        model = self.root_program_model()
        removal = '/usr/bin/rm -f -- "$app" "$dns" "$ready" "$claimed" || cleanup_status=1'
        self.assertIn(removal, model)
        model = model.replace(removal, 'cleanup_status=1 # injected known cleanup failure')
        master, slave = pty.openpty()
        try:
            result = subprocess.run(['bash', '-c', model, 'fixture', 'install', token,
                str(self.sizes[0]), self.expected[0], str(self.sizes[1]), self.expected[1]],
                stdin=slave, capture_output=True, timeout=10)
        finally:
            os.close(slave)
            os.close(master)
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        self.assertIn(b'cleanup is incomplete', result.stderr)
        self.assertFalse((stage / 'ready').exists())
        self.assertTrue((stage / 'claimed').exists())
        for action in ('install', 'discard'):
            self.assertNotEqual(self.finish(token, action).returncode, 0)

    def test_error_reading_stream_end_is_not_successful_eof(self):
        model = self.root_program_model().replace('/usr/bin/dd bs=1 count=1 status=none',
            "/usr/bin/bash -c 'exit 7'")
        result = subprocess.run(['bash', '-c', model, 'fixture', 'prepare',
            str(self.sizes[0]), self.expected[0], str(self.sizes[1]), self.expected[1]],
            input=self.app.read_bytes() + self.dns.read_bytes(), capture_output=True, timeout=10)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.base / 'consumed').exists())
        self.assertEqual(list(self.cache.iterdir()), [])

    def test_prepared_source_symlink_mutation_and_arbitrary_identifier_are_refused(self):
        for mutation in ('symlink', 'bytes', 'mode', 'hardlink'):
            with self.subTest(mutation=mutation):
                token = self.prepare()
                app = self.cache / token / 'omavless.pkg.tar.zst'
                if mutation == 'symlink':
                    app.unlink()
                    app.symlink_to(self.app)
                elif mutation == 'bytes':
                    app.chmod(0o600)
                    app.write_bytes(b'x' * self.app.stat().st_size)
                    app.chmod(0o400)
                elif mutation == 'mode':
                    app.chmod(0o600)
                else:
                    os.link(app, self.base / 'extra-link')
                self.assertNotEqual(self.finish(token).returncode, 0)
                self.assertFalse((self.base / 'consumed').exists())
        for token in ('../outside', '/var/cache/other', 'omavless-packages.12345678/../other'):
            self.assertNotEqual(self.finish(token).returncode, 0)

    def test_frontend_installs_verified_private_bytes_even_if_cache_changes_at_consumer(self):
        model = self.root_program_model()
        (self.base / "install-package-pair.sh").write_text(model)
        source = (ROOT / "plugin/setup-runtime.sh").read_text().replace('/usr/bin/sudo', 'fixture_sudo')
        master, slave = pty.openpty()
        self.addCleanup(os.close, master)
        self.addCleanup(os.close, slave)
        # Only the terminal endpoint is modelled here; real sudo/PTY relay is
        # an installed gate. The final root subprocess still gets tty stdin.
        source = source.replace('/dev/tty', os.ttyname(slave))
        # This positive stream test substitutes root admission above. Rebind
        # only its fixture's trusted program pin to those modelled bytes.
        source = re.sub(r'(?m)^readonly install_pair_program_sha256=[a-f0-9]{64}$',
                        'readonly install_pair_program_sha256=' + hashlib.sha256(model.rstrip('\n').encode()).hexdigest(), source)
        # Old implementation reaches this consumer with the user-writable path.
        source = source.replace('/usr/bin/pacman', shlex.quote(str(self.consumer)))
        source_path = self.base / "setup-runtime.sh"
        source_path.write_text(source)
        result = subprocess.run(['bash', '-c',
            'set -euo pipefail; source "$1"; setup_dir="$2"; fixture_sudo() { SUDO_UID=$(id -u) "$@"; }; fresh_package_boundary() { return 0; }; '
            'package_install "$3" "$4" "$5" "$6"',
            'fixture', str(source_path), str(self.base), str(self.app), str(self.dns), *self.expected],
            capture_output=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        self.assertTrue((self.base / "consumed").exists())
        self.assertEqual(self.app.read_bytes(), b"poison")
        self.assertEqual(list(self.cache.iterdir()), [])

    def test_changed_neighbor_program_is_refused_before_privileged_submission(self):
        (self.base / 'install-package-pair.sh').write_text('echo untrusted-root-program\n')
        source = (ROOT / 'plugin/setup-runtime.sh').read_text().replace('/usr/bin/sudo', 'fixture_sudo')
        master, slave = pty.openpty()
        self.addCleanup(os.close, master)
        self.addCleanup(os.close, slave)
        source = source.replace('/dev/tty', os.ttyname(slave))
        launcher = self.base / 'setup-runtime.sh'
        launcher.write_text(source)
        marker = self.base / 'sudo-submitted'
        result = subprocess.run(['bash', '-c',
            'set -euo pipefail; source "$1"; setup_dir="$2"; fixture_marker="$7"; fixture_sudo() { touch -- "$fixture_marker"; }; '
            'package_install "$3" "$4" "$5" "$6"',
            'fixture', str(launcher), str(self.base), str(self.app), str(self.dns), *self.expected, str(marker)],
            capture_output=True, timeout=10)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(marker.exists())

    def test_shipped_program_pin_matches_exact_captured_command(self):
        source = (ROOT / 'plugin/setup-runtime.sh').read_text()
        pin = re.search(r'(?m)^readonly install_pair_program_sha256=([a-f0-9]{64})$', source)
        self.assertIsNotNone(pin)
        self.assertEqual(pin.group(1), hashlib.sha256((ROOT / 'plugin/install-package-pair.sh').read_bytes().rstrip(b'\n')).hexdigest())

    def test_changed_neighbor_with_poisoned_path_cannot_forge_program_digest(self):
        (self.base / 'install-package-pair.sh').write_text('echo untrusted-root-program\n')
        source = (ROOT / 'plugin/setup-runtime.sh').read_text().replace('/usr/bin/sudo', 'fixture_sudo')
        master, slave = pty.openpty()
        self.addCleanup(os.close, master)
        self.addCleanup(os.close, slave)
        source = source.replace('/dev/tty', os.ttyname(slave))
        launcher = self.base / 'setup-runtime.sh'
        launcher.write_text(source)
        pin = re.search(r'(?m)^readonly install_pair_program_sha256=([a-f0-9]{64})$', source).group(1)
        poison = self.base / 'path'
        poison.mkdir(mode=0o700)
        digest = poison / 'sha256sum'
        digest.write_text('#!/bin/bash\n/usr/bin/cat >/dev/null\nprintf "%s  -\\n" ' + shlex.quote(pin) + '\n')
        digest.chmod(0o700)
        marker = self.base / 'sudo-submitted'
        result = subprocess.run(['/usr/bin/bash', '-c',
            'set -euo pipefail; source "$1"; setup_dir="$2"; fixture_marker="$7"; '
            'fixture_sudo() { /usr/bin/touch -- "$fixture_marker"; }; '
            'package_install "$3" "$4" "$5" "$6"',
            'fixture', str(launcher), str(self.base), str(self.app), str(self.dns), *self.expected, str(marker)],
            env={**os.environ, 'PATH': str(poison) + ':/usr/bin:/bin'},
            capture_output=True, timeout=10)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(marker.exists())

    def test_changed_truncated_extra_and_out_of_bound_streams_never_reach_pacman(self):
        app, dns = self.app.read_bytes(), self.dns.read_bytes()
        for stream, size in [(b"x" * len(app) + dns, len(app)), (app + dns[:-1], len(app)),
                             (app + dns + b"extra", len(app)), (b"", 268435457)]:
            with self.subTest(size=size, bytes=len(stream)):
                result = subprocess.run(['bash', '-c', self.root_program_model(), 'fixture',
                    'prepare', str(size), self.expected[0], str(len(dns)), self.expected[1]],
                    input=stream, capture_output=True, timeout=10)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((self.base / "consumed").exists())
                self.assertEqual(list(self.cache.iterdir()), [])


if __name__ == '__main__':
    unittest.main()
