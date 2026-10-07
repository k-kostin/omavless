"""Inert textual export checks: no patch application, compiler or receive entry."""
import hashlib
import json
from pathlib import Path
import re
import stat
import unittest

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tests/research/rustix-owned-ancillary'
FILES = [
    'rustix-1.1.5/src/backend/libc/net/syscalls.rs',
    'rustix-1.1.5/src/backend/linux_raw/net/syscalls.rs',
    'rustix-1.1.5/src/net/send_recv/owned_ancillary_preallocated.rs',
    'rustix-1.1.5/src/net/send_recv/owned_ancillary_terminal.rs',
]


class TerminalExportControls(unittest.TestCase):
    def setUp(self):
        self.proof = json.loads((FIXTURE / 'terminal-provenance.json').read_text())
        self.patch = (FIXTURE / 'terminal-source-79975b1.patch').read_text()

    def test_exact_nonexecutable_patch_bytes(self):
        expected = {
            'terminal-source-79975b1.patch': (18823, 'e836be063215e738b346c741f0ce7f4dbd1d15bd5b2df54bc0a99a7c1f3f937c'),
            'terminal-docs-79975b1.patch': (1597, '5b8a3a8267685e266a036bf8b2873588e76644afc114e6ed1d05e8c08bed5f36'),
        }
        for name, (size, sha) in expected.items():
            path = FIXTURE / name
            raw = path.read_bytes(); info = path.lstat()
            self.assertTrue(stat.S_ISREG(info.st_mode))
            self.assertEqual(info.st_mode & 0o111, 0)
            self.assertEqual((len(raw), hashlib.sha256(raw).hexdigest()), (size, sha))
            self.assertEqual(self.proof['patches'][name], {'bytes': size, 'sha256': sha})

    def test_incremental_four_file_boundary(self):
        headers = [line for line in self.patch.splitlines() if line.startswith('diff --git ')]
        self.assertEqual(headers, [f'diff --git a/{name} b/{name}' for name in FILES])
        self.assertEqual(list(self.proof['result_files']), FILES)
        self.assertEqual(self.proof['baseline'], '0a0c958cd823b5be70abd38cf8def36b1554a089')
        self.assertEqual(self.proof['source'], '79975b10a0c21aa008bb6a8941459beb9bf35d2e')
        self.assertNotIn('/home/kk/', self.patch)
        self.assertNotIn('/home/kk/', (FIXTURE / 'terminal-provenance.json').read_text())

    def test_exact_terminal_module_and_seven_pure_controls(self):
        module = self.patch.split(f'diff --git a/{FILES[-1]} b/{FILES[-1]}', 1)[1]
        added = ''.join(line[1:] + '\n' for line in module.splitlines()
                        if line.startswith('+') and not line.startswith('+++'))
        self.assertEqual(hashlib.sha256(added.encode()).hexdigest(), self.proof['result_files'][FILES[-1]])
        tests = re.findall(r'#\[test\]\s+fn (\w+)\(', added)
        self.assertEqual(tests, self.proof['terminal_tests'])
        self.assertEqual(len(tests), 7)
        self.assertIn('let mut held = ManuallyDrop::new(value);', added)
        self.assertIn('On error this retains memory, NOT ownership of unreported installed FDs.', added)
        self.assertIn('Error field values are', added)
        self.assertIn('never passed to legacy Drop/parse', added)
        self.assertNotIn('#[ignore', added)
        self.assertNotIn('from_raw_fd', added)

    def test_four_doc_sites_only_add_docs_and_trailing_commas(self):
        patch = (FIXTURE / 'terminal-docs-79975b1.patch').read_text()
        changed = {'+': [], '-': []}
        for line in patch.splitlines():
            if line.startswith(('+++', '---')) or not line.startswith(('+', '-')):
                continue
            body = line[1:]
            if not body.lstrip().startswith('///'):
                changed[line[0]].append(body)
        # Rust permits the formatting-only trailing field commas in both forms.
        normalize = lambda lines: re.sub(r',}', '}', re.sub(r'\s+', '', ''.join(lines)))
        self.assertEqual(normalize(changed['+']), normalize(changed['-']))
        self.assertEqual(sum(line.startswith('+') and line[1:].lstrip().startswith('///')
                             for line in patch.splitlines()), 4)
        self.assertNotIn('allow(', patch)

    def test_narrow_evidence_is_not_product_or_error_recovery(self):
        self.assertEqual(self.proof['backends'], {
            'linux_raw': {'compiled': True, 'pure_controls_passed': 7},
            'use-libc': {'compiled': True, 'pure_controls_passed': 8},
        })
        self.assertEqual(self.proof['original_terminal_exit'], 0)
        for key in ('product_dependency', 'automatic_patch_application', 'actual_receive_selected',
                    'descriptor_callbacks_selected', 'musl_compiled', 'unknown_fd_recovery_proven',
                    'installed_unreported_fd_ownership_proven', 'whole_t4_accepted'):
            self.assertIs(self.proof[key], False)
        self.assertIn('terminal_header_count_refuses_narrowing_without_syscall', self.patch)
        self.assertFalse((FIXTURE / 'Cargo.toml').exists())
        self.assertNotIn('recvmsg_retained_snapshot',
                         (ROOT / 'crates/omavless-runtime/Cargo.toml').read_text())


if __name__ == '__main__':
    unittest.main()
