"""Pure terminal-flow controls. No account, manager, executable or VM effects."""
import hashlib
import errno
import json
import os
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

from tests.first_abort_cli import root_guard as guard
from tests.first_abort_cli import stage_loader as loader
from tests.first_abort_process import vm_guard as owned_core


class GuardTests(unittest.TestCase):
    def test_mail_defaults_requires_one_exact_no_never_override_or_warning(self):
        for raw in (b'CREATE_MAIL_SPOOL=no\n', b'# comment\nGROUP=100\nCREATE_MAIL_SPOOL=no\n'):
            guard.mail_spool_disabled(raw)
        for raw in (b'', b'GROUP=100\n', b'CREATE_MAIL_SPOOL=yes\n',
                    b'CREATE_MAIL_SPOOL=no\nCREATE_MAIL_SPOOL=no\n',
                    b'CREATE_MAIL_SPOOL=no\nCREATE_MAIL_SPOOL=yes\n',
                    b' CREATE_MAIL_SPOOL=no\n', b'CREATE_MAIL_SPOOL="no"\n',
                    b'CREATE_MAIL_SPOOL=no # ignored?\n', b'CREATE_MAIL_SPOOL=no\r\n',
                    b'CREATE_MAIL_SPOOL=no\0', b'x'*65537):
            self.core.UNCERTAIN = False
            with self.subTest(raw=raw[:32]), self.assertRaises(guard.Refused):
                guard.mail_spool_disabled(raw)
        self.core.UNCERTAIN = False

    def test_fresh_instance_absence_rechecks_before_effect(self):
        obj = guard.Guard.__new__(guard.Guard)
        parent = SimpleNamespace(rows=[(Path('/fixed'), 17, None)], recheck=Mock())
        with patch.object(guard.os, 'stat', side_effect=FileNotFoundError) as read:
            obj.check_fresh_instances(parent, None)
            self.assertEqual(read.call_count, 4)
            self.assertEqual(parent.recheck.call_count, 2)
        with patch.object(guard.os, 'stat', return_value=object()), self.assertRaises(guard.Refused):
            obj.check_fresh_instances(parent, 'missing-root')

    def test_account_argv_no_unsupported_mail_key_and_no_effect_after_failure(self):
        obj = guard.Guard.__new__(guard.Guard)
        obj.source_recheck = Mock()
        obj.run_child = Mock(side_effect=[(0,b'',b''), RuntimeError('known refusal')])
        with patch.object(guard.os,'mkdir') as mkdir, self.assertRaises(RuntimeError):
            obj.create_account()
        mkdir.assert_not_called()
        argv = obj.run_child.call_args_list[1].args[0]
        self.assertIn('--no-create-home',argv)
        self.assertIn('--no-log-init',argv)
        self.assertNotIn('CREATE_MAIL_SPOOL=no',argv)
        self.assertIn('SUB_UID_COUNT=0',argv)
        self.assertIn('SUB_GID_COUNT=0',argv)
        self.assertIn('48045',argv)
        self.assertNotIn('48044',argv)

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
        for name in ('source_admission', 'absent_account', 'remaining_capacity', 'create_account', 'manager_start',
                     'publish_elfs', 'normal_cli_case', 'manager_stopped_app', 'source_recheck'):
            setattr(obj, name, Mock())
        obj.run_child = Mock(return_value=(0, b'kvm\n', b''))
        obj.account_snapshot = Mock(return_value={'passwd': {'other_rows_sha256': 'a'}})
        obj.exact_fixture_absence = Mock(return_value=2)
        return obj

    def test_each_phase_failure_never_reaches_next_phase_or_result(self):
        names = ('source_admission', 'absent_account', 'remaining_capacity', 'create_account', 'manager_start',
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

    def test_pre_account_capacity_refusal_seals_before_any_mutation(self):
        obj = self.fixture()
        obj.pins = {name: SimpleNamespace(before=SimpleNamespace(st_size=100))
                    for name in ('helper', 'omavless')}
        parent = SimpleNamespace(rows=[(Path('/fixed'), 10, None)], recheck=Mock())
        obj.remaining_capacity = lambda: guard.Guard.remaining_capacity(obj)
        with patch.object(guard, 'Parents', return_value=parent), \
             patch.object(guard.os, 'fstat', return_value=SimpleNamespace(st_dev=1)), \
             patch.object(guard.os, 'fstatvfs', return_value=SimpleNamespace(f_bavail=1, f_frsize=1)):
            with self.assertRaises(guard.Refused): obj.execute()
        self.assertTrue(self.core.UNCERTAIN)
        obj.create_account.assert_not_called(); obj.manager_start.assert_not_called()
        obj.publish_elfs.assert_not_called(); obj.normal_cli_case.assert_not_called()
        self.assertEqual(self.core.snapshot.call_count, 1)

    def test_pre_account_same_device_reserves_home_copy_and_both_headrooms(self):
        obj = self.fixture()
        obj.pins = {name: SimpleNamespace(before=SimpleNamespace(st_size=100))
                    for name in ('helper', 'omavless')}
        parent = SimpleNamespace(rows=[(Path('/fixed'), 10, None)], recheck=Mock())
        minimum = 200 + 1024 * 1024 * 1024
        for free, accepted in ((minimum, True), (minimum - 1, False)):
            self.core.UNCERTAIN = False
            with patch.object(guard, 'Parents', return_value=parent), \
                 patch.object(guard.os, 'fstat', return_value=SimpleNamespace(st_dev=1)), \
                 patch.object(guard.os, 'fstatvfs', return_value=SimpleNamespace(f_bavail=free, f_frsize=1)):
                if accepted: guard.Guard.remaining_capacity(obj)
                else:
                    with self.assertRaises(guard.Refused): guard.Guard.remaining_capacity(obj)

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
            with patch.object(guard, 'await_allowed', side_effect=status if isinstance(status, Exception) else None,
                              return_value=status) as awaited, \
                 patch.object(guard.os, 'fstat') as metadata, patch.object(guard.os, 'pread') as read:
                with self.assertRaises((guard.Refused, OSError)):
                    guard.Guard.run_child(obj, ['/fixed'], 0, 'pure')
                metadata.assert_not_called()
                read.assert_not_called()
            self.core.spawn.assert_called_once()
            awaited.assert_called_once()
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

    def test_raw_disallowed_signal_malformed_timeout_and_unknown_never_reap(self):
        def quarantine(child):
            self.core.UNCERTAIN = True
            raise guard.Refused()
        self.core.quarantine = quarantine
        for first in (SimpleNamespace(si_pid=123, si_code=os.CLD_EXITED, si_status=1),
                      SimpleNamespace(si_pid=123, si_code=os.CLD_KILLED, si_status=9),
                      SimpleNamespace(si_pid=123, si_code=os.CLD_DUMPED, si_status=11),
                      SimpleNamespace(si_pid=124, si_code=os.CLD_EXITED, si_status=0),
                      SimpleNamespace(si_pid=123, si_code=os.CLD_EXITED, si_status=False),
                      OSError('ECHILD'), KeyboardInterrupt()):
            self.core.UNCERTAIN = False
            child = SimpleNamespace(pid=123, returncode=None)
            with patch.object(guard.os, 'waitid', side_effect=first if isinstance(first, BaseException) else None,
                              return_value=first) as observe, patch.object(guard.os, 'waitpid') as reap:
                with self.assertRaises(guard.Refused):
                    guard.await_allowed(child, 1, (0,))
                with self.assertRaises(guard.Refused):
                    guard.await_allowed(child, 1, (0,))
                observe.assert_called_once()
                reap.assert_not_called()
                self.assertIsNone(child.returncode)
        self.core.UNCERTAIN = False
        with patch.object(guard.os, 'waitid') as observe, patch.object(guard.os, 'waitpid') as reap:
            with self.assertRaises(guard.Refused):
                guard.await_allowed(SimpleNamespace(pid=123, returncode=None), 0, (0,))
            observe.assert_not_called()
            reap.assert_not_called()

    def test_raw_only_expected_zero_or_readonly_absence_gets_matching_reap(self):
        for code, allowed in ((0, (0,)), (2, (2,)), (1, (0, 1))):
            child = SimpleNamespace(pid=123, returncode=None)
            seen = SimpleNamespace(si_pid=123, si_code=os.CLD_EXITED, si_status=code)
            with patch.object(guard.os, 'waitid', return_value=seen) as observe, \
                 patch.object(guard.os, 'waitpid', return_value=(123, code << 8)) as reap:
                self.assertEqual(guard.await_allowed(child, 1, allowed), code)
                self.assertEqual(child.returncode, code)
                observe.assert_called_once_with(os.P_PID, 123, os.WEXITED | os.WNOHANG | os.WNOWAIT)
                reap.assert_called_once_with(123, os.WNOHANG)

    def test_fixed_fd_reservation_never_overwrites_foreign_slot(self):
        meta = SimpleNamespace(st_dev=1, st_ino=2, st_mode=0o100500, st_uid=0,
                               st_gid=0, st_nlink=1, st_size=4, st_mtime_ns=1, st_ctime_ns=1)
        pins = {name: SimpleNamespace(fd=10 + index, before=meta, recheck=Mock())
                for index, name in enumerate(('helper', 'omavless'))}
        with patch.object(guard.os, 'fstat', return_value=meta), patch.object(guard.os, 'dup2') as duplicate:
            with self.assertRaises(guard.Refused):
                guard.reserve_elf_slots(pins)
            duplicate.assert_not_called()
        self.core.UNCERTAIN = False
        with patch.object(guard, 'RETAINED', []), \
             patch.object(guard.os, 'fstat', side_effect=[OSError(errno.EBADF, 'closed'), meta,
                                                        OSError(errno.EBADF, 'closed'), meta]), \
             patch.object(guard.os, 'dup2') as duplicate:
            guard.reserve_elf_slots(pins)
            self.assertEqual([call.args for call in duplicate.call_args_list], [(10, 198), (11, 199)])
            self.assertTrue(all(call.kwargs == {'inheritable': False} for call in duplicate.call_args_list))

    def test_real_publication_closes_writer_before_harmless_original_fd_exec(self):
        # No TemporaryDirectory/finally cleanup: an unknown child leaves this
        # bounded synthetic directory and descriptors retained, terminating the
        # test runner rather than allowing subsequent observations or retries.
        directory = Path(tempfile.mkdtemp(prefix='ov-publish-', dir=Path.home()))
        directory.chmod(0o700)
        path = directory / 'helper'
        raw = Path('/usr/bin/true').read_bytes()
        self.assertTrue(raw.startswith(b'\x7fELF'))
        writer = os.open(path, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC, 0o600)
        self.assertEqual(os.write(writer, raw), len(raw))
        os.fchmod(writer, 0o500)
        os.fsync(writer)
        original = os.fstat(writer)
        held = [writer]
        readonly_before = os.open(path, os.O_RDONLY | os.O_CLOEXEC)

        def execute(fd, expect_busy):
            receive, send = os.pipe2(os.O_CLOEXEC)
            pid = os.fork()
            if pid == 0:
                os.close(receive)
                try:
                    os.execve(f'/proc/self/fd/{fd}', ['fixed-harmless-true'], {'PATH': '/usr/bin'})
                except OSError as error:
                    if expect_busy and error.errno == errno.ETXTBSY:
                        os.write(send, b'EXACT_ETXTBSY')
                        os._exit(0)
                    os._exit(1)
                except BaseException:
                    os._exit(1)
            os.close(send)
            child = SimpleNamespace(pid=pid, returncode=None)
            try:
                guard.await_allowed(child, 5, (0,))
            except BaseException:
                raise KeyboardInterrupt('publication_child_unknown_preserve_no_cleanup') from None
            marker = os.read(receive, 64)
            os.close(receive)
            self.assertEqual(marker, b'EXACT_ETXTBSY' if expect_busy else b'')

        with patch.object(guard, 'core', owned_core), patch.object(owned_core, 'UNCERTAIN', False), \
             patch.object(owned_core, 'RETAINED', []), patch.object(guard, 'RETAINED', held), \
             patch.object(guard, 'ARTIFACTS', directory), patch.object(guard, 'UID', os.getuid()):
            execute(readonly_before, True)  # chmod0500 is not sufficient.
            source = SimpleNamespace(sha=hashlib.sha256(raw).hexdigest(), recheck=Mock())
            readonly = guard.complete_publication(writer, original, 'helper', source)
            self.assertNotIn(writer, held)
            with self.assertRaises(OSError) as closed:
                os.fstat(writer)
            self.assertEqual(closed.exception.errno, errno.EBADF)
            self.assertEqual(guard.identity(readonly.before), guard.identity(original))
            source.recheck.assert_called_once()
            execute(readonly.fd, False)
        # Both exact own children have observed/reaped exit0. Only now clean
        # this one known synthetic artifact; no general or recursive cleanup.
        os.close(readonly_before)
        for fd in held:
            os.close(fd)
        path.unlink()
        directory.rmdir()

    def test_publication_source_or_original_identity_failure_never_closes_writer(self):
        original = SimpleNamespace(st_dev=1, st_ino=2, st_mode=0o100500, st_uid=guard.UID,
                                   st_gid=guard.UID, st_nlink=1, st_size=4, st_mtime_ns=1, st_ctime_ns=1)
        for fault in ('identity', 'source'):
            before = SimpleNamespace(**vars(original))
            if fault == 'identity':
                before.st_ino += 1
            readonly = SimpleNamespace(before=before, recheck=Mock())
            source = SimpleNamespace(sha='a' * 64, recheck=Mock(side_effect=OSError('source unknown')
                                                                  if fault == 'source' else None))
            self.core.UNCERTAIN = False
            with patch.object(guard, 'RETAINED', [55]) as held, \
                 patch.object(guard, 'File', return_value=readonly), \
                 patch.object(guard.os, 'fstat', return_value=original), \
                 patch.object(guard.os, 'close') as close:
                with self.assertRaises((OSError, guard.Refused)):
                    guard.complete_publication(55, original, 'helper', source)
                close.assert_not_called()
                self.assertEqual(held, [55])


class LoaderTests(unittest.TestCase):
    def test_fixed_v2_cache_source_never_uses_user_runtime_tmpfs(self):
        self.assertEqual(loader.SOURCE, Path('/home/kdk_vm/.cache/t4-first-abort-cli-delivery-v3'))
        self.assertEqual(loader.DESTINATION, guard.ROOT)
        self.assertEqual(guard.ROOT, Path('/run/ov-t4-cli-guard-v3'))
        self.assertEqual(guard.HOME, Path('/home/ov-t4-abort-v2'))
        self.assertEqual(guard.RUNTIME, Path('/run/user/48045'))

    def test_loader_capacity_separate_and_shared_devices(self):
        for devices, amounts, accepted in (
            ((1,2), (100 + loader.HEADROOM, 200 + loader.HEADROOM), True),
            ((1,2), (99 + loader.HEADROOM, 200 + loader.HEADROOM), False),
            ((1,2), (100 + loader.HEADROOM, 199 + loader.HEADROOM), False),
            ((1,1), (300 + 2 * loader.HEADROOM,) * 2, True),
            ((1,1), (299 + 2 * loader.HEADROOM,) * 2, False),
        ):
            with patch.object(loader.os, 'fstat', side_effect=[SimpleNamespace(st_dev=d) for d in devices]), \
                 patch.object(loader.os, 'fstatvfs', side_effect=[SimpleNamespace(f_bavail=n, f_frsize=1) for n in amounts]):
                if accepted: loader.remaining_capacity(10, 11, 100, 200)
                else:
                    with self.assertRaises(RuntimeError): loader.remaining_capacity(10, 11, 100, 200)
        with patch.object(loader.os, 'fstat') as observed:
            with self.assertRaises(RuntimeError): loader.remaining_capacity(10, 11, True, 200)
            observed.assert_not_called()

    def test_real_files_all_admitted_before_publication_mode_hash_and_inode(self):
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            root.chmod(0o700)
            data = {name: b'pass\n' for name in loader.CODE}
            data.update({name: b'\x7fELFsynthetic' for name in loader.ELFS})
            value = {'schema': 't4-disposable-cli-delivery-v3',
                     'native_head': guard.NATIVE_HEAD, 'guard_head': 'a' * 40,
                     'code': {name: hashlib.sha256(data[name]).hexdigest() for name in loader.CODE},
                     'elfs': {name: {'sha256': hashlib.sha256(data[name]).hexdigest(), 'size': len(data[name]),
                         'host_original': [1, 2, 0o100755, 1000, 1000, 1, len(data[name]), 1, 1],
                         'host_frozen': [1, 3, 0o100500, 1000, 1000, 1, len(data[name]), 1, 1]}
                         for name in loader.ELFS}}
            for name, row in value['elfs'].items():
                row['host_alias'] = None
                if name == 'omavless':
                    row['host_original'][5] = 2
                    row['host_alias'] = {'relative_path': 'debug/deps/omavless-3eaa741bede04cf2',
                                         'identity': list(row['host_original']), 'sha256': row['sha256']}
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
            for fault in ('mode', 'hash', 'same-byte-inode', 'capacity'):
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
                         patch.object(loader, 'remaining_capacity', side_effect=RuntimeError('space refused')
                                      if fault == 'capacity' else None), \
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
