# SPDX-License-Identifier: MIT
"""Offline fixture boundaries; no compiler download, build or namespace access."""
import hashlib
from pathlib import Path
import re
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "tests/namespace_api_compile_review"


class NamespaceCompileFixtureTests(unittest.TestCase):
    def test_consumer_is_not_a_workspace_dependency_or_package(self):
        self.assertFalse((FIXTURE / "Cargo.toml").exists())
        data = tomllib.loads((FIXTURE / "Cargo.toml.fixture").read_text())
        self.assertIs(data["package"]["publish"], False)
        self.assertEqual(data["package"]["rust-version"], "1.69")
        self.assertEqual(data["dependencies"]["nix"], {
            "path": "../nix", "default-features": False, "features": ["ioctl", "socket"]})
        workspace = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]
        self.assertNotIn("tests/namespace_api_compile_review", workspace["members"])

    def test_exact_review_lock_contains_only_path_fixture_and_checksummed_registry_packages(self):
        raw = (FIXTURE / "Cargo.lock.fixture").read_bytes()
        self.assertEqual(hashlib.sha256(raw).hexdigest(),
                         "7f680d59bfc7534c1705d368e365e5b73213d9bf98ab7e0e930d3bb7c182fa14")
        data = tomllib.loads(raw.decode("utf-8"))
        self.assertEqual(data["version"], 3)
        for package in data["package"]:
            if package["name"] in ("nix", "namespace-api-compile-review"):
                self.assertNotIn("source", package)
            else:
                self.assertEqual(package["source"], "registry+https://github.com/rust-lang/crates.io-index")
                self.assertTrue(re.fullmatch("[0-9a-f]{64}", package["checksum"]))

    def test_consumer_only_compile_checks_fixed_borrowed_api_and_linux_constants(self):
        source = (FIXTURE / "lib.rs.fixture").read_text()
        self.assertIn("#![forbid(unsafe_code)]", source)
        self.assertNotIn("unsafe {", source)
        self.assertNotIn("fn main(", source)
        self.assertNotIn("Command::", source)
        for item in ("namespace_type", "namespace_id", "socket_namespace_cookie"):
            self.assertIn("pub fn " + item + "(fd: BorrowedFd<'_>)", source)
        for constant in ("0x8008_b70d", "0xb703", "SO_NETNS_COOKIE == 71"):
            self.assertIn(constant, source)
