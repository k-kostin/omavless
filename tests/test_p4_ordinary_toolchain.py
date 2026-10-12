"""Synthetic package/ELF controls and mocked runner paths; no tool queries."""
from contextlib import contextmanager
import hashlib
import importlib.util
import io
from pathlib import Path
import struct
import sys
import tarfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from tests import test_p4_default_residue_runner as baseline

ROOT = Path(__file__).parent / 'fixtures/p4_awg_peer'


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, ROOT / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


admission = load('p4_inert_toolchain', 'ordinary_toolchain_admission.py')
runner = load('p4_inert_ordinary', 'run_default_residue_ordinary.py')


def elf(ptype=1, dynamic=None):
    raw = bytearray(160)
    raw[:7] = b'\x7fELF\x02\x01\x01'
    struct.pack_into('<HHI', raw, 16, 2, 62, 1)
    struct.pack_into('<Q', raw, 32, 64)
    struct.pack_into('<HHH', raw, 52, 64, 56, 1)
    struct.pack_into('<IIQQQQQQ', raw, 64, ptype, 5, 128, 0, 0, 32, 32, 8)
    if dynamic is not None:
        struct.pack_into('<qQ', raw, 128, dynamic, 0)
    return bytes(raw)


def archive(entries):
    out = io.BytesIO()
    with tarfile.open(fileobj=out, mode='w') as stream:
        for name, kind, payload, mode in entries:
            member = tarfile.TarInfo(name)
            member.mode = mode
            if kind == 'directory':
                member.type = tarfile.DIRTYPE
            elif kind == 'link':
                member.type = tarfile.SYMTYPE
                member.linkname = payload
            else:
                member.size = len(payload)
            stream.addfile(member, io.BytesIO(payload) if kind == 'file' else None)
    return out.getvalue()


def valid_entries():
    return [('usr/lib/go', 'directory', b'', 0o755),
            ('usr/lib/go/bin', 'directory', b'', 0o755),
            ('usr/lib/go/bin/go', 'file', elf(), 0o755),
            ('usr/lib/go/src', 'directory', b'', 0o755),
            ('usr/lib/go/src/empty.go', 'file', b'', 0o644),
            ('usr/lib/go/pkg', 'directory', b'', 0o755),
            ('usr/lib/go/pkg/tool', 'directory', b'', 0o755),
            ('usr/lib/go/pkg/tool/linux_amd64', 'directory', b'', 0o755),
            *[('usr/lib/go/' + name, 'file', elf(), 0o755)
              for name in admission.REQUIRED_NATIVE_TOOLS if name != 'bin/go'],
            ('usr/bin/go', 'link', '/usr/lib/go/bin/go', 0o777)]


