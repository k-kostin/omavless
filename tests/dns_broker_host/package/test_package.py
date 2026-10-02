"""No package build/install, root action, host systemd query or real DNS input."""
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("dns_package_stage", ROOT / "stage.py")
stage = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(stage)
sys.modules["stage"] = stage
BUILD_SPEC = importlib.util.spec_from_file_location("dns_pair_build", ROOT / "build_pair.py")
build_pair = importlib.util.module_from_spec(BUILD_SPEC)
BUILD_SPEC.loader.exec_module(build_pair)
RELEASE_SPEC = importlib.util.spec_from_file_location(
    "dns_release_stage", ROOT.parents[2] / "packaging/dns/stage.py")
release_stage = importlib.util.module_from_spec(RELEASE_SPEC)
RELEASE_SPEC.loader.exec_module(release_stage)
EMPTY = "LoadState=loaded\nActiveState=inactive\nSubState=dead\nMainPID=0\nNFileDescriptorStore=0\n"


class PackageTests(unittest.TestCase):
    def test_native_ci_emits_the_reviewed_package_extension_on_both_arches(self):
        script_path = ROOT / "build-ci.sh"
        script = script_path.read_text()
        self.assertEqual(subprocess.run(["bash", "-n", str(script_path)],
                                       capture_output=True, check=False).returncode, 0)
        self.assertIn("PKGEXT=.pkg.tar.zst PKGDEST=", script)
        self.assertIn('"$package_name"-*.pkg.tar.zst)', script)
        self.assertIn("command -v zstd >/dev/null", script)

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="omavless-package-test-")
        self.root = Path(self.temporary.name)
        self.addCleanup(self.temporary.cleanup)
        self.binary = self.root / "reviewed-elf"
        header = bytearray(64)
        header[:6] = b"\x7fELF\x02\x01"
        header[18:20] = (183).to_bytes(2, "little")
        self.binary.write_bytes(header)
        self.pin = hashlib.sha256(header).hexdigest()
        self.pair = self.root / "pair"
        self.pair.mkdir(mode=0o700)
        source = io.BytesIO()
        with tarfile.open(fileobj=source, mode="w:xz") as archive:
            for name, content in (
                ("mihomo/go.mod", b"synthetic Go module"),
                ("mihomo/vendor/modules.txt", b"synthetic vendored modules"),
                ("mihomo/LICENSE", b"synthetic GPL fixture"),
                ("sing-tun/go.mod", b"synthetic sing-tun module"),
                ("sing-tun/LICENSE", b"synthetic GPL fixture"),
                ("omavless/Cargo.lock", b"synthetic Cargo lock"),
                ("omavless/LICENSE", b"synthetic MIT fixture"),
            ):
                member = tarfile.TarInfo(name)
                member.size = len(content)
                archive.addfile(member, io.BytesIO(content))
        files = {
            "omavless-dns-broker": bytes(header), "mihomo": bytes(header),
            "corresponding-source.tar.xz": source.getvalue(),
            "mihomo.LICENSE": b"synthetic GPL fixture",
            "sing-tun.LICENSE": b"synthetic GPL fixture",
            "omavless.LICENSE": b"synthetic MIT fixture",
        }
        for name, content in files.items():
            (self.pair / name).write_bytes(content)
        receipt = {
            "schema": 1, "architecture": "aarch64", "omavless_commit": "a" * 40,
            "mihomo_commit": build_pair.MIHOMO, "sing_tun_commit": build_pair.SING_TUN,
            "patch_sha256": build_pair.PATCH_SHA,
            "mihomo_tag": "v1.19.31", "sing_tun_tag": "v0.4.24",
            "go_version": "go version go1.26.8 linux/arm64",
            "go_build_tags": "with_gvisor", "go_dependency_mode": "vendor",
            "go_binary_sha256": "b" * 64, "cargo_lock_sha256": "c" * 64,
            "rustc_version": "rustc 1.98.1", "cargo_version": "cargo 1.98.1",
            "sha256": {name: hashlib.sha256(content).hexdigest()
                       for name, content in files.items()},
        }
        (self.pair / "source-receipt.json").write_text(json.dumps(receipt))

    def render(self, output=None, **changes):
        args = {"pair": self.pair, "architecture": "aarch64", "revision": "a" * 40,
                "output": output or self.root / "staged"}
        args.update(changes)
        stage.stage(**args)
        return Path(args["output"])

    def test_stage_is_local_pinned_bounded_and_complete(self):
        destination = self.render()
        recipe = (destination / "PKGBUILD").read_text()
        manifest = json.loads((destination / "reviewed-inputs.json").read_text())
        self.assertEqual(manifest["architecture"], "aarch64")
        self.assertEqual(manifest["source_revision"], "a" * 40)
        for name, expected in manifest["sha256"].items():
            self.assertEqual(hashlib.sha256((destination / name).read_bytes()).hexdigest(), expected)
            self.assertIn(expected, recipe)
        self.assertNotIn("@", recipe)
        self.assertNotIn("SKIP", recipe)
        self.assertNotIn("://", recipe)
        self.assertEqual(destination.stat().st_mode & 0o777, 0o700)
        self.assertEqual(subprocess.run(["bash", "-n", str(destination / "PKGBUILD")],
                                       capture_output=True, check=False).returncode, 0)

    def test_release_staging_requires_release_broker_and_has_distinct_fixed_paths(self):
        with self.assertRaises(stage.Refused):
            release_stage.stage(self.pair, "aarch64", "a" * 40, self.root / "release")
        receipt_path = self.pair / "source-receipt.json"
        receipt = json.loads(receipt_path.read_text())
        receipt["package_flavor"] = "release"
        receipt["broker_feature"] = "release-package"
        receipt_path.write_text(json.dumps(receipt))
        with self.assertRaises(stage.Refused):
            stage.stage(self.pair, "aarch64", "a" * 40, self.root / "experimental")
        destination = self.root / "release"
        release_stage.stage(self.pair, "aarch64", "a" * 40, destination)
        recipe = (destination / "PKGBUILD").read_text()
        unit = (destination / "omavless-dns-broker.service").read_text()
        hook = (destination / "omavless-dns.hook").read_text()
        script = (destination / "omavless-dns.install").read_text()
        self.assertIn("pkgname=omavless-dns\n", recipe)
        self.assertIn("pkgver=0.9.7rc1\n", recipe)
        self.assertIn("conflicts=('omavless-dns-experimental')", recipe)
        self.assertIn("/usr/lib/omavless-dns/mihomo", recipe)
        self.assertIn("/usr/lib/omavless-dns/omavless-dns-broker", unit)
        self.assertIn("Target = omavless-dns", hook)
        self.assertIn("AbortOnFail", hook)
        self.assertIn("/usr/lib/omavless-dns/package-guard", script)
        self.assertIn("/usr/bin/setcap cap_net_bind_service,cap_net_admin,cap_net_raw=ep /usr/lib/omavless-dns/mihomo", script)
        self.assertNotIn("systemctl enable", script)
        self.assertNotIn("--enroll", script)
        self.assertNotIn("SKIP", recipe)
        self.assertNotIn("://", recipe)
        for filename in ("PKGBUILD", "omavless-dns.install"):
            self.assertEqual(subprocess.run(["bash", "-n", str(destination / filename)],
                                           capture_output=True, check=False).returncode, 0)
        manifest = json.loads((destination / "reviewed-inputs.json").read_text())
        self.assertEqual(manifest["package"], "omavless-dns")
        for name, expected in manifest["sha256"].items():
            self.assertEqual(hashlib.sha256((destination / name).read_bytes()).hexdigest(), expected)

    def test_release_package_version_requires_matching_frontend_and_runtime(self):
        for version, expected in (("0.9.0-rc.1", "0.9.0rc1"),
                                  ("0.9.0-rc.12", "0.9.0rc12"),
                                  ("0.9.5-beta.1", "0.9.5beta1"),
                                  ("0.9.5-beta.12", "0.9.5beta12"),
                                  ("0.9.0", "0.9.0")):
            cargo = f'[workspace.package]\nversion = "{version}"\n'
            manifest = json.dumps({"version": version})
            self.assertEqual(release_stage.package_version(cargo, manifest), expected)
        for cargo, manifest in (
            ('[workspace.package]\nversion = "0.9.0"\n', '{"version":"0.9.1"}'),
            ('[workspace.package]\nversion = "latest"\n', '{"version":"latest"}'),
            ('[workspace.package]\nversion = "0.9.0-rc.0"\n', '{"version":"0.9.0-rc.0"}'),
            ('[workspace.package]\nversion = "0.9.5-beta.0"\n', '{"version":"0.9.5-beta.0"}'),
            ('[workspace.package]\nversion = "0.9.5-beta.01"\n', '{"version":"0.9.5-beta.01"}'),
            ('[workspace.package]\nversion = "0.9.5-alpha.1"\n', '{"version":"0.9.5-alpha.1"}'),
            ('not toml', '{"version":"0.9.0"}'),
        ):
            with self.assertRaises(stage.Refused):
                release_stage.package_version(cargo, manifest)

    def test_offline_pair_builder_pins_committed_patches_and_local_tool(self):
        for name, expected in build_pair.PATCH_SHA.items():
            self.assertEqual(build_pair.digest(build_pair.PATCHES / name), expected)
        self.assertEqual(len(build_pair.git_value(build_pair.REPO, "rev-parse", "HEAD")), 40)
        self.assertEqual(build_pair.MIHOMO, "ab405bad5beeeac8b003bb01f60f134f6df54471")
        self.assertEqual(build_pair.SING_TUN, "b50ae28a1409c7bce8e96e6c6966cf57d8ace754")
        with self.assertRaises(stage.Refused):
            build_pair.reviewed_go("go", "x86_64")
        link = self.root / "linked-go"
        link.symlink_to("/usr/bin/go")
        with self.assertRaises(stage.Refused):
            build_pair.reviewed_go(str(link), "x86_64")
        with self.assertRaises(stage.Refused):
            build_pair.build(self.root, self.root, "/usr/bin/go", "x86_64",
                             build_pair.REPO / "candidate")

    def test_pair_builder_requires_matching_native_architecture_and_go(self):
        for architecture, go_arch in (("x86_64", "amd64"), ("aarch64", "arm64")):
            self.assertEqual(build_pair.reviewed_target(architecture, "Linux", architecture),
                             go_arch)
            with self.assertRaises(stage.Refused):
                build_pair.reviewed_target(architecture, "Linux", "other")
            with self.assertRaises(stage.Refused):
                build_pair.reviewed_target(architecture, "Darwin", architecture)
            with mock.patch.object(build_pair.subprocess, "run") as run:
                run.return_value.returncode = 0
                run.return_value.stdout = (
                    f"go version go1.26.8 linux/{go_arch}\n".encode("ascii"))
                self.assertIn(go_arch, build_pair.reviewed_go("/usr/bin/git", architecture))
                run.return_value.stdout = (
                    b"go version go1.26.8 linux/arm64\n" if go_arch == "amd64"
                    else b"go version go1.26.8 linux/amd64\n")
                with self.assertRaises(stage.Refused):
                    build_pair.reviewed_go("/usr/bin/git", architecture)
        with self.assertRaises(stage.Refused):
            build_pair.reviewed_target("unknown", "Linux", "unknown")

    def test_bad_hash_arch_revision_symlink_hardlink_existing_target_refuse(self):
        for changes in ({"architecture": "x86_64"}, {"revision": "main"}):
            with self.assertRaises(stage.Refused):
                self.render(**changes)
        link = self.root / "link"
        link.symlink_to(self.pair, target_is_directory=True)
        with self.assertRaises(stage.Refused):
            self.render(pair=link)
        (self.pair / "mihomo").unlink()
        os.link(self.binary, self.pair / "mihomo")
        with self.assertRaises(stage.Refused):
            self.render()
        (self.pair / "mihomo").unlink()
        (self.pair / "mihomo").write_bytes(self.binary.read_bytes())
        (self.pair / "mihomo").write_bytes(b"changed")
        with self.assertRaises(stage.Refused):
            self.render()
        (self.pair / "mihomo").write_bytes(self.binary.read_bytes())
        existing = self.root / "existing"
        existing.mkdir()
        with self.assertRaises(FileExistsError):
            self.render(output=existing)

    def test_pair_receipt_tamper_extra_fields_and_wrong_patches_refuse(self):
        path = self.pair / "source-receipt.json"
        original = path.read_text()
        try:
            for mutation in (
                lambda record: record.update({"private_profile": "synthetic"}),
                lambda record: record.update({"patch_sha256": {}}),
                lambda record: record["sha256"].update({"mihomo": "0" * 64}),
                lambda record: record.update({"go_version": "synthetic\nsecret"}),
            ):
                record = json.loads(original)
                mutation(record)
                path.write_text(json.dumps(record))
                with self.assertRaises(stage.Refused):
                    self.render()
        finally:
            path.write_text(original)

    def test_package_has_no_stock_core_override_activation_or_enrollment(self):
        recipe = (ROOT / "PKGBUILD.in").read_text()
        script = (ROOT / "omavless-dns-experimental.install").read_text()
        self.assertIn("/usr/lib/omavless-dns-experimental/mihomo", recipe)
        self.assertIn("corresponding-source.tar.xz", recipe)
        self.assertIn("noextract=('corresponding-source.tar.xz')", recipe)
        self.assertIn("mihomo.LICENSE", recipe)
        self.assertIn("sing-tun.LICENSE", recipe)
        self.assertIn("omavless.LICENSE", recipe)
        self.assertNotIn("$pkgdir/usr/bin/", recipe)
        self.assertNotIn("$pkgdir/etc/", recipe)
        self.assertIn("/usr/bin/setcap cap_net_bind_service,cap_net_admin,cap_net_raw=ep /usr/lib/omavless-dns-experimental/mihomo", script)
        self.assertNotRegex(script, r"(?m)^\s*(sudo|pkexec|systemctl|rm|curl|wget)\b")
        self.assertEqual(subprocess.run(["bash", "-n", str(ROOT / "omavless-dns-experimental.install")],
                                       capture_output=True, check=False).returncode, 0)

    def test_staging_refuses_normal_bare_linked_and_symlinked_git_destinations(self):
        for bare in (False, True):
            repository = self.root / ("bare" if bare else "repository")
            command = ["/usr/bin/git", "init", "--quiet"]
            if bare:
                command.append("--bare")
            subprocess.run([*command, str(repository)], check=True, capture_output=True,
                           env={"PATH": "/usr/bin:/bin", "GIT_CONFIG_NOSYSTEM": "1",
                                "GIT_CONFIG_GLOBAL": "/dev/null"})
            with self.assertRaises(stage.Refused):
                self.render(output=repository / "staged")
            self.assertFalse((repository / "staged").exists())
        linked = self.root / "linked"
        linked.mkdir()
        (linked / ".git").write_text(f"gitdir: {self.root / 'repository/.git'}\n")
        alias = self.root / "alias"
        alias.symlink_to(linked, target_is_directory=True)
        for directory in (linked, alias):
            with self.assertRaises(stage.Refused):
                self.render(output=directory / "staged")
            self.assertFalse((directory / "staged").exists())

    def test_staging_requires_absolute_destination_under_private_owned_parent(self):
        with self.assertRaises(stage.Refused):
            self.render(output=Path("relative-staged"))
        shared = self.root / "shared"
        shared.mkdir()
        shared.chmod(0o777)
        with self.assertRaises(stage.Refused):
            self.render(output=shared / "staged")
        self.assertFalse((shared / "staged").exists())

    def test_real_abort_is_pretransaction_hook_not_only_scriptlet(self):
        hook = (ROOT / "omavless-dns-experimental.hook").read_text()
        for line in ("Operation = Upgrade", "Operation = Remove", "Type = Package",
                     "Target = omavless-dns-experimental", "When = PreTransaction",
                     "Exec = /usr/lib/omavless-dns-experimental/package-guard", "AbortOnFail"):
            self.assertIn(line, hook.splitlines())

    def run_guard_fixture(self, state=EMPTY, entry=None, args=()):
        # Private source fixture substitutions only. Production has no injected
        # paths, command, UID or bus overrides. Never queries host systemd.
        runtime = self.root / "run"
        private = runtime / "omavless-dns/private"
        private.mkdir(parents=True, exist_ok=True)
        for directory in (runtime, private.parent, private):
            directory.chmod(0o700)
        if entry:
            (private / entry).write_text("synthetic")
        statefile = self.root / "properties"
        statefile.write_text(state)
        source = (ROOT / "package-guard").read_text()
        self.assertIn("[[ $# == 0 && $EUID == 0 ]]", source)
        source = source.replace("[[ $# == 0 && $EUID == 0 ]]", "[[ $# == 0 ]]")
        source = source.replace('"$owner" == 0', f'"$owner" == {os.getuid()}')
        source = source.replace("for directory in /run ", f"for directory in {runtime} ")
        source = source.replace("/run/omavless-dns", str(runtime / "omavless-dns"))
        source, replaced = re.subn(r"state=\$\(/usr/bin/env -i PATH=/usr/bin:/bin LC_ALL=C /usr/bin/timeout.*?--no-pager\)",
                                  f"state=$(/usr/bin/cat {statefile})", source, flags=re.S)
        self.assertEqual(replaced, 1)
        fixture = self.root / "guard-fixture"
        fixture.write_text(source)
        return subprocess.run(["bash", str(fixture), *args], capture_output=True,
                              timeout=5, check=False, env={"PATH": "/usr/bin:/bin"})

    def test_guard_accepts_only_proven_inactive_empty_fixture(self):
        self.assertEqual(self.run_guard_fixture().returncode, 0)
        self.assertNotEqual(self.run_guard_fixture(args=("--force",)).returncode, 0)
        for invalid in (EMPTY.replace("inactive", "active"), EMPTY.replace("inactive", "failed"),
                        EMPTY.replace("MainPID=0", "MainPID=12"),
                        EMPTY.replace("NFileDescriptorStore=0", "NFileDescriptorStore=1"),
                        EMPTY.replace("NFileDescriptorStore=0\n", ""),
                        EMPTY + "MainPID=0\n", EMPTY + "Unexpected=0\n", ""):
            result = self.run_guard_fixture(state=invalid)
            self.assertNotEqual(result.returncode, 0)
            self.assertLess(len(result.stderr), 200)
            self.assertIn(b"OmaVLESS DNS broker is not proven inactive", result.stderr)
            self.assertNotIn(b"Experimental DNS package", result.stderr)
            self.assertEqual(result.stdout, b"")

    def test_any_journal_staging_or_unknown_entry_blocks(self):
        for entry in ("lease.json", ".lease.pending", "unknown"):
            self.assertNotEqual(self.run_guard_fixture(entry=entry).returncode, 0)
            (self.root / "run/omavless-dns/private" / entry).unlink()

    def test_stale_socket_or_symlink_blocks_without_cleanup(self):
        self.assertEqual(self.run_guard_fixture().returncode, 0)
        path = self.root / "run/omavless-dns/control.sock"
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as listener:
            listener.bind(str(path))
        self.assertNotEqual(self.run_guard_fixture().returncode, 0)
        self.assertTrue(path.is_socket())
        path.unlink()
        path.symlink_to("missing")
        self.assertNotEqual(self.run_guard_fixture().returncode, 0)
        self.assertTrue(path.is_symlink())


