"""Synthetic local directories only; no real catalog, guest, tool or child."""
import os
import unittest
from unittest.mock import patch

from tests.six_library_static_closure import probe
from tests import test_six_library_static_closure as base_tests


class ExactPackageDirectoryTests(unittest.TestCase):
    def helper(self):
        return base_tests.CatalogTests('test_original_catalog_and_only_ten_selected_contents')

    def test_hyphenated_package_siblings_are_not_exact_package_duplicates(self):
        helper = self.helper()
        temporary, root, local = helper.synthetic()
        with temporary:
            siblings = ['openssl-1.1-1.1.1.w-1'] + [
                name+'-compat-2.0-1' for name in probe.PACKAGES]
            for sibling in siblings:
                (local/sibling).mkdir()
                (local/sibling/'desc').write_bytes(b'UNSELECTED_PRIVATE_SENTINEL')
            obj = probe.Sources({})
            a,b,c = helper.normalized(root)
            with a,b,c:
                try:
                    selected = obj.select_packages()
                    self.assertEqual(set(selected), set(probe.PACKAGES))
                    self.assertEqual(len(obj.package_files), 10)
                    self.assertFalse(obj.files)
                    for package, (directory, version) in selected.items():
                        self.assertEqual(directory, package+'-1:2.3~rc1-4')
                        self.assertEqual(version, '1:2.3~rc1-4')
                    for path in obj.package_files:
                        obj.take(path)
                    self.assertEqual(len(obj.files), 10)
                    self.assertFalse(any(sibling+'/' in path for sibling in siblings for path in obj.files))
                    obj.recheck()
                finally:
                    helper.close(obj)

    def test_same_original_directory_fd_scan_and_rescans_keep_complete_names(self):
        helper = self.helper()
        temporary, root, local = helper.synthetic()
        with temporary:
            (local/'ALPM_DB_VERSION').write_text('9')
            obj = probe.Sources({})
            original_scan = os.scandir
            a,b,c = helper.normalized(root)
            with a,b,c, patch.object(probe.os,'scandir',wraps=original_scan) as scan:
                try:
                    obj.select_packages()
                    fd, before, expected = obj.catalog
                    self.assertEqual(len(expected), 6)
                    for _ in range(3):
                        self.assertEqual(obj.names(fd), expected)
                        obj.recheck_catalog()
                    self.assertTrue(all(call.args == (fd,) for call in scan.call_args_list))
                    self.assertEqual(scan.call_count, 8)
                    self.assertEqual(probe.identity(os.fstat(fd)), probe.identity(before))
                    self.assertFalse(obj.files)
                    self.assertEqual(obj.state,'ready')
                finally:
                    helper.close(obj)

    def test_duplicate_exact_versions_still_refuse_and_permanently_seal(self):
        helper = self.helper()
        temporary, root, local = helper.synthetic()
        with temporary:
            (local/'openssl-3.6.0-1').mkdir()
            (local/'openssl-1.1-1.1.1.w-1').mkdir()
            obj = probe.Sources({})
            a,b,c = helper.normalized(root)
            with a,b,c:
                try:
                    with self.assertRaises(RuntimeError): obj.select_packages()
                    self.assertFalse(obj.files)
                    self.assertEqual(obj.state,'refused')
                    with patch.object(probe.os,'scandir') as scan, patch.object(probe.os,'open') as opened:
                        with self.assertRaises(RuntimeError): obj.select_packages()
                        scan.assert_not_called(); opened.assert_not_called()
                finally:
                    helper.close(obj)

    def test_missing_exact_package_and_noncanonical_exact_version_never_fall_back(self):
        for replacement in ('openssl-1.1-1.1.1.w-1', 'openssl-3.6.0-bad',
                            'openssl-3:4:5-1', 'openssl--1'):
            helper = self.helper()
            temporary, root, local = helper.synthetic()
            with temporary:
                (local/'openssl-1:2.3~rc1-4').rename(local/replacement)
                obj = probe.Sources({})
                a,b,c = helper.normalized(root)
                with a,b,c:
                    try:
                        with self.assertRaises(RuntimeError): obj.select_packages()
                        self.assertEqual(obj.state,'refused')
                        self.assertFalse(obj.files)
                    finally:
                        helper.close(obj)

    def test_exact_directory_version_uses_last_two_hyphens_without_numeric_guess(self):
        for version in ('1:2.3~rc1-4', '3.6.0-1.2', 'alpha_+.-2'):
            self.assertEqual(probe.directory_version('openssl-'+version,'openssl'), version)
        for name in ('openssl-1.1-1.1.1.w-1', 'openssl-compat-alpha-1',
                     'openssl-3.6.0-1-2', 'openssl--1', 'openssl-3.6.0-bad'):
            with self.assertRaises(RuntimeError): probe.directory_version(name,'openssl')

    def test_unselected_hyphenated_sibling_change_is_still_whole_catalog_failure(self):
        helper = self.helper()
        temporary, root, local = helper.synthetic()
        with temporary:
            sibling = local/'openssl-1.1-1.1.1.w-1'; sibling.mkdir()
            obj = probe.Sources({})
            a,b,c = helper.normalized(root)
            with a,b,c:
                try:
                    obj.select_packages()
                    sibling.rename(local/'openssl-1.1-1.1.1.w-2')
                    with self.assertRaises(RuntimeError): obj.recheck_catalog()
                    self.assertEqual(obj.state,'refused')
                    self.assertFalse(obj.files)
                    with patch.object(probe.os,'scandir') as scan:
                        with self.assertRaises(RuntimeError): obj.recheck_catalog()
                        scan.assert_not_called()
                finally:
                    helper.close(obj)


if __name__ == '__main__':
    unittest.main()
