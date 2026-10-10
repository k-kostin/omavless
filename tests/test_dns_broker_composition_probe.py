#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Guard contracts only: no namespaces, processes, mounts or TUN effects."""
import unittest
from unittest.mock import patch

import dns_broker_composition_probe as probe


class CompositionGuardTests(unittest.TestCase):
    def test_missing_child_arguments_refuse_before_mount(self):
        with patch.object(probe.subprocess, 'run') as run:
            with self.assertRaises(RuntimeError):
                probe.isolated([])
            run.assert_not_called()

    def test_same_network_namespace_refuses_before_mount(self):
        with patch.object(probe.base.ns, 'namespace', return_value='net:[1]'), \
                patch.object(probe.subprocess, 'run') as run:
            with self.assertRaises(RuntimeError):
                probe.isolated(['net:[1]', 'user:[1]', 'pid:[1]', 'mnt:[1]',
                                '/tmp/fixed', '0' * 64, 'success'])
            run.assert_not_called()

    def test_same_mount_namespace_refuses_before_mount(self):
        with patch.object(probe.base, 'guard'), \
                patch.object(probe.base.ns, 'namespace', return_value='mnt:[1]'), \
                patch.object(probe.subprocess, 'run') as run:
            with self.assertRaises(RuntimeError):
                probe.isolated(['net:[1]', 'user:[1]', 'pid:[1]', 'mnt:[1]',
                                '/tmp/fixed', '0' * 64, 'success'])
            run.assert_not_called()

    def test_unknown_case_refuses_before_mount(self):
        with patch.object(probe.base, 'guard'), \
                patch.object(probe.base.ns, 'namespace', return_value='mnt:[2]'), \
                patch.object(probe.subprocess, 'run') as run:
            with self.assertRaises(RuntimeError):
                probe.isolated(['net:[1]', 'user:[1]', 'pid:[1]', 'mnt:[1]',
                                '/tmp/fixed', '0' * 64, 'arbitrary'])
            run.assert_not_called()

    def test_untrusted_binary_refuses_before_mount(self):
        with patch.object(probe.base, 'guard'), \
                patch.object(probe.base.ns, 'namespace', return_value='mnt:[2]'), \
                patch.object(probe.ownership, 'validate_source', side_effect=RuntimeError), \
                patch.object(probe.subprocess, 'run') as run:
            with self.assertRaises(RuntimeError):
                probe.isolated(['net:[1]', 'user:[1]', 'pid:[1]', 'mnt:[1]',
                                '/tmp/fixed', '0' * 64, 'success'])
            run.assert_not_called()


if __name__ == '__main__':
    unittest.main()
