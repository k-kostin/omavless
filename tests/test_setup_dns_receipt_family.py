# SPDX-License-Identifier: MIT
"""Real jq against the two actual callback programs; synthetic JSON only.

No shell sourcing, extractor, package, systemd, native binary, VM or network
operation. The old release compatibility branch is deliberately retained.
"""
import copy
import json
from pathlib import Path
import re
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]
SOURCE = "a" * 40
PATCHES = {
    "mihomo-dns-broker.patch": "d5ebe9d6b37f6b76599fc3c2dd25adbfb774ca0121beeb79c5768a9a08d7ff37",
    "sing-tun-descriptor.patch": "2556c82aafbeb598a817d43042cf2069c6f209433c7a506395df581b4e31e2ab",
    "mihomo-traffic-ready.patch": "e9a2f1ea78046b6f2cfafe574bc2d989a8381d57d3464a0aff2d71507424beb1",
}
MEMBERS = ["mihomo", "omavless-dns-broker", "corresponding-source.tar.xz",
           "mihomo.LICENSE", "sing-tun.LICENSE", "omavless.LICENSE"]


def fixture(arch="x86_64", source=SOURCE):
    return {
        "schema": "omavless-managed-dns-traffic-ready-pair-v1", "architecture": arch,
        "omavless_commit": source, "mihomo_commit": "ab405bad5beeeac8b003bb01f60f134f6df54471",
        "mihomo_tag": "v1.19.31", "sing_tun_commit": "b50ae28a1409c7bce8e96e6c6966cf57d8ace754",
        "sing_tun_tag": "v0.4.24", "patch_sha256": PATCHES.copy(),
        "go_version": "go version go1.27.0 linux/" + ("amd64" if arch == "x86_64" else "arm64"),
        "go_binary_sha256": "b" * 64, "cargo_lock_sha256": "c" * 64,
        "go_build_tags": "with_gvisor", "go_dependency_mode": "vendor",
        "rustc_version": "rustc 1.98.1", "cargo_version": "cargo 1.98.1",
        "sha256": {name: str(index) * 64 for index, name in enumerate(MEMBERS, 1)},
        "package_flavor": "release", "broker_feature": "release-package", "traffic_ready_abi": 1,
    }


def callbacks():
    runtime = (ROOT / "plugin/setup-runtime.sh").read_text()
    predecessor = (ROOT / "plugin/setup-predecessor.sh").read_text()
    archive = re.findall(r"jq -e --arg source \"\$2\" --arg arch \"\$3\" \\\n    '(.*?)' \\\n", runtime, re.S)
    inventory = re.findall(r"/usr/bin/jq -er --arg arch \"\$arch\" '\n(.*?)' \"\$dns_receipt\"", predecessor, re.S)
    if len(archive) != 1 or len(inventory) != 1:
        raise AssertionError("actual receipt callback extraction changed")
    return archive[0], inventory[0]


