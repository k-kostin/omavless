# SPDX-License-Identifier: MIT
"""CPU-only developer publication guards; no core, broker or compiler launched."""
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    "artifact_support", Path(__file__).with_name("test_core_connections_adapter.py"))
support = importlib.util.module_from_spec(spec)
spec.loader.exec_module(support)
EXPORT, COMPOSITION = support.EXPORT, support.COMPOSITION


class ArtifactExportTests(unittest.TestCase):
    def root(self):
        # Actual secure ancestry: /tmp is deliberately not an admitted export
        # parent. This is owned test scratch below HOME, never installed state.
        # A fresh CI HOME need not contain .cache. Reserve only our temporary
        # directory under the existing HOME; do not create shared cache state.
        value = tempfile.TemporaryDirectory(prefix="ov-export-", dir=Path.home())
        self.addCleanup(value.cleanup)
        return Path(value.name)

    def fill(self, bundle):
        for name in sorted(EXPORT.FILES):
            bundle.put(name, b"synthetic artifact\n")

    def test_complete_manifest_is_last_and_explicitly_non_authorizing(self):
        root = self.root()
        with EXPORT.destination(root / "bundle") as bundle:
            self.fill(bundle)
            self.assertFalse((bundle.path / "developer-manifest.json").exists())
            bundle.finish({"synthetic_test_only": True, "effect_authority": True})
        manifest = json.loads((root / "bundle/developer-manifest.json").read_bytes())
        self.assertEqual(manifest["schema"], EXPORT.SCHEMA)
        self.assertEqual(set(manifest["sha256"]), EXPORT.FILES)
        for key in ("broker_executed", "installed_compatibility", "package_attestation", "effect_authority"):
            self.assertIs(manifest[key], False)
        self.assertEqual((root / "bundle").stat().st_mode & 0o777, 0o700)
        self.assertTrue(all(p.stat().st_mode & 0o777 == 0o600 for p in (root / "bundle").iterdir()))

    def test_partial_or_changed_inputs_never_publish_and_cleanup_only_owned_names(self):
        for failure in ("missing", "changed", "exception", "sync"):
            with self.subTest(failure=failure):
                root = self.root()
                sentinel = root / "unrelated"
                sentinel.write_bytes(b"retain")
                with self.assertRaises((RuntimeError, OSError)):
                    with EXPORT.destination(root / "bundle") as bundle:
                        if failure != "missing":
                            self.fill(bundle)
                        if failure == "changed":
                            (bundle.path / "mihomo").write_bytes(b"different")
                        if failure == "exception":
                            raise RuntimeError("synthetic build failure")
                        if failure == "sync":
                            with patch.object(EXPORT.os, "fsync", side_effect=OSError("sync refused")):
                                bundle.finish({})
                        else:
                            bundle.finish({})
                self.assertFalse((root / "bundle").exists())
                self.assertEqual(sentinel.read_bytes(), b"retain")

    def test_existing_outputs_and_symlinks_are_never_replaced(self):
        root = self.root()
        for kind in ("file", "directory", "symlink"):
            path = root / kind
            if kind == "file":
                path.write_bytes(b"retain")
            elif kind == "directory":
                path.mkdir()
            else:
                path.symlink_to(root / "absent")
            with self.assertRaises(FileExistsError):
                EXPORT.Bundle(path)
            self.assertTrue(path.exists() or path.is_symlink())

    def test_unsafe_parent_traversal_and_member_names_refuse(self):
        root = self.root()
        root.chmod(0o755)
        with self.assertRaises(RuntimeError):
            EXPORT.Bundle(root / "bundle")
        root.chmod(0o700)
        with self.assertRaises(RuntimeError):
            EXPORT.Bundle(root / ".." / "escape")
        with EXPORT.destination(root / "bundle") as bundle:
            for name in ("../escape", "unexpected", "/absolute"):
                with self.assertRaises(RuntimeError):
                    bundle.put(name, b"refuse")
        self.assertFalse((root / "bundle").exists())

    def test_destination_replacement_is_not_completed_or_removed_as_ours(self):
        root = self.root()
        with self.assertRaises(RuntimeError):
            with EXPORT.destination(root / "bundle") as bundle:
                bundle.put("mihomo", b"original")
                bundle.path.rename(root / "original")
                bundle.path.mkdir(mode=0o700)
                (bundle.path / "foreign").write_bytes(b"retain")
                bundle.finish({})
        self.assertEqual((root / "bundle/foreign").read_bytes(), b"retain")
        self.assertFalse((root / "original/mihomo").exists())
        self.assertFalse((root / "bundle/developer-manifest.json").exists())

    def test_input_alias_hardlink_and_mode_refuse(self):
        root = self.root()
        path = root / "source"
        path.write_bytes(b"fixed")
        alias = root / "alias"
        alias.symlink_to(path)
        with self.assertRaises(OSError):
            EXPORT.read_object(alias)
        alias.unlink()
        os.link(path, alias)
        with self.assertRaises(RuntimeError):
            EXPORT.read_object(path)
        alias.unlink()
        path.chmod(0o666)
        with self.assertRaises(RuntimeError):
            EXPORT.read_object(path)

    def test_source_archive_refuses_links_and_bounds(self):
        for kind in ("symlink", "size"):
            root = self.root()
            source = root / "source"
            source.mkdir()
            (source / "LICENSE").write_bytes(b"fixed source")
            if kind == "symlink":
                (source / "alias").symlink_to("LICENSE")
            with patch.object(EXPORT, "MAX_SOURCE", 1 if kind == "size" else EXPORT.MAX_SOURCE):
                with self.assertRaises(RuntimeError):
                    EXPORT.source_archive(root, [source])

    def test_same_byte_member_replacement_refuses_and_is_preserved(self):
        root = self.root()
        with self.assertRaisesRegex(RuntimeError, "preserved substituted"):
            with EXPORT.destination(root / "bundle") as bundle:
                self.fill(bundle)
                replacement = root / "replacement"
                replacement.write_bytes((bundle.path / "mihomo").read_bytes())
                replacement.chmod(0o600)
                replacement.replace(bundle.path / "mihomo")
                bundle.finish({})
        self.assertEqual((root / "bundle/mihomo").read_bytes(), b"synthetic artifact\n")
        self.assertFalse((root / "bundle/developer-manifest.json").exists())

    def test_archive_copy_refuses_fifo_symlink_and_midcopy_replacement(self):
        for kind in ("fifo", "symlink", "replacement"):
            root = self.root()
            source = root / "archive"
            if kind == "fifo":
                os.mkfifo(source, 0o600)
            elif kind == "symlink":
                source.symlink_to(root / "absent")
            else:
                source.write_bytes(b"synthetic archive")
                source.chmod(0o600)
            with self.assertRaises((RuntimeError, OSError)):
                with EXPORT.destination(root / "bundle") as bundle:
                    original = EXPORT.os.read
                    changed = False
                    def replace(fd, count):
                        nonlocal changed
                        data = original(fd, count)
                        if not changed:
                            changed = True
                            source.rename(root / "original")
                            os.mkfifo(source, 0o600)
                        return data
                    if kind == "replacement":
                        with patch.object(EXPORT.os, "read", side_effect=replace):
                            bundle.put_archive(source)
                    else:
                        bundle.put_archive(source)
            self.assertFalse((root / "bundle").exists())

    def test_export_requires_actual_wire_and_udp_before_any_build(self):
        root = self.root()
        with patch.object(COMPOSITION, "dns_patches") as patches:
            with self.assertRaisesRegex(RuntimeError, "requires wire and UDP"):
                COMPOSITION.exercise(root, root, root, root, developer_export=root / "bundle")
            patches.assert_not_called()
        self.assertFalse((root / "bundle").exists())


if __name__ == "__main__":
    unittest.main()