class GitExportTests(unittest.TestCase):
    """Actual offline Git fixtures; no upstream/build/install/host input."""

    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="omavless-git-export-test-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.repository = self.root / "repository"
        self.git("init", "--quiet", "--template=", str(self.repository), repository=None)
        (self.repository / "keep.txt").write_text("original committed source\n")
        (self.repository / "format.txt").write_text("$Format:%H$\n")
        (self.repository / "ignored.txt").write_text("committed export exclusion\n")
        (self.repository / ".gitattributes").write_text("ignored.txt export-ignore\n")
        self.revision = self.commit()

    def git(self, *arguments, repository=True, replacements=False):
        env = build_pair.git_environment()
        if replacements:
            env.pop("GIT_NO_REPLACE_OBJECTS")
        prefix = [] if repository is None else ["-C", str(
            self.repository if repository is True else repository)]
        result = subprocess.run(["/usr/bin/git", "-c", "core.hooksPath=/dev/null",
                                 "-c", "core.fsmonitor=false", *prefix, *arguments],
                                env=env, stdin=subprocess.DEVNULL, capture_output=True,
                                timeout=10, check=False)
        self.assertEqual(result.returncode, 0, "synthetic Git fixture failed")
        return result.stdout

    def commit(self):
        self.git("add", ".")
        self.git("-c", "user.name=Synthetic", "-c", "user.email=synthetic@example.invalid",
                 "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "fixture")
        return self.git("rev-parse", "HEAD").decode().strip()

    def export(self, repository=None, destination="export"):
        destination = self.root / destination
        build_pair.export_git(repository or self.repository, self.revision, destination)
        return destination

    def test_source_info_attributes_poison_is_excluded_and_preserved(self):
        attributes = self.repository / ".git/info/attributes"
        poison = "keep.txt export-ignore\nformat.txt export-subst\n"
        attributes.parent.mkdir()
        attributes.write_text(poison)
        # Demonstrate that an exact SHA alone did not protect the old exporter.
        raw = self.git("archive", "--format=tar", self.revision)
        with tarfile.open(fileobj=io.BytesIO(raw), mode="r:") as archive:
            self.assertNotIn("keep.txt", archive.getnames())
            self.assertEqual(archive.extractfile("format.txt").read(),
                             (self.revision + "\n").encode())
        exported = self.export()
        self.assertEqual((exported / "keep.txt").read_text(), "original committed source\n")
        self.assertEqual((exported / "format.txt").read_text(), "$Format:%H$\n")
        self.assertFalse((exported / "ignored.txt").exists())
        self.assertEqual(attributes.read_text(), poison)
        self.assertFalse(list(self.root.glob(".source-export-*")))

    def test_source_local_attributes_configuration_is_excluded(self):
        attributes = self.root / "external-attributes"
        attributes.write_text("keep.txt export-ignore\n")
        self.git("config", "core.attributesFile", str(attributes))
        with tarfile.open(fileobj=io.BytesIO(self.git("archive", self.revision)), mode="r:") as archive:
            self.assertNotIn("keep.txt", archive.getnames())
        self.assertTrue((self.export() / "keep.txt").is_file())
        self.assertEqual(self.git("config", "--get", "core.attributesFile").decode().strip(),
                         str(attributes))

    def test_dirty_source_and_untracked_files_do_not_enter_export(self):
        (self.repository / "keep.txt").write_text("dirty working source\n")
        (self.repository / "untracked.txt").write_text("untracked fixture\n")
        index = self.repository / ".git/index"
        index_before = index.read_bytes()
        exported = self.export()
        self.assertEqual((exported / "keep.txt").read_text(), "original committed source\n")
        self.assertFalse((exported / "untracked.txt").exists())
        self.assertEqual((self.repository / "keep.txt").read_text(), "dirty working source\n")
        self.assertEqual((self.repository / "untracked.txt").read_text(), "untracked fixture\n")
        self.assertEqual(index.read_bytes(), index_before)

    def test_actual_replacement_commit_cannot_change_observation_or_export(self):
        (self.repository / "keep.txt").write_text("replacement source\n")
        replacement = self.commit()
        self.git("replace", self.revision, replacement)
        reference = self.repository / ".git/refs/replace" / self.revision
        before = reference.read_bytes()
        self.assertEqual(self.git("show", self.revision + ":keep.txt", replacements=True),
                         b"replacement source\n")
        self.assertEqual(build_pair.git_value(self.repository, "show", self.revision + ":keep.txt"),
                         "original committed source")
        self.assertEqual((self.export() / "keep.txt").read_text(), "original committed source\n")
        self.assertEqual(reference.read_bytes(), before)

    def test_linked_worktree_uses_common_object_store_not_its_dirty_files(self):
        linked = self.root / "linked"
        self.git("worktree", "add", "--quiet", "--detach", str(linked), self.revision)
        (linked / "keep.txt").write_text("dirty linked source\n")
        self.assertEqual((self.export(repository=linked) / "keep.txt").read_text(),
                         "original committed source\n")
        self.assertEqual((linked / "keep.txt").read_text(), "dirty linked source\n")

    def test_bare_source_object_store_is_supported(self):
        bare = self.root / "bare"
        self.git("clone", "--quiet", "--bare", "--no-hardlinks", str(self.repository), str(bare),
                 repository=None)
        self.assertEqual((self.export(repository=bare) / "keep.txt").read_text(),
                         "original committed source\n")

    def test_ambient_git_configuration_and_template_are_not_inherited(self):
        template = self.root / "poison-template"
        (template / "info").mkdir(parents=True)
        (template / "info/attributes").write_text("keep.txt export-ignore\n")
        global_config = self.root / "poison-global"
        global_config.write_text(f"[init]\n\ttemplateDir = {template}\n")
        with mock.patch.dict(os.environ, {"GIT_CONFIG_GLOBAL": str(global_config),
                                          "GIT_TEMPLATE_DIR": str(template),
                                          "GIT_DIR": str(self.root / "absent"),
                                          "GIT_CONFIG_COUNT": "1",
                                          "GIT_CONFIG_KEY_0": "core.attributesFile",
                                          "GIT_CONFIG_VALUE_0": str(template / "info/attributes")}):
            self.assertTrue((self.export() / "keep.txt").is_file())
        env = build_pair.git_environment()
        self.assertEqual(env["GIT_NO_REPLACE_OBJECTS"], "1")
        self.assertEqual(env["GIT_CONFIG_NOSYSTEM"], "1")
        self.assertEqual(env["GIT_CONFIG_GLOBAL"], "/dev/null")
        self.assertEqual(env["GIT_NO_LAZY_FETCH"], "1")
        self.assertEqual(env["GIT_OPTIONAL_LOCKS"], "0")

    def test_nonexact_or_absent_revision_refuses_without_destination(self):
        for revision in ("HEAD", self.revision.upper(), "0" * 40):
            with self.subTest(revision=revision), self.assertRaises(stage.Refused):
                build_pair.export_git(self.repository, revision, self.root / "refused")
            self.assertFalse((self.root / "refused").exists())
            self.assertFalse((self.root / "refused.tar").exists())

    def test_existing_archive_is_not_overwritten(self):
        archive = self.root / "export.tar"
        archive.write_bytes(b"existing private fixture")
        with self.assertRaises(FileExistsError):
            self.export()
        self.assertEqual(archive.read_bytes(), b"existing private fixture")
        self.assertFalse((self.root / "export").exists())

    def test_git_metadata_and_error_pipes_are_bounded_without_content_disclosure(self):
        secret = "synthetic private marker " * 1000
        (self.repository / "oversize.txt").write_text(secret)
        revision = self.commit()
        with self.assertRaisesRegex(stage.Refused, "exceeded its bound") as refusal:
            build_pair.git_value(self.repository, "show", revision + ":oversize.txt")
        self.assertNotIn("marker", str(refusal.exception))
        with mock.patch.object(build_pair, "MAX_GIT_ERROR", 8):
            with self.assertRaisesRegex(stage.Refused, "exceeded its bound") as refusal:
                build_pair.git_value(self.repository, "rev-parse", "--verify", "synthetic-private-name")
        self.assertNotIn("synthetic-private-name", str(refusal.exception))

    def test_archive_bound_is_enforced_before_oversized_write(self):
        with mock.patch.object(build_pair, "MAX_ARCHIVE", 16):
            with self.assertRaisesRegex(stage.Refused, "exceeded its bound"):
                self.export()
        self.assertLessEqual((self.root / "export.tar").stat().st_size, 16)
        self.assertFalse((self.root / "export").exists())
        self.assertFalse(list(self.root.glob(".source-export-*")))
        self.assertEqual((self.repository / "keep.txt").read_text(), "original committed source\n")

    def test_expired_deadline_reaps_only_the_owned_git_child(self):
        children = []
        original = subprocess.Popen

        def record_child(*arguments, **keywords):
            child = original(*arguments, **keywords)
            children.append(child)
            return child

        with mock.patch.object(build_pair.subprocess, "Popen", side_effect=record_child):
            with self.assertRaisesRegex(stage.Refused, "exceeded its bound"):
                build_pair.git_command(["-C", str(self.repository), "rev-parse", "HEAD"], timeout=0)
        self.assertEqual(len(children), 1)
        self.assertIsNotNone(children[0].returncode)
        self.assertTrue(children[0].stdout.closed)
        self.assertTrue(children[0].stderr.closed)


if __name__ == "__main__":
    unittest.main()