class SetupDNSReceiptFamilyTests(unittest.TestCase):
    def setUp(self):
        self.programs = callbacks()

    def query(self, raw, arch="x86_64", source=SOURCE):
        if not isinstance(raw, str):
            raw = json.dumps(raw)
        results = []
        for program in self.programs:
            result = subprocess.run(
                ["/usr/bin/jq", "-er", "--arg", "source", source, "--arg", "arch", arch, program],
                input=raw, text=True, capture_output=True, timeout=5,
                env={"PATH": "/usr/bin:/bin", "LC_ALL": "C"}, check=False)
            results.append(result)
        return results

    def accepts(self, value, arch="x86_64", source=SOURCE):
        archive, inventory = self.query(value, arch, source)
        self.assertEqual(archive.returncode, 0, archive.stderr)
        self.assertEqual(archive.stdout, "true\n")
        self.assertEqual(inventory.returncode, 0, inventory.stderr)
        expected = [value["omavless_commit"], value["sha256"]["mihomo"],
                    value["sha256"]["omavless-dns-broker"]]
        self.assertEqual(inventory.stdout, "\t".join(expected) + "\n")

    def rejects(self, value, arch="x86_64"):
        for result in self.query(value, arch):
            self.assertNotEqual(result.returncode, 0)
            self.assertNotIn(result.stdout, ["true\n", SOURCE + "\t1\t2\n"])

    def test_exact_new_family_both_architectures(self):
        for arch in ["x86_64", "aarch64"]:
            with self.subTest(arch=arch):
                self.accepts(fixture(arch), arch)

    def test_old_normal_public_predecessor_admission_is_not_replaced(self):
        # Existing advisory fixtures intentionally use the historical minimal
        # receipt; new closed-family requirements must not be applied to them.
        for source in ["4915ae4a8f4ec6cfb469ad6b38126bd70d5a1998",
                       "e3d2245966c725ee11b505899b22a20721ff2ddd",
                       "8d02f9d7dd1a9063a82e9dcd93cb19cf4c5d8524",
                       "b65695ee0609c2a971289109975678b8da87c946",
                       "99c556e5c966e1e1dae04f2d8babd2ca2dec9375",
                       "d4d4310be6ade7e7e166f8b672cf90c7ad3e5b3b"]:
            for arch in ["x86_64", "aarch64"]:
                value = {"schema": 1, "architecture": arch, "package_flavor": "release",
                         "broker_feature": "release-package", "omavless_commit": source,
                         "sha256": {"mihomo": "1" * 64, "omavless-dns-broker": "2" * 64}}
                self.accepts(value, arch, source)

    def test_every_new_top_level_and_nested_field_is_required(self):
        for arch in ["x86_64", "aarch64"]:
            value = fixture(arch)
            for group in [None, "patch_sha256", "sha256"]:
                for key in (value if group is None else value[group]):
                    changed = copy.deepcopy(value)
                    del (changed if group is None else changed[group])[key]
                    with self.subTest(arch=arch, group=group, key=key):
                        self.rejects(changed, arch)

    def test_unknown_schema_and_abi_not_just_a_string_or_boolean(self):
        for key, bad in [("schema", "unknown"), ("schema", True),
                         ("schema", "omavless-managed-dns-close-pair-v1"),
                         ("schema", "omavless-managed-dns-k1-close-pair-v1"),
                         ("traffic_ready_abi", True), ("traffic_ready_abi", False),
                         ("traffic_ready_abi", "1"), ("traffic_ready_abi", None),
                         ("traffic_ready_abi", 0), ("traffic_ready_abi", 2),
                         ("traffic_ready_abi", 1.0)]:
            changed = fixture()
            changed[key] = bad
            with self.subTest(key=key, bad=bad):
                self.rejects(changed)

    def test_all_three_patch_names_hashes_and_no_fourth_patch(self):
        for key in PATCHES:
            changed = fixture()
            changed["patch_sha256"][key] = "f" * 64
            self.rejects(changed)
        for name in ["mihomo-conditional-close.patch", "mihomo-k1-device.patch", "unknown.patch"]:
            changed = fixture()
            changed["patch_sha256"][name] = "f" * 64
            self.rejects(changed)

    def test_extra_and_mixed_close_fields_never_qualify(self):
        for key, bad in [("conditional_close_abi", 1), ("go_cgo", False),
                         ("go_buildvcs", False), ("managed_device", "omavless0"),
                         ("enrollment_policy", "omavless0-ipv4-development-v1"),
                         ("production_adoption", True), ("unknown", None)]:
            changed = fixture()
            changed[key] = bad
            self.rejects(changed)
        changed = fixture()
        changed["sha256"]["unknown"] = "1" * 64
        self.rejects(changed)

    def test_wrong_upstream_build_configuration_and_go_target(self):
        for key, bad in [("mihomo_commit", "a" * 40), ("mihomo_tag", "v1.19.32"),
                         ("sing_tun_commit", "b" * 40), ("sing_tun_tag", "v0.4.25"),
                         ("package_flavor", "release-close"), ("broker_feature", "k1-managed-device"),
                         ("go_build_tags", "with_gvisor,omavless_k1_device"),
                         ("go_dependency_mode", "mod"),
                         ("go_version", "go version go1.27.0 linux/arm64"),
                         ("go_version", "go version go1.27.0 linux/amd64\n"),
                         ("rustc_version", "rustc 1.98.1\n"), ("cargo_version", "cargo 1.98.1\x7f")]:
            changed = fixture()
            changed[key] = bad
            with self.subTest(key=key, bad=bad):
                self.rejects(changed)
        self.rejects(fixture("riscv64"), "riscv64")

    def test_objects_and_strings_are_required(self):
        value = fixture()
        for key in value:
            for bad in [None, [], {}]:
                changed = copy.deepcopy(value)
                changed[key] = bad
                with self.subTest(key=key, bad=bad):
                    self.rejects(changed)
        self.rejects([])
        self.rejects(None)

    def test_new_digest_shape_and_no_zero_placeholders(self):
        for group, key, width in [(None, "omavless_commit", 40), (None, "go_binary_sha256", 64),
                                  (None, "cargo_lock_sha256", 64)] + [("sha256", key, 64) for key in MEMBERS]:
            for bad in ["0" * width, "A" * width, "a" * (width - 1), "a" * width + "\n"]:
                changed = fixture()
                (changed if group is None else changed[group])[key] = bad
                self.rejects(changed)

    def test_archive_source_gate_is_preserved_and_inventory_retains_mixed_diagnosis(self):
        value = fixture()
        archive, inventory = self.query(value, source="b" * 40)
        self.assertNotEqual(archive.returncode, 0)
        # This is metadata projection ONLY. The unchanged inventory code later
        # classifies source mismatch; it never turns this into update permission.
        self.assertEqual(inventory.returncode, 0, inventory.stderr)
        for result in self.query(value, arch="aarch64"):
            self.assertNotEqual(result.returncode, 0)

    def test_both_callbacks_share_identical_new_family_and_old_guards(self):
        archive, inventory = self.programs
        # The first semicolon occurs in jq all(generator; condition), so compare
        # the complete definition through the final rust/cargo metadata fence.
        definition = lambda program: program.split("def traffic_pair:", 1)[1].split(";\n    ", 1)[0]
        self.assertEqual(definition(archive), definition(inventory))
        self.assertIn("(.schema == 1 or traffic_pair)", archive)
        self.assertIn("(.schema == 1 or traffic_pair)", inventory)
        self.assertIn(".omavless_commit == $source", archive)
        for program in self.programs:
            self.assertIn('.package_flavor == "release"', program)
            self.assertIn('.broker_feature == "release-package"', program)
        self.assertEqual(len(PATCHES), 3)


if __name__ == "__main__":
    unittest.main()
