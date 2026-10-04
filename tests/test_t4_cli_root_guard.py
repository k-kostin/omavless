"""Pure terminal-flow controls. No account, manager, executable or VM effects."""
import hashlib
import json
import os
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

from tests.first_abort_cli import root_guard as guard
from tests.first_abort_cli import stage_loader as loader


class GuardTests(unittest.TestCase):
    def setUp(self):
        self.core = SimpleNamespace(UNCERTAIN=False, snapshot=Mock(return_value={'network': {}}),
                                    network_equal=Mock(return_value=True))
        item = patch.object(guard, 'core', self.core)
        item.start()
        self.addCleanup(item.stop)

    def fixture(self):
        obj = guard.Guard.__new__(guard.Guard)
        obj.value = {'guard_head': 'a' * 40}
        obj.manager_pid = 123
        obj.chain = Mock()
        obj.evidence = Mock()
        for name in ('source_admission', 'absent_account', 'create_account', 'manager_start',
                     'publish_elfs', 'normal_cli_case', 'manager_stopped_app', 'source_recheck'):
            setattr(obj, name, Mock())
        obj.run_child = Mock(return_value=(0, b'kvm\n', b''))
        obj.account_snapshot = Mock(return_value={'passwd': {'other_rows_sha256': 'a'}})
        obj.exact_fixture_absence = Mock(return_value=2)
        return obj

    def test_each_phase_failure_never_reaches_next_phase_or_result(self):
        names = ('source_admission', 'absent_account', 'create_account', 'manager_start',
                 'publish_elfs', 'normal_cli_case', 'manager_stopped_app',
                 'exact_fixture_absence', 'source_recheck')
        for index, name in enumerate(names):
            obj = self.fixture()
            getattr(obj, name).side_effect = RuntimeError('injected terminal failure')
            with self.assertRaises(RuntimeError):
                obj.execute()
            for later in names[index + 1:]:
                getattr(obj, later).assert_not_called()
            self.assertNotIn('result.json', [call.args[0] for call in obj.evidence.write.call_args_list])

    def test_before_baseline_write_failure_forbids_first_mutation(self):
        for failed in ('baseline-before.json', 'accounts-before.json'):
            obj = self.fixture()
            def write(name, value):
                if name == failed:
                    raise OSError('write or fsync failure')
            obj.evidence.write.side_effect = write
            with self.assertRaises(OSError):
                obj.execute()
            obj.absent_account.assert_not_called()
            obj.create_account.assert_not_called()

    def test_bad_after_baseline_never_publishes_result(self):
        obj = self.fixture()
        self.core.network_equal.return_value = False
        with self.assertRaises(guard.Refused):
            obj.execute()
        self.assertTrue(self.core.UNCERTAIN)
        obj.source_recheck.assert_not_called()
        self.assertNotIn('result.json', [call.args[0] for call in obj.evidence.write.call_args_list])

    def test_child_unknown_nonzero_bool_and_deadline_no_output_reads_or_retry(self):
        for status in (1, False, 'unknown', OSError('unknown')):
            self.core.UNCERTAIN = False
            obj = self.fixture()
            obj.count = 0
            obj.evidence.create.return_value = 10
            self.core.spawn = Mock(return_value=object())
            self.core.await_exact = Mock(side_effect=status if isinstance(status, Exception) else None,
                                         return_value=status)
            with patch.object(guard.os, 'fstat') as metadata, patch.object(guard.os, 'pread') as read:
                with self.assertRaises((guard.Refused, OSError)):
                    guard.Guard.run_child(obj, ['/fixed'], 0, 'pure')
                metadata.assert_not_called()
                read.assert_not_called()
            self.core.spawn.assert_called_once()
            self.core.await_exact.assert_called_once()
        self.core.UNCERTAIN = True
        obj = self.fixture()
        with self.assertRaises(guard.Refused):
            guard.Guard.run_child(obj, ['/fixed'], 0, 'pure')
        obj.evidence.create.assert_not_called()
        self.core.UNCERTAIN = False
        with patch.object(guard, 'DEADLINE', 0), self.assertRaises(guard.Refused):
            guard.available()
        self.assertTrue(self.core.UNCERTAIN)

    def test_strict_duplicate_field_bool_and_fixed_source_receipt(self):
        for data in (b'{"x":1,"x":2}', b'NaN'):
            with self.assertRaises(guard.Refused):
                guard.decode(data)
        with self.assertRaises(guard.Refused):
            guard.fields(b'MainPID=1\nMainPID=2\n', ('MainPID',))


