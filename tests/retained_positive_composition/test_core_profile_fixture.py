"""Pure fixed-config/data-mapping controls; no engine, namespace or socket."""
import hashlib
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


p = module('profile_case', 'positive.py')
i = module('profile_images', 'images.py')
PROFILE = b'profile:\n  store-selected: false\n  store-fake-ip: false\n'
PREDECESSOR_BYTES = 529
PREDECESSOR_SHA = 'acc9e60bca550b5b3ce593bc8290f01a09c7177f48e931e5c922b2ce2e0ec8da'


class Controls(unittest.TestCase):
    def test_only_fixture_profile_block_changes_frozen_config(self):
        self.assertEqual(p.CORE_CONFIG.count(PROFILE), 1)
        self.assertIn(PROFILE + b'proxies: []\nproxy-groups: []\n', p.CORE_CONFIG)
        original = p.CORE_CONFIG.replace(PROFILE, b'')
        self.assertEqual(len(original), PREDECESSOR_BYTES)
        self.assertEqual(hashlib.sha256(original).hexdigest(), PREDECESSOR_SHA)
        self.assertEqual(p.CORE_CONFIG.count(b'profile:'), 1)
        self.assertEqual(p.CORE_CONFIG.count(b'store-selected:'), 1)
        self.assertEqual(p.CORE_CONFIG.count(b'store-fake-ip:'), 1)
        self.assertNotIn(b'proxy-providers:', p.CORE_CONFIG)
        self.assertNotIn(b'rule-providers:', p.CORE_CONFIG)

    def test_genuine_readonly_cache_data_mapping_remains_refused_without_io(self):
        # Valid increasing file-backed rows. PROT_READ data is not a code image;
        # the mutable fixed data pathname is still outside PUBLIC, never opened.
        raw = ('1000-2000 r-xp 0 00:01 1 /artifacts/mihomo\n'
               '2000-3000 r--s 0 00:01 2 /home/core/cache.db\n')
        rejected = []
        with patch.object(i.os, 'open') as opened, patch.object(i.os, 'read') as read:
            with self.assertRaises(i.Refused):
                i.map_objects(raw, before_reject=rejected.append)
            opened.assert_not_called()
            read.assert_not_called()
        self.assertEqual(rejected, ['named_path'])

    def test_unknown_deleted_and_path_aliases_are_not_cache_exemptions(self):
        for path in ('/home/core/cache.db (deleted)', '/home/core/./cache.db',
                     '/home/core/other.db', '/artifacts/cache.db', '/private/value'):
            with self.subTest(path=path):
                rejected = []
                with self.assertRaises(i.Refused):
                    i.map_objects('1000-2000 r--s 0 00:01 1 ' + path + '\n',
                                  before_reject=rejected.append)
                self.assertEqual(rejected, ['named_path'])
        self.assertEqual(i.map_objects('1000-2000 r-xp 0 00:01 1 /artifacts/mihomo\n'),
                         {'/artifacts/mihomo': (i.os.makedev(0, 1), 1)})


if __name__ == '__main__':
    unittest.main()
