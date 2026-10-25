# SPDX-License-Identifier: MIT
"""Actual advisory inspector, inert payloads; no installed binaries are run."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import shutil
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
INSPECTOR = ROOT / "plugin/setup-predecessor.sh"
OLD = "e3d2245966c725ee11b505899b22a20721ff2ddd"
TARGET = "b65695ee0609c2a971289109975678b8da87c946"


class PairInventoryTests(unittest.TestCase):
    def setUp(self):
        parent = Path.home() / ".cache/ovt"
        parent.mkdir(parents=True, exist_ok=True)
        temporary = tempfile.TemporaryDirectory(prefix="pair-inv-", dir=parent)
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        self.directory.chmod(0o700)
        self.app = self.directory / "app"
        self.core = self.directory / "core"
        self.broker = self.directory / "broker"
        self.identity = self.directory / "identity"
        self.receipt = self.directory / "receipt"
        for path in (self.app, self.core, self.broker):
            path.write_bytes(b"inert fixture executable; never execute")
            path.chmod(0o755)
        self.delivery(OLD)

    def delivery(self, source, dns_source=None):
        digest = hashlib.sha256(self.app.read_bytes()).hexdigest()
        self.identity.write_text(
            f"schemaVersion=3\nsourceCommit={source}\nbinarySha256={digest}\n"
            "architecture=x86_64\nprovenance=caller-supplied-prebuilt\nproductVersion=0.9.8\n")
        self.identity.chmod(0o644)
        self.receipt.write_text(json.dumps({
            "schema": 1, "architecture": "x86_64", "package_flavor": "release",
            "broker_feature": "release-package", "omavless_commit": dns_source or source,
            "sha256": {"mihomo": hashlib.sha256(self.core.read_bytes()).hexdigest(),
                       "omavless-dns-broker": hashlib.sha256(self.broker.read_bytes()).hexdigest()}}))
        self.receipt.chmod(0o644)
        self.fixture_pins = '\t'.join(hashlib.sha256(path.read_bytes()).hexdigest()
                                      for path in (self.app, self.core, self.broker, self.receipt))

    def inspect(self, published=False, candidate=False):
        # Only this inert test replaces the compiled/public expected identity;
        # the production caller has no pin/path override.
        # Capture fixture pins before entering the two-argument helper.
        override = '' if published else 'fixture_pins=$9; _omavless_pair_pins() { printf "%s\\n" "$fixture_pins"; }; '
        command = 'set -euo pipefail; source "$1"; ' + override + '_omavless_pair_delivery_inspect "$2" "$3" "$4" "$5" "$6" x86_64 "$7" "$8"' + (' 0.9.9' if candidate else '')
        result = subprocess.run(
            ["/bin/bash", "-c", command, "fixture", str(INSPECTOR),
             str(self.identity), str(self.app), str(self.receipt), str(self.core), str(self.broker),
             TARGET, str(os.getuid()), self.fixture_pins], capture_output=True, text=True, timeout=15,
            env={"PATH": "/usr/bin:/bin"})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, "")
        self.assertIn(result.stdout, ["current\n", "stale\n", "missing\n", "mismatch\n", "unknown\n"])
        return result.stdout.strip()

    def test_nonpublic_current_requires_exact_version_target_and_all_payloads(self):
        self.delivery(TARGET)
        self.assertEqual(self.inspect(), 'current')
        self.assertEqual(self.inspect(candidate=True), 'unknown')
        self.identity.write_text(self.identity.read_text().replace('productVersion=0.9.8', 'productVersion=0.9.9'))
        self.assertEqual(self.inspect(candidate=True), 'current')
        # Public path does not acquire the candidate's version admission.
        self.assertEqual(self.inspect(), 'unknown')
        # Self-consistent old source cannot be relabelled as a candidate.
        self.delivery(OLD)
        self.identity.write_text(self.identity.read_text().replace('productVersion=0.9.8', 'productVersion=0.9.9'))
        self.assertEqual(self.inspect(candidate=True), 'unknown')
        self.delivery(TARGET)
        self.identity.write_text(self.identity.read_text().replace('productVersion=0.9.8', 'productVersion=0.9.9'))
        self.app.write_bytes(b'modified candidate app')
        self.assertEqual(self.inspect(candidate=True), 'unknown')

    def test_nonpublic_current_still_distinguishes_missing_and_mixed_dns(self):
        self.delivery(TARGET, dns_source=OLD)
        self.identity.write_text(self.identity.read_text().replace('productVersion=0.9.8', 'productVersion=0.9.9'))
        self.assertEqual(self.inspect(candidate=True), 'mismatch')
        for path in (self.receipt, self.core, self.broker):
            path.unlink()
        self.assertEqual(self.inspect(candidate=True), 'missing')

    def test_known_public_source_does_not_admit_a_self_consistent_local_rebuild(self):
        self.assertEqual(self.inspect(published=True), 'unknown')
        self.delivery('99c556e5c966e1e1dae04f2d8babd2ca2dec9375')
        self.assertEqual(self.inspect(published=True), 'unknown')

    def test_real_public_pin_lookup_is_closed_and_shared_with_native_updater(self):
        data = json.loads((ROOT / 'plugin/published-pairs.json').read_text())
        self.assertEqual(data['schemaVersion'], 1)
        self.assertEqual(len(data['pairs']), 12)
        self.assertEqual(len({(pair['sourceCommit'], pair['architecture'])
                              for pair in data['pairs']}), 12)
        for pair in data['pairs']:
            result = subprocess.run(['/bin/bash', '-c', 'source "$1"; _omavless_pair_pins "$2" "$3"',
                                     'fixture', str(INSPECTOR), pair['sourceCommit'], pair['architecture']],
                                    capture_output=True, text=True, timeout=5)
            expected = '\t'.join(pair[key] for key in ['appSha','coreSha','brokerSha','dnsReceiptSha']) + '\n'
            self.assertEqual(result.stdout, expected)
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_fix7_remains_known_when_the_frontend_selects_a_new_target(self):
        source = "d4d4310be6ade7e7e166f8b672cf90c7ad3e5b3b"
        inspector = self.directory / 'setup-predecessor.sh'
        shutil.copyfile(INSPECTOR, inspector)
        shutil.copyfile(ROOT / 'plugin/published-pairs.json', self.directory / 'published-pairs.json')
        metadata = json.loads((ROOT / 'plugin/component-updates.json').read_text())
        metadata['sourceCommit'] = 'c' * 40
        (self.directory / 'component-updates.json').write_text(json.dumps(metadata))
        registry = json.loads((ROOT / 'plugin/published-pairs.json').read_text())['pairs']
        for arch in ['x86_64', 'aarch64']:
            matches = [record for record in registry
                       if record['sourceCommit'] == source and record['architecture'] == arch]
            self.assertEqual(len(matches), 1)
            expected = '\t'.join(matches[0][key] for key in
                                 ['appSha', 'coreSha', 'brokerSha', 'dnsReceiptSha']) + '\n'
            result = subprocess.run(['/bin/bash', '-c', 'source "$1"; _omavless_pair_pins "$2" "$3"',
                                     'fixture', str(inspector), source, arch],
                                    capture_output=True, text=True, timeout=5)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, expected)

    def test_target_frontend_identity_is_separate_from_compiled_predecessors(self):
        inspector = self.directory / 'setup-predecessor.sh'
        shutil.copyfile(INSPECTOR, inspector)
        shutil.copyfile(ROOT / 'plugin/published-pairs.json', self.directory / 'published-pairs.json')
        target = 'c' * 40
        keys = ['appSha','coreSha','brokerSha','dnsReceiptSha']
        identity = dict(zip(keys, self.fixture_pins.split('\t')))
        metadata = {'schemaVersion': 2, 'sourceCommit': target, 'updatePair': False,
                    'identities': {'x86_64': identity, 'aarch64': identity}}
        path = self.directory / 'component-updates.json'
        def query():
            return subprocess.run(['/bin/bash','-c','source "$1"; _omavless_pair_pins "$2" x86_64',
                                   'fixture', str(inspector), target], capture_output=True, text=True, timeout=5)
        path.write_text(json.dumps(metadata))
        self.assertEqual(query().stdout, self.fixture_pins + '\n')
        # A receipt's source/hash cannot select another target or a permissive
        # runtime registry; only the fixed reviewed frontend metadata can.
        for invalid in [{**metadata,'sourceCommit':'d'*40}, {**metadata,'command':'ignored'},
                        {**metadata,'identities':{}}, {**metadata,'updatePair':'true'}]:
            path.write_text(json.dumps(invalid))
            self.assertNotEqual(query().returncode,0)

    def test_selected_candidate_uses_real_frontend_identity_lookup_for_both_architectures(self):
        metadata = json.loads((ROOT / 'plugin/component-updates.json').read_text())
        app = json.loads((ROOT / 'plugin/runtime-release.json').read_text())
        dns = json.loads((ROOT / 'plugin/dns-release.json').read_text())
        self.assertEqual(metadata['schemaVersion'], 2)
        self.assertEqual(metadata['sourceCommit'], '9fb8d2b84c7ac6038c8ee1e5e52696694538a366')
        self.assertIs(metadata['updatePair'], True)
        self.assertEqual(set(metadata['identities']), {'x86_64', 'aarch64'})
        for arch in ('x86_64', 'aarch64'):
            self.assertEqual(app['packages'][arch]['sourceCommit'], metadata['sourceCommit'])
            self.assertEqual(dns['packages'][arch]['sourceCommit'], metadata['sourceCommit'])
            expected = '\t'.join(metadata['identities'][arch][key] for key in
                                  ('appSha', 'coreSha', 'brokerSha', 'dnsReceiptSha')) + '\n'
            result = subprocess.run(['/bin/bash', '-c',
                'source "$1"; _omavless_pair_pins "$2" "$3"',
                'fixture', str(INSPECTOR), metadata['sourceCommit'], arch],
                capture_output=True, text=True, timeout=5)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, expected)
            # A previous nonpublic native producer remains removable, but it is
            # not an updater predecessor or this frontend's selected identity.
            previous = subprocess.run(['/bin/bash', '-c',
                'source "$1"; _omavless_pair_pins "$2" "$3"',
                'fixture', str(INSPECTOR), '5281e02fb89b61e39859b0d4085d34599b848c1c', arch],
                capture_output=True, text=True, timeout=5)
            self.assertNotEqual(previous.returncode, 0)
            self.assertEqual(previous.stdout, '')

    def test_same_package_version_does_not_hide_a_previous_delivery(self):
        self.assertEqual(self.inspect(), "stale")
        self.delivery(TARGET)
        self.assertEqual(self.inspect(), "current")

    def test_every_recorded_public_producer_reaches_the_full_advisory_inspector(self):
        records = json.loads((ROOT / 'plugin/published-pairs.json').read_text())['pairs']
        for source in sorted({record['sourceCommit'] for record in records}):
            with self.subTest(source=source):
                self.delivery(source)
                self.assertEqual(self.inspect(), 'current' if source == TARGET else 'stale')
                self.app.write_bytes(b'modified inert fixture')
                self.assertEqual(self.inspect(), 'unknown')
                self.app.write_bytes(b'inert fixture executable; never execute')

    def test_mixed_receipts_are_not_an_ordinary_update(self):
        self.delivery(TARGET, OLD)
        self.assertEqual(self.inspect(), "mismatch")

    def test_missing_and_partial_payloads_are_distinct(self):
        self.core.unlink()
        self.assertEqual(self.inspect(), "unknown")
        self.broker.unlink()
        self.receipt.unlink()
        self.assertEqual(self.inspect(), "missing")

    def test_modified_payload_never_becomes_ready(self):
        for path in (self.app, self.core, self.broker):
            original = path.read_bytes()
            path.write_bytes(b"changed inert fixture")
            self.assertEqual(self.inspect(), "unknown")
            path.write_bytes(original)

    def test_developer_unknown_and_invalid_receipts_refuse(self):
        self.delivery("a" * 40)
        self.assertEqual(self.inspect(), "unknown")
        self.delivery(OLD)
        original = self.identity.read_text()
        for wrong in [original + f"sourceCommit={OLD}\n", original.replace("0.9.8", "0.9.7"),
                      original.replace("schemaVersion=3", "schemaVersion=2"), original.replace("x86_64", "aarch64")]:
            self.identity.write_text(wrong)
            self.assertEqual(self.inspect(), "unknown")

    def test_unsafe_modes_and_links_never_qualify(self):
        for path in (self.identity, self.receipt, self.app, self.core, self.broker):
            original = path.stat().st_mode & 0o7777
            path.chmod(0o666)
            self.assertEqual(self.inspect(), "unknown")
            path.chmod(original)
        original = self.app.read_bytes()
        self.app.unlink()
        self.app.symlink_to("missing")
        self.assertEqual(self.inspect(), "unknown")
        self.app.unlink()
        self.app.write_bytes(original)
        self.app.chmod(0o755)

    def test_invalid_dns_metadata_is_not_current_or_missing(self):
        original = self.receipt.read_text()
        for value in ["null", "{}", original.replace('"release"', '"developer"'),
                      original.replace('"x86_64"', '"aarch64"')]:
            self.receipt.write_text(value)
            self.assertEqual(self.inspect(), "unknown")

    def test_inventory_is_effect_free_and_preserves_all_members(self):
        before = {path.name: (path.read_bytes(), path.stat().st_mode, path.stat().st_mtime_ns)
                  for path in self.directory.iterdir()}
        self.assertEqual(self.inspect(), "stale")
        after = {path.name: (path.read_bytes(), path.stat().st_mode, path.stat().st_mtime_ns)
                 for path in self.directory.iterdir()}
        self.assertEqual(before, after)


if __name__ == "__main__":
    unittest.main()
