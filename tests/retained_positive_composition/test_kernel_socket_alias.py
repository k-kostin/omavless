"""Approved HOST synthetic Unix-inode mechanism, not candidate/VM execution.

Only fresh temporary owned paths and unconnected Unix sockets are touched.
This proves the Linux primitive, not retained core/namespace effect authority.
"""
import os
from pathlib import Path
import socket
import stat
import tempfile
import unittest


@unittest.skipUnless(hasattr(os,'O_PATH') and Path('/proc/self/fd').is_dir(),'Linux O_PATH/proc required')
class KernelControls(unittest.TestCase):
    def test_original_o_path_socket_alias_chmod_without_new_path_resolution(self):
        with tempfile.TemporaryDirectory(prefix='sak.') as name:
            path=str(Path(name)/'s')
            with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as server:
                server.bind(path);os.chmod(path,0o666)
                fd=os.open(path,os.O_PATH|os.O_NOFOLLOW|os.O_CLOEXEC)
                directory=os.open('/proc/self/fd',os.O_PATH|os.O_DIRECTORY|os.O_CLOEXEC)
                try:
                    before=os.fstat(fd);self.assertTrue(stat.S_ISSOCK(before.st_mode))
                    self.assertEqual(stat.S_IMODE(before.st_mode),0o666)
                    os.chmod(str(fd),0o600,dir_fd=directory)
                    after=os.fstat(fd);named=os.stat(path,follow_symlinks=False)
                    self.assertEqual((before.st_dev,before.st_ino),(after.st_dev,after.st_ino))
                    self.assertEqual((after.st_dev,after.st_ino),(named.st_dev,named.st_ino))
                    self.assertEqual(stat.S_IMODE(after.st_mode),0o600)
                finally:os.close(fd);os.close(directory)  # Only this HOST synthetic test cleanup.

    def test_named_aba_does_not_redirect_original_alias_chmod(self):
        with tempfile.TemporaryDirectory(prefix='sak.') as name:
            path=Path(name)/'s';moved=Path(name)/'old'
            with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as first, \
                 socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as second:
                first.bind(str(path));os.chmod(path,0o666)
                fd=os.open(path,os.O_PATH|os.O_NOFOLLOW|os.O_CLOEXEC)
                directory=os.open('/proc/self/fd',os.O_PATH|os.O_DIRECTORY|os.O_CLOEXEC)
                try:
                    before=os.fstat(fd);os.rename(path,moved)
                    second.bind(str(path));os.chmod(path,0o666)
                    os.chmod(str(fd),0o600,dir_fd=directory)
                    after=os.fstat(fd);replacement=os.stat(path,follow_symlinks=False)
                    self.assertEqual((before.st_dev,before.st_ino),(after.st_dev,after.st_ino))
                    self.assertNotEqual((after.st_dev,after.st_ino),(replacement.st_dev,replacement.st_ino))
                    self.assertEqual(stat.S_IMODE(after.st_mode),0o600)
                    self.assertEqual(stat.S_IMODE(replacement.st_mode),0o666)
                finally:os.close(fd);os.close(directory)  # Owned synthetic resources only.


if __name__=='__main__':unittest.main()
