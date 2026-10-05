"""Read-only source-fixture checks; no patch application, Cargo or native entry."""
import hashlib
from pathlib import Path
import stat
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tests/research/prepared-local-reply'
PINS = {
    'source-only-68f618c.patch': (14036, '348e6c3ef5da4a4b7e0c272a0054cd2c2d81b0287f5603a01e06789e78642e13'),
    'private-driver.toml.in': (1616, 'cb3b94c96ba5687cbd1f53e0c5ba21e3c462e37c477f7013ad660cb54f1e4b46'),
    'resolver-Cargo.lock.txt': (49718, '1b132245a15e024b3f23b5a018ec01e9891058f05eea81ccb09cefe4d6d7b71e'),
}

class ExportControls(unittest.TestCase):
    def test_exact_inert_fixture_bytes(self):
        for name, (size, sha) in PINS.items():
            path = FIXTURE / name
            info = path.lstat(); raw = path.read_bytes()
            self.assertTrue(stat.S_ISREG(info.st_mode))
            self.assertEqual(info.st_mode & 0o111, 0)
            self.assertEqual((len(raw), hashlib.sha256(raw).hexdigest()), (size, sha))

    def test_patch_only_exports_two_source_files(self):
        raw = (FIXTURE / 'source-only-68f618c.patch').read_text()
        headers = [line for line in raw.splitlines() if line.startswith('diff --git ')]
        self.assertEqual(headers, [
            'diff --git a/crates/omavless-runtime/src/restore_abort_prepared_local_reply.rs b/crates/omavless-runtime/src/restore_abort_prepared_local_reply.rs',
            'diff --git a/crates/omavless-runtime/src/restore_abort_retained_parent_prototype.rs b/crates/omavless-runtime/src/restore_abort_retained_parent_prototype.rs',
        ])
        self.assertNotIn('/home/kk/', raw)

    def test_new_module_exact_hash_and_slice_regression(self):
        raw = (FIXTURE / 'source-only-68f618c.patch').read_text()
        module = raw.split('diff --git ', 2)[1]
        added = ''.join(line[1:]+'\n' for line in module.splitlines()
                        if line.startswith('+') and not line.startswith('+++'))
        self.assertEqual(hashlib.sha256(added.encode()).hexdigest(),
                         '8599de705f67f7f071546e095e9d2981dd6cf69eb0b042b7806067803db96dcf')
        self.assertIn('[b"wrong".as_slice(), b"public".as_slice()].concat()', added)
        self.assertIn('let operands: [&[u8]; 2]', added)
        self.assertIn('#[ignore = "SOURCE ONLY', added)
        self.assertIn('frames.push(prepared.receive(', added)

    def test_template_is_not_an_ordinary_manifest(self):
        path = FIXTURE / 'private-driver.toml.in'
        driver = tomllib.loads(path.read_text())
        self.assertFalse(driver['package']['publish'])
        self.assertEqual(driver['dev-dependencies']['prepared-rustix']['path'],
                         '@PREPARED_RUSTIX_0A0C958_HARNESS@')
        self.assertEqual(driver['lib']['path'], '../crates/omavless-runtime/src/lib.rs')
        self.assertFalse((FIXTURE / 'Cargo.toml').exists())
        self.assertNotIn('/home/kk/', path.read_text())

    def test_resolver_is_exact_private_review_graph(self):
        lock = tomllib.loads((FIXTURE / 'resolver-Cargo.lock.txt').read_text())
        self.assertEqual(len(lock['package']), 212)
        names = {entry['name'] for entry in lock['package']}
        self.assertIn('rustix-owned-ancillary-local-harness', names)
        self.assertIn('omavless-prepared-local-reply-source-experiment', names)

    def test_normal_runtime_does_not_enable_export(self):
        self.assertFalse((ROOT / 'crates/omavless-runtime/src/restore_abort_prepared_local_reply.rs').exists())
        self.assertNotIn('prepared-local-reply', (ROOT / 'crates/omavless-runtime/Cargo.toml').read_text())
        self.assertNotIn('prepared_local_reply',
                         (ROOT / 'crates/omavless-runtime/src/restore_abort_retained_parent_prototype.rs').read_text())

if __name__ == '__main__':
    unittest.main()
