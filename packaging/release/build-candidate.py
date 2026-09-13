#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Developer-only offline assembly; Python is NOT shipped to either artifact.

The prebuilt binary is inspected by the existing Arch packager, never executed.
Its source provenance remains a caller responsibility, not inferred from ELF.
No download, publication, package installation, service or private-store access.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import posixpath
import re
import subprocess
import tarfile
import tomllib

ROOT = Path(__file__).resolve().parents[2]
FILES = (
    "backend.sh", "manifest.json", "preview.png", "LICENSE",
    "THIRD_PARTY_NOTICES.md", "CHANGELOG.md", "install.sh",
    "plugin/", "templates/", "docs/user/",
    "packaging/release/install-frontend.sh", "packaging/release/FRONTEND_README.md",
)


def git(root, *args):
    return subprocess.check_output(["git", "-C", str(root), *args], timeout=30)


def checked_source(root, expected):
    if not re.fullmatch(r"[0-9a-f]{40}", expected):
        raise ValueError("source identity")
    if git(root, "rev-parse", "HEAD").decode().strip() != expected:
        raise ValueError("source changed")
    if git(root, "status", "--porcelain", "--untracked-files=normal"):
        raise ValueError("uncommitted source")


def version(root):
    value = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+-rc\.[1-9][0-9]*", value):
        raise ValueError("candidate version required")
    return value


def frontend(root, destination, expected, release_version, epoch):
    """Archive only reviewed Git blobs, not private/untracked working files."""
    records = git(root, "ls-tree", "-rz", "--full-tree", expected, "--", *FILES).split(b"\0")
    entries = {}
    for record in filter(None, records):
        meta, raw_path = record.split(b"\t", 1)
        mode, kind, oid = meta.split()
        path = raw_path.decode("utf-8")
        if kind != b"blob" or mode not in (b"100644", b"100755"):
            raise ValueError("non-regular frontend member")
        if path.startswith("/") or ".." in path.split("/") or path.endswith(".py"):
            raise ValueError("unsafe frontend member")
        data = git(root, "cat-file", "blob", oid.decode())
        if path.startswith("docs/user/") and path.endswith(".md"):
            # Developer/acceptance docs are deliberately not shipped. Resolve
            # user-guide cross-links to this exact public source, never main.
            def link(match):
                target = match[1]
                if ":" in target or target.startswith(("#", "/")):
                    return match[0]
                resolved = posixpath.normpath(posixpath.join(posixpath.dirname(path), target))
                if resolved.startswith("../"):
                    raise ValueError("documentation link outside source")
                return "](https://github.com/k-kostin/omavless/blob/" + expected + "/" + resolved + ")"
            data = re.sub(r"\]\(([^\s)]+)\)", link, data.decode("utf-8")).encode()
        if path == "manifest.json":
            manifest = json.loads(data)
            manifest["version"] = release_version
            data = (json.dumps(manifest, ensure_ascii=False, indent=2) + "\n").encode()
        elif path == "install.sh":
            path = "install-frontend.sh"
        elif path == "packaging/release/install-frontend.sh":
            path = "install.sh"
        elif path == "packaging/release/FRONTEND_README.md":
            path = "README.md"
        if path in entries:
            raise ValueError("duplicate member")
        entries[path] = (data, 0o755 if path.endswith(".sh") else int(mode, 8) & 0o777)
    for required in ("README.md", "install.sh", "install-frontend.sh", "backend.sh", "manifest.json", "plugin/Panel.qml"):
        if required not in entries:
            raise ValueError("incomplete frontend")
    with tarfile.open(destination, "w:xz") as archive:
        for name, (data, mode) in sorted(entries.items()):
            member = tarfile.TarInfo("omavless-frontend/" + name)
            member.mode, member.mtime, member.size = mode, epoch, len(data)
            member.uid = member.gid = 0
            archive.addfile(member, io.BytesIO(data))


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def assemble(root, output, binary, expected):
    checked_source(root, expected)
    release_version = version(root)
    if not output.is_absolute() or output.resolve() != output or not output.is_dir():
        raise ValueError("unsafe output")
    if output == root or root in output.parents or output.stat().st_uid != os.getuid():
        raise ValueError("unsafe output")
    if any(output.iterdir()):
        raise ValueError("occupied output")
    output.chmod(0o700)
    build = output / "arch-build"
    build.mkdir(mode=0o700)
    subprocess.run(["/bin/bash", str(root / "packaging/arch/build-local-package.sh"),
                    str(build), str(binary), expected, "--candidate"],
                   check=True, timeout=180, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    packages = list(build.glob("omavless-*.pkg.tar.zst"))
    if len(packages) != 1:
        raise ValueError("missing package")
    package = output / packages[0].name
    # Same filesystem: keep the exact verified archive, not a second repack.
    packages[0].rename(package)
    epoch = int(git(root, "show", "-s", "--format=%ct", expected))
    archive = output / f"omavless-{release_version}-frontend.tar.xz"
    frontend(root, archive, expected, release_version, epoch)
    checked_source(root, expected)
    artifacts = {p.name: digest(p) for p in (package, archive)}
    identity = {"schemaVersion": 1, "version": release_version, "sourceCommit": expected,
                "architecture": os.uname().machine, "binarySha256": digest(build / "payload/usr/bin/omavless"),
                "provenance": "caller-supplied-prebuilt", "publication": "unpublished-candidate",
                "artifacts": artifacts}
    record = output / "release-candidate.json"
    record.write_text(json.dumps(identity, indent=2) + "\n")
    artifacts[record.name] = digest(record)
    (output / "SHA256SUMS").write_text("".join(f"{sha}  {name}\n" for name, sha in sorted(artifacts.items())))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("binary", type=Path)
    parser.add_argument("commit")
    args = parser.parse_args()
    try:
        assemble(ROOT, args.output, args.binary, args.commit)
    except (OSError, ValueError, KeyError, subprocess.SubprocessError):
        parser.exit(2, "Candidate assembly refused or failed; nothing was installed or published.\n")
    print("Native release candidate assembled; nothing was installed or published.")


if __name__ == "__main__":
    main()
