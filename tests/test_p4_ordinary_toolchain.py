"""Synthetic package/ELF controls and mocked runner paths; no tool queries."""
from contextlib import contextmanager
import hashlib
import importlib.util
import io
import json
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


def archive(entries, owners=None):
    out = io.BytesIO()
    with tarfile.open(fileobj=out, mode='w') as stream:
        for name, kind, payload, mode in entries:
            member = tarfile.TarInfo(name)
            member.mode = mode
            member.uid = (owners or {}).get(name, 0)
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
            ('usr/lib/go/src/cmd', 'directory', b'', 0o755),
            ('usr/lib/go/src/cmd/test2json', 'directory', b'', 0o755),
            ('usr/lib/go/src/cmd/internal', 'directory', b'', 0o755),
            ('usr/lib/go/src/cmd/internal/test2json', 'directory', b'', 0o755),
            *[('usr/lib/go/' + name, 'file', b'INERT_NOT_COMPILED_SOURCE', 0o644)
              for name in admission.REQUIRED_HELPER_SOURCE],
            ('usr/lib/go/doc', 'link', '/usr/share/doc/go', 0o777),
            ('usr/bin/go', 'link', '/usr/lib/go/bin/go', 0o777)]


class ToolchainControls(unittest.TestCase):
    def catalogue(self, entries):
        with patch.object(admission, 'GO_SHA256', hashlib.sha256(elf()).hexdigest()):
            return admission.catalogue(archive(entries))

    def test_pure_catalogue_binds_full_tree_and_explicit_go_link(self):
        tree, link = self.catalogue(valid_entries())
        self.assertEqual(len(tree), 20)
        self.assertEqual(link, '/usr/lib/go/bin/go')
        self.assertEqual(tree['usr/lib/go/src/empty.go']['size'], 0)
        self.assertEqual(tree['usr/lib/go/bin/go']['sha256'], hashlib.sha256(elf()).hexdigest())
        self.assertEqual(tree[admission.DOC_LINK], {'kind': 'documentation_symlink', 'mode': 0o777, 'target': admission.DOC_TARGET})

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

    def test_signed_catalogue_documentation_exception_is_exact(self):
        good = [entry for entry in valid_entries() if entry[0] != admission.DOC_LINK]
        for kind, target, mode in (('link', '/caller/docs', 0o777),
                                  ('link', admission.DOC_TARGET, 0o755),
                                  ('directory', b'', 0o777)):
            with self.subTest(kind=kind, target=target, mode=mode), self.assertRaises(ValueError):
                self.catalogue(good + [(admission.DOC_LINK, kind, target, mode)])
        with self.assertRaises(ValueError):
            self.catalogue(good)
        with patch.object(admission, 'GO_SHA256', hashlib.sha256(elf()).hexdigest()), self.assertRaises(ValueError):
            admission.catalogue(archive(valid_entries(), owners={admission.DOC_LINK: 1000}))

    def test_catalogue_requires_signed_helper_source_without_invented_tool_elf(self):
        self.assertNotIn('pkg/tool/linux_amd64/test2json', admission.REQUIRED_NATIVE_TOOLS)
        for name in admission.REQUIRED_HELPER_SOURCE:
            entries = [entry for entry in valid_entries() if entry[0] != 'usr/lib/go/' + name]
            with self.subTest(name=name), self.assertRaises(ValueError):
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
        doc = admission.GOROOT / 'doc'
        tree = {'usr/lib/go': {'kind': 'directory', 'mode': 0o755},
                'usr/lib/go/bin': {'kind': 'directory', 'mode': 0o755},
                'usr/lib/go/bin/go': {'kind': 'file', 'mode': 0o755, 'size': len(raw),
                                      'sha256': hashlib.sha256(raw).hexdigest()},
                admission.DOC_LINK: {'kind': 'documentation_symlink', 'mode': 0o777, 'target': admission.DOC_TARGET}}
        def metadata(mode, size=0):
            return SimpleNamespace(st_dev=1, st_ino=1, st_uid=0, st_gid=0,
                                   st_mode=mode, st_nlink=1, st_size=size,
                                   st_mtime_ns=1, st_ctime_ns=1)
        file_metadata = metadata(0o100755, len(raw))
        link_reads, directory_reads, doc_reads = [], [], []
        def observed(path):
            if path == admission.GO:
                link_reads.append(True)
                m = metadata(0o120777)
                if cut == 'link_owner': m.st_uid = 1000
                if cut == 'link_drift' and len(link_reads) > 1: m.st_ino = 2
                return m
            if path == doc:
                doc_reads.append(True)
                m = metadata(0o120777)
                if cut == 'doc_owner': m.st_uid = 1000
                if cut == 'doc_group': m.st_gid = 1000
                if cut == 'doc_mode': m.st_mode = 0o120755
                if cut == 'doc_type': m.st_mode = 0o100777
                if cut == 'doc_hardlink': m.st_nlink = 2
                if cut == 'doc_drift' and len(doc_reads) > 1: m.st_ino = 2
                return m
            if path in paths:
                m = metadata(0o040755) if path != paths[-1] else file_metadata
                if cut == 'metadata' and path == paths[1]: m.st_mode = 0o040777
                return m
            raise AssertionError('unselected synthetic path')
        def children(path):
            directory_reads.append(path)
            result = [paths[1], doc] if path == paths[0] else [paths[2]] if path == paths[1] else None
            if result is None: raise AssertionError('unexpected synthetic directory')
            if cut == 'extra' and path == paths[0]: result += [path / 'extra']
            if cut == 'directory_drift' and directory_reads.count(path) > 1: result = []
            return iter(result)
        def link_target(path):
            if path == admission.GO: return admission.GO_SYMLINK
            if path == doc:
                changed = cut == 'doc_target' or cut == 'doc_target_drift' and len(doc_reads) > 1
                return '/caller/docs' if changed else admission.DOC_TARGET
            raise AssertionError('unexpected synthetic link')
        def ancestry(path):
            if path not in (*paths, doc, admission.GO):
                raise AssertionError('documentation target was traversed')
        def read(path, *args, **kwargs):
            if path == admission.ARCHIVE:
                return package, metadata(0o100644, len(package))
            if path == paths[-1]:
                return (raw + b'DRIFT' if cut == 'bytes' else raw), file_metadata
            raise AssertionError('unexpected synthetic read')
        compression = SimpleNamespace(zstd=SimpleNamespace(ZstdFile=lambda unused: io.BytesIO(b'INERT_TAR')))
        with patch.dict(sys.modules, {'compression': compression}), \
                patch.object(admission, 'root_bytes', side_effect=read), \
                patch.object(admission, 'root_ancestry', side_effect=ancestry), \
                patch.object(admission, 'ARCHIVE_SIZE', len(package)), \
                patch.object(admission, 'ARCHIVE_SHA256', hashlib.sha256(package).hexdigest()), \
                patch.object(admission, 'catalogue', return_value=(tree, admission.GO_SYMLINK)), \
                patch.object(Path, 'lstat', observed), \
                patch.object(Path, 'iterdir', children), \
                patch.object(admission.os, 'listxattr', return_value=[]), \
                patch.object(admission.os, 'readlink', side_effect=link_target):
            yield

    def test_mocked_installed_tree_provenance_and_each_drift_cut(self):
        with self.installed_model():
            record = admission.admit()
            self.assertEqual(record['members'], 4)
            self.assertTrue(record['static_executable_closure'])
            self.assertFalse(record['hostile_root_atomic_attestation'])
            self.assertEqual(record['tree_scope'], 'lexical-goroot')
            self.assertFalse(record['documentation_target_traversed'])
        for cut in ('link_owner', 'link_drift', 'metadata', 'extra', 'bytes', 'directory_drift'):
            with self.subTest(cut=cut), self.installed_model(cut), self.assertRaises(ValueError):
                admission.admit()

    def test_mocked_documentation_link_metadata_target_and_drift_refuse(self):
        for cut in ('doc_owner', 'doc_group', 'doc_type', 'doc_mode', 'doc_hardlink', 'doc_drift', 'doc_target', 'doc_target_drift'):
            with self.subTest(cut=cut), self.installed_model(cut), self.assertRaises(ValueError):
                admission.admit()