class LoaderTests(unittest.TestCase):
    def test_real_files_all_admitted_before_publication_mode_hash_and_inode(self):
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            root.chmod(0o700)
            data = {name: b'pass\n' for name in loader.CODE}
            data.update({name: b'\x7fELFsynthetic' for name in loader.ELFS})
            value = {'schema': 't4-disposable-cli-delivery-v1',
                     'native_head': guard.NATIVE_HEAD, 'guard_head': 'a' * 40,
                     'code': {name: hashlib.sha256(data[name]).hexdigest() for name in loader.CODE},
                     'elfs': {name: {'sha256': hashlib.sha256(data[name]).hexdigest(), 'size': len(data[name]),
                         'host_original': [1, 2, 0o100755, 1000, 1000, 1, len(data[name]), 1, 1],
                         'host_frozen': [1, 3, 0o100500, 1000, 1000, 1, len(data[name]), 1, 1]}
                         for name in loader.ELFS}}
            data['receipt.json'] = json.dumps(value).encode()
            receipt_sha = hashlib.sha256(data['receipt.json']).hexdigest()
            directory = os.open(root, os.O_RDONLY | os.O_DIRECTORY)
            self.addCleanup(os.close, directory)
            real_fstat, real_stat = os.fstat, os.stat
            def synthetic_uid(s):
                # Only credentials are synthetic; actual native FD/path inode,
                # timestamps, mode, size and link count remain unchanged.
                names = ('st_dev', 'st_ino', 'st_mode', 'st_nlink', 'st_size', 'st_mtime_ns', 'st_ctime_ns')
                return SimpleNamespace(**{name: getattr(s, name) for name in names}, st_uid=1000, st_gid=1000)
            for fault in ('mode', 'hash', 'same-byte-inode'):
                for name, raw in data.items():
                    path = root / name
                    if path.exists():
                        path.chmod(0o600)
                    path.write_bytes(raw)
                    path.chmod(0o600 if name == 'receipt.json' else 0o500)
                last = root / 'omavless'
                if fault == 'mode':
                    last.chmod(0o700)
                if fault == 'hash':
                    last.chmod(0o600)
                    last.write_bytes(b'\x7fELFchanged')
                    last.chmod(0o500)
                real_recheck = loader.recheck
                changed = False
                def check(parent, name, row):
                    nonlocal changed
                    if fault == 'same-byte-inode' and name == 'omavless' and not changed:
                        changed = True
                        replacement = root / 'replacement'
                        replacement.write_bytes(data[name])
                        replacement.chmod(0o500)
                        replacement.replace(last)
                    return real_recheck(parent, name, row)
                held = []
                try:
                    # Each synthetic invocation gets a fresh enumeration offset.
                    os.lseek(directory, 0, os.SEEK_SET)
                    with patch.object(loader, 'HELD', held), \
                         patch.object(loader, 'parents', return_value=[(root, directory, synthetic_uid(root.stat()))]), \
                         patch.object(loader.os, 'fstat', side_effect=lambda fd: synthetic_uid(real_fstat(fd))), \
                         patch.object(loader.os, 'stat', side_effect=lambda *a, **k: synthetic_uid(real_stat(*a, **k))), \
                         patch.object(loader, 'admit', wraps=loader.admit) as admit, \
                         patch.object(loader, 'recheck', side_effect=check), \
                         patch.object(loader.os, 'mkdir') as publish:
                        with self.assertRaises(RuntimeError):
                            loader.deliver(receipt_sha)
                        publish.assert_not_called()
                        self.assertEqual([call.args[1] for call in admit.call_args_list],
                                         ['receipt.json', *loader.CODE, *loader.ELFS])
                finally:
                    for fd in held:
                        os.close(fd)

    def test_invalid_receipt_pin_before_any_filesystem_access(self):
        with patch.object(loader.os, 'open') as opened:
            for value in (None, True, 'x', 'a' * 63, 'A' * 64, '../guard'):
                with self.assertRaises(RuntimeError):
                    loader.deliver(value)
            opened.assert_not_called()

    def test_original_inode_hash_metadata_or_xattr_change_refuses(self):
        original = SimpleNamespace(st_dev=1, st_ino=2, st_mode=0o100500, st_uid=1000,
                                   st_gid=1000, st_nlink=1, st_size=4, st_mtime_ns=1, st_ctime_ns=1)
        for change in ('inode', 'hash', 'xattr', 'timestamp'):
            newer = SimpleNamespace(**vars(original))
            if change == 'inode':
                newer.st_ino += 1
            if change == 'timestamp':
                newer.st_ctime_ns += 1
            with patch.object(loader.os, 'fstat', return_value=original), \
                 patch.object(loader.os, 'stat', return_value=newer), \
                 patch.object(loader.os, 'listxattr', return_value=['x'] if change == 'xattr' else []), \
                 patch.object(loader, 'digest', return_value='wrong' if change == 'hash' else 'expected'):
                with self.assertRaises(RuntimeError):
                    loader.recheck(8, 'helper', (9, original, 'expected'))

    def test_no_delivered_code_execution_or_subprocess_surface(self):
        source = Path(loader.__file__).read_text()
        self.assertNotIn('exec(', source)
        self.assertNotIn('subprocess', source)
        self.assertNotIn('os.system', source)


if __name__ == '__main__':
    unittest.main()
