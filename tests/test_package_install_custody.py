# SPDX-License-Identifier: MIT
"""Real byte-stream/copy/hash/consumer process, with root admission modelled.

No sudo or pacman is run. Root UID/cache and the terminal are substituted in a
private fixture; actual root protection remains an installed acceptance gate.
"""
import hashlib
import os
import re
from pathlib import Path
import shlex
import subprocess
import tempfile
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
        self.assertIn('[[ $# == 4 && $EUID -eq 0 ]]', source)
        self.assertIn("== 0:700", source)
        # Substitute admission only. Copy limits, hashes, modes, protected-path
        # handoff and cleanup execute the production script in real subprocesses.
        source = source.replace('$EUID -eq 0', '$EUID -eq ' + str(os.getuid()))
        source = source.replace('for parent in / /var /var/cache;',
                                'for parent in ' + shlex.quote(str(self.cache)) + ';')
        source = source.replace('stat -c \'%u\' -- "$parent") == 0 ]]',
                                'stat -c \'%u\' -- "$parent") == ' + str(os.getuid()) + ' ]]')
        source = source.replace('== 0:700', '== ' + str(os.getuid()) + ':700')
        source = source.replace('-p /var/cache ', '-p ' + shlex.quote(str(self.cache)) + ' ')
        return source.replace('/usr/bin/pacman', shlex.quote(str(self.consumer))).replace('</dev/tty', '</dev/null')

    def test_frontend_installs_verified_private_bytes_even_if_cache_changes_at_consumer(self):
        model = self.root_program_model()
        (self.base / "install-package-pair.sh").write_text(model)
        source = (ROOT / "plugin/setup-runtime.sh").read_text().replace('/usr/bin/sudo', 'fixture_sudo')
        # This positive stream test substitutes root admission above. Rebind
        # only its fixture's trusted program pin to those modelled bytes.
        source = re.sub(r'(?m)^readonly install_pair_program_sha256=[a-f0-9]{64}$',
                        'readonly install_pair_program_sha256=' + hashlib.sha256(model.rstrip('\n').encode()).hexdigest(), source)
        # Old implementation reaches this consumer with the user-writable path.
        source = source.replace('/usr/bin/pacman', shlex.quote(str(self.consumer)))
        source_path = self.base / "setup-runtime.sh"
        source_path.write_text(source)
        result = subprocess.run(['bash', '-c',
            'set -euo pipefail; source "$1"; setup_dir="$2"; fixture_sudo() { "$@"; }; '
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
                    str(size), self.expected[0], str(len(dns)), self.expected[1]],
                    input=stream, capture_output=True, timeout=10)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((self.base / "consumed").exists())
                self.assertEqual(list(self.cache.iterdir()), [])


if __name__ == '__main__':
    unittest.main()