class ToolchainControls(unittest.TestCase):
    def catalogue(self, entries):
        with patch.object(admission, 'GO_SHA256', hashlib.sha256(elf()).hexdigest()):
            return admission.catalogue(archive(entries))

    def test_pure_catalogue_binds_full_tree_and_explicit_go_link(self):
        tree, link = self.catalogue(valid_entries())
        self.assertEqual(len(tree), 13)
        self.assertEqual(link, '/usr/lib/go/bin/go')
        self.assertEqual(tree['usr/lib/go/src/empty.go']['size'], 0)
        self.assertEqual(tree['usr/lib/go/bin/go']['sha256'], hashlib.sha256(elf()).hexdigest())

    def test_catalogue_refuses_duplicates_escape_and_missing_ancestry(self):
        good = valid_entries()
        for entries in (good + [good[0]], good + [('../escape', 'file', b'x', 0o644)],
                        [x for x in good if x[0] != 'usr/lib/go/bin'],
                        [x for x in good if x[0] != 'usr/bin/go'],
                        good[:-1] + [('usr/bin/go', 'link', '/caller/go', 0o777)],
                        good + [('usr/lib/go/alias', 'link', 'bin/go', 0o777)],
                        good + [('usr/lib/go/unsafe', 'file', b'x', 0o666)]):
            with self.subTest(entries=entries), self.assertRaises(ValueError):
                self.catalogue(entries)

    def test_elf_refuses_interpreter_shared_dependencies_and_malformed_headers(self):
        admission.static_elf(elf())
        admission.static_elf(elf(2, 0))
        malformed = bytearray(elf())
        struct.pack_into('<H', malformed, 56, 65535)
        for raw in (b'#!/bin/sh', elf(3), elf(2, 1), bytes(malformed), elf()[:100]):
            with self.subTest(raw=raw[:8]), self.assertRaises(ValueError):
                admission.static_elf(raw)

    def test_checker_has_fixed_package_no_execution_or_extraction_authority(self):
        source = (ROOT / 'ordinary_toolchain_admission.py').read_text()
        for forbidden in ('subprocess', 'extractall(', 'extract(', 'os.system(', 'argparse'):
            # The docstring explicitly denies subprocess authority.
            self.assertNotIn(forbidden, source.split('import hashlib', 1)[1])
        self.assertEqual(admission.GOROOT, Path('/usr/lib/go'))
        self.assertEqual(admission.GO, Path('/usr/bin/go'))
        self.assertIn("from compression import zstd", source)
        self.assertIn("identity(path.lstat()) != metadata", source)

    @contextmanager
    def installed_model(self, cut=None):
        # Every filesystem/compression operation is fabricated. Unexpected paths
        # raise rather than fall back to the real installed tree/package.
        package = b'INERT_COMPRESSED_PACKAGE_NEVER_DECOMPRESSED'
        raw = elf()
        paths = [admission.GOROOT, admission.GOROOT / 'bin', admission.GOROOT / 'bin/go']
        tree = {'usr/lib/go': {'kind': 'directory', 'mode': 0o755},
                'usr/lib/go/bin': {'kind': 'directory', 'mode': 0o755},
                'usr/lib/go/bin/go': {'kind': 'file', 'mode': 0o755, 'size': len(raw),
                                      'sha256': hashlib.sha256(raw).hexdigest()}}
        def metadata(mode, size=0):
            return SimpleNamespace(st_dev=1, st_ino=1, st_uid=0, st_gid=0,
                                   st_mode=mode, st_nlink=1, st_size=size,
                                   st_mtime_ns=1, st_ctime_ns=1)
        file_metadata = metadata(0o100755, len(raw))
        link_reads, directory_reads = [], []
        def observed(path):
            if path == admission.GO:
                link_reads.append(True)
                m = metadata(0o120777)
                if cut == 'link_owner': m.st_uid = 1000
                if cut == 'link_drift' and len(link_reads) > 1: m.st_ino = 2
                return m
            if path in paths:
                m = metadata(0o040755) if path != paths[-1] else file_metadata
                if cut == 'metadata' and path == paths[1]: m.st_mode = 0o040777
                return m
            raise AssertionError('unselected synthetic path')
        def children(path):
            directory_reads.append(path)
            result = [paths[1]] if path == paths[0] else [paths[2]] if path == paths[1] else None
            if result is None: raise AssertionError('unexpected synthetic directory')
            if cut == 'extra' and path == paths[0]: result += [path / 'extra']
            if cut == 'directory_drift' and directory_reads.count(path) > 1: result = []
            return iter(result)
        def read(path, *args, **kwargs):
            if path == admission.ARCHIVE:
                return package, metadata(0o100644, len(package))
            if path == paths[-1]:
                return (raw + b'DRIFT' if cut == 'bytes' else raw), file_metadata
            raise AssertionError('unexpected synthetic read')
        compression = SimpleNamespace(zstd=SimpleNamespace(ZstdFile=lambda unused: io.BytesIO(b'INERT_TAR')))
        with patch.dict(sys.modules, {'compression': compression}), \
                patch.object(admission, 'root_bytes', side_effect=read), \
                patch.object(admission, 'root_ancestry'), \
                patch.object(admission, 'ARCHIVE_SIZE', len(package)), \
                patch.object(admission, 'ARCHIVE_SHA256', hashlib.sha256(package).hexdigest()), \
                patch.object(admission, 'catalogue', return_value=(tree, admission.GO_SYMLINK)), \
                patch.object(Path, 'lstat', observed), \
                patch.object(Path, 'iterdir', children), \
                patch.object(admission.os, 'listxattr', return_value=[]), \
                patch.object(admission.os, 'readlink', return_value=admission.GO_SYMLINK):
            yield

    def test_mocked_installed_tree_provenance_and_each_drift_cut(self):
        with self.installed_model():
            record = admission.admit()
            self.assertEqual(record['members'], 3)
            self.assertTrue(record['static_executable_closure'])
            self.assertFalse(record['hostile_root_atomic_attestation'])
        for cut in ('link_owner', 'link_drift', 'metadata', 'extra', 'bytes', 'directory_drift'):
            with self.subTest(cut=cut), self.installed_model(cut), self.assertRaises(ValueError):
                admission.admit()