class OrdinaryRunnerControls(unittest.TestCase):
    @contextmanager
    def harness(self, failure=None):
        # Reuse only baseline's synthetic coordinator. All admission is mocked;
        # its archive/system-tree function is never invoked by these controls.
        toolchain = {'go_sha256': 'a' * 64, 'tree_sha256': 'b' * 64}
        with patch.object(baseline, 'subject', runner), \
                patch.object(runner.admission, 'admit', return_value=toolchain) as admit, \
                patch.object(runner.owned, 'UNSETTLED', []):
            with baseline.RunnerControls().harness(failure) as state:
                paths, _, _, _, _, _, commands, _ = state
                def command(args, *unused):
                    kind = 'modules' if args[1] == 'mod' else 'helper' if args[1] == 'build' else 'compile' if args[1] == 'test' else 'execute'
                    if failure == kind: return 2, b'', b'INERT_FIXED_NONPASS'
                    if failure == kind + '_unknown':
                        runner.owned.UNSETTLED.append(object())
                        raise runner.owned.Unsettled('INERT_GRAPH_UNKNOWN')
                    if kind == 'modules': return 0, b'all modules verified\n', b''
                    if kind in ('helper', 'compile'):
                        name = 'test2json' if kind == 'helper' else 'device.test'
                        runner.owned.save(paths['artifacts'], name, elf(), 0o700)
                        return 0, b'', b''
                    from tests.test_p4_default_residue_source import ReceiptControls
                    control = ReceiptControls()
                    return 0, control.encoded(control.events('reject')), b''
                commands.side_effect = command
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

    def test_new_ordinary_nproc_bound_preserves_stricter_soft_and_hard_limits(self):
        for inherited, expected in ((runner.resource.RLIM_INFINITY, 2048),
                                    (1024, 1024), (4096, 2048)):
            calls = []
            with patch.object(runner.resource, 'getrlimit', return_value=(inherited, 8192)), \
                    patch.object(runner.resource, 'setrlimit', side_effect=lambda kind, value: calls.append((kind, value))):
                runner.limits()
            self.assertEqual(dict(calls)[runner.resource.RLIMIT_NPROC], (expected, 8192))
            self.assertTrue(all(value[1] == 8192 for _, value in calls))
        source = (ROOT / 'run_default_residue_ordinary.py').read_text()
        self.assertIn("'nproc':2048", source)
        self.assertIn('recipe="p4-default-residue-ordinary-arch-go-v3"', source)
        old = (ROOT / 'run_default_residue_overlay.py').read_text()
        self.assertIn('(resource.RLIMIT_NPROC,1024)', old)
        self.assertIn("'nproc':1024", old)

    def test_mocked_build_execute_rechecks_before_commands_and_final_receipt(self):
        with self.harness() as (state, admit):
            paths, argv, _, _, _, saved, commands, _ = state
            runner.main()
            self.assertEqual(admit.call_count, 5)
            self.assertEqual(saved[-1], 'build-receipt.json')
            self.assertEqual(commands.call_count, 3)
            for call in commands.call_args_list:
                self.assertEqual(call.args[0][0], '/usr/bin/go')
                self.assertEqual(call.args[2]['GOROOT'], '/usr/lib/go')
                self.assertEqual(call.args[2]['CGO_ENABLED'], '0')
                self.assertEqual(call.args[2]['TEST_TELEMETRY_DIR'], str(paths['artifacts'] / 'configuration/go/telemetry'))
                self.assertEqual(call.args[2]['XDG_CONFIG_HOME'], str(paths['artifacts'] / 'configuration'))
                self.assertEqual(call.args[2]['XDG_CACHE_HOME'], str(paths['cache']))
            self.assertEqual(commands.call_args_list[1].args[0], ['/usr/bin/go', 'build', '-p=2', '-trimpath', '-o', str(paths['artifacts'] / 'test2json'), 'cmd/test2json'])
            receipt = json.loads((paths['artifacts'] / 'build-receipt.json').read_bytes())
            self.assertEqual(receipt['test2json_sha256'], hashlib.sha256(elf()).hexdigest())
            self.assertEqual(receipt['binary_sha256'], hashlib.sha256(elf()).hexdigest())
            argv[argv.index('build')] = 'execute'
            runner.main()
            self.assertEqual(admit.call_count, 8)
            self.assertEqual(saved[-1], 'execution-receipt.json')
            self.assertTrue((paths['artifacts'] / 'execution-attempt.json').exists())
            self.assertEqual(commands.call_args.args[0], [str(paths['artifacts'] / 'test2json'), '-p', runner.PACKAGE, '-t', str(paths['artifacts'] / 'device.test'), '-test.v=test2json', '-test.run=^TestP4DefaultElapsedRejectAndResidue$', '-test.count=1', '-test.timeout=600s'])

    def test_mocked_toolchain_drift_prevents_each_command_and_final_receipt(self):
        for cut in (1, 2, 3, 4):
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
            self.assertEqual(admit.call_count, 4)
            self.assertTrue(exported[0].exists())
            self.assertNotIn('build-receipt.json', saved)
            output.assert_not_called()

    def test_mocked_failed_or_unknown_helper_build_stops_before_device_compile(self):
        for failure in ('helper', 'helper_unknown'):
            with self.subTest(failure=failure), self.harness(failure) as (state, admit):
                _, _, _, verify, exported, saved, commands, output = state
                with self.assertRaises((ValueError, runner.owned.Unsettled)):
                    runner.main()
                self.assertEqual(commands.call_count, 2)
                self.assertEqual(admit.call_count, 3)
                self.assertEqual(verify.call_count, 3)
                self.assertTrue(exported[0].exists())
                self.assertNotIn('build-receipt.json', saved)
                self.assertNotIn('build.stdout', saved)
                if failure == 'helper_unknown':
                    self.assertNotIn('test2json-build.stdout', saved)
                output.assert_not_called()

    def test_mocked_build_refuses_dynamic_helper_or_device_elf(self):
        for kind in ('helper', 'compile'):
            with self.subTest(kind=kind), self.harness() as (state, _):
                paths, _, _, _, _, saved, commands, _ = state
                original = commands.side_effect
                def dynamic(args, *rest):
                    result = original(args, *rest)
                    if args[1] == ('build' if kind == 'helper' else 'test'):
                        name = 'test2json' if kind == 'helper' else 'device.test'
                        (paths['artifacts'] / name).write_bytes(elf(3))
                    return result
                commands.side_effect = dynamic
                with self.assertRaises(ValueError): runner.main()
                self.assertNotIn('build-receipt.json', saved)
                self.assertEqual(commands.call_count, 2 if kind == 'helper' else 3)

    def test_mocked_execute_never_regenerates_missing_or_changed_helper(self):
        for cut in ('missing', 'bytes', 'mode', 'hardlink'):
            with self.subTest(cut=cut), self.harness() as (state, _):
                paths, argv, _, _, _, saved, commands, _ = state
                runner.main()
                helper = paths['artifacts'] / 'test2json'
                argv[argv.index('build')] = 'execute'
                if cut == 'missing': helper.unlink()
                elif cut == 'bytes': helper.write_bytes(elf() + b'INERT_DRIFT')
                elif cut == 'mode': helper.chmod(0o600)
                else: baseline.os.link(helper, paths['artifacts'] / 'inert-alias')
                with self.assertRaises((OSError, ValueError)): runner.main()
                self.assertEqual(commands.call_count, 3)
                self.assertNotIn('execution-attempt.json', saved)
                self.assertNotIn('execution-receipt.json', saved)

    def test_mocked_pre_and_post_execute_helper_drift_refuse_without_receipt(self):
        for moment in ('before', 'after'):
            with self.subTest(moment=moment), self.harness() as (state, admit):
                paths, argv, _, _, _, saved, commands, _ = state
                runner.main()
                argv[argv.index('build')] = 'execute'
                helper = paths['artifacts'] / 'test2json'
                if moment == 'before':
                    good = admit.return_value
                    observed = []
                    def change():
                        observed.append(True)
                        if len(observed) == 2: helper.write_bytes(elf() + b'INERT_DRIFT')
                        return good
                    admit.side_effect = change
                else:
                    original = commands.side_effect
                    def change(args, *rest):
                        result = original(args, *rest)
                        helper.write_bytes(elf() + b'INERT_DRIFT')
                        return result
                    commands.side_effect = change
                with self.assertRaises(ValueError): runner.main()
                self.assertEqual(commands.call_count, 3 if moment == 'before' else 4)
                self.assertEqual('execution-attempt.json' in saved, moment == 'after')
                self.assertNotIn('execution-receipt.json', saved)

    def test_mocked_private_telemetry_is_off_and_never_recreated_in_execute(self):
        for cut in ('mode', 'missing', 'counter_data'):
            with self.subTest(cut=cut), self.harness() as (state, _):
                paths, argv, _, _, _, saved, commands, _ = state
                runner.main()
                telemetry = paths['artifacts'] / 'configuration/go/telemetry'
                if cut == 'mode': (telemetry / 'mode').write_bytes(b'local\n')
                elif cut == 'missing': (telemetry / 'mode').unlink()
                else: runner.owned.save(telemetry, 'counter-data', b'INERT')
                argv[argv.index('build')] = 'execute'
                with self.assertRaises((OSError, ValueError)): runner.main()
                self.assertEqual(commands.call_count, 3)
                self.assertNotIn('execution-attempt.json', saved)
        with self.harness() as (state, _):
            paths, argv, _, _, _, _, _, _ = state
            runner.main()
            telemetry = paths['artifacts'] / 'configuration/go/telemetry'
            (telemetry / 'local').mkdir(mode=0o700)
            (telemetry / 'upload').mkdir(mode=0o700)
            argv[argv.index('build')] = 'execute'
            runner.main()  # Only empty fixed private directories may coexist.

    def test_unknown_latch_prevents_toolchain_observation(self):
        with patch.object(runner.owned, 'UNSETTLED', [object()]), \
                patch.object(runner.admission, 'admit') as admit, \
                patch.object(runner, 'inputs') as inputs:
            with self.assertRaises(runner.owned.Unsettled):
                runner.recheck({}, 'inert', {}, Path('inert'), Path('inert'), Path('inert'))
            admit.assert_not_called()
            inputs.assert_not_called()


if __name__ == '__main__':
    unittest.main()