class OrdinaryRunnerControls(unittest.TestCase):
    @contextmanager
    def harness(self, failure=None):
        # Reuse only baseline's synthetic coordinator. All admission is mocked;
        # its archive/system-tree function is never invoked by these controls.
        toolchain = {'go_sha256': 'a' * 64, 'tree_sha256': 'b' * 64}
        with patch.object(baseline, 'subject', runner), \
                patch.object(runner.admission, 'admit', return_value=toolchain) as admit:
            with baseline.RunnerControls().harness(failure) as state:
                yield state, admit

    def test_fixed_ordinary_environment_and_frozen_baseline(self):
        self.assertEqual(hashlib.sha256((ROOT / 'run_default_residue_overlay.py').read_bytes()).hexdigest(), runner.BASE_RUNNER_SHA)
        self.assertEqual(len(runner.inputs()), 9)
        source = (ROOT / 'run_default_residue_ordinary.py').read_text()
        for field in ('GOROOT="/usr/lib/go"', 'GOTOOLCHAIN="local"', 'CGO_ENABLED="0"',
                      'body_limit_seconds=570', 'go_timeout_seconds=600',
                      'supervisor_timeout_seconds=620'):
            self.assertIn(field, source)
        for forbidden in ('parser.add_argument("--race"', 'build.append("-race")', 'CC=', 'CXX=', 'rmtree'):
            self.assertNotIn(forbidden, source)

    def test_mocked_build_execute_rechecks_before_commands_and_final_receipt(self):
        with self.harness() as (state, admit):
            paths, argv, _, _, _, saved, commands, _ = state
            runner.main()
            self.assertEqual(admit.call_count, 4)
            self.assertEqual(saved[-1], 'build-receipt.json')
            self.assertEqual(commands.call_count, 2)
            for call in commands.call_args_list:
                self.assertEqual(call.args[0][0], '/usr/bin/go')
                self.assertEqual(call.args[2]['GOROOT'], '/usr/lib/go')
                self.assertEqual(call.args[2]['CGO_ENABLED'], '0')
            argv[argv.index('build')] = 'execute'
            runner.main()
            self.assertEqual(admit.call_count, 7)
            self.assertEqual(saved[-1], 'execution-receipt.json')
            self.assertTrue((paths['artifacts'] / 'execution-attempt.json').exists())

    def test_mocked_toolchain_drift_prevents_each_command_and_final_receipt(self):
        for cut in (1, 2, 3):
            with self.subTest(cut=cut), self.harness() as (state, admit):
                _, _, _, _, _, saved, commands, output = state
                good = admit.return_value
                admit.side_effect = [good] * cut + [{**good, 'tree_sha256': 'c' * 64}]
                with self.assertRaises(ValueError):
                    runner.main()
                self.assertEqual(commands.call_count, cut - 1)
                self.assertNotIn('build-receipt.json', saved)
                output.assert_not_called()

    def test_mocked_failed_compile_never_issues_build_receipt(self):
        with self.harness('compile') as (state, admit):
            _, _, _, _, exported, saved, _, output = state
            with self.assertRaises(ValueError):
                runner.main()
            self.assertEqual(admit.call_count, 3)
            self.assertTrue(exported[0].exists())
            self.assertNotIn('build-receipt.json', saved)
            output.assert_not_called()

    def test_unknown_latch_prevents_toolchain_observation(self):
        with patch.object(runner.owned, 'UNSETTLED', [object()]), \
                patch.object(runner.admission, 'admit') as admit, \
                patch.object(runner, 'inputs') as inputs:
            with self.assertRaises(runner.owned.Unsettled):
                runner.recheck({}, 'inert', {}, Path('inert'))
            admit.assert_not_called()
            inputs.assert_not_called()


if __name__ == '__main__':
    unittest.main()
