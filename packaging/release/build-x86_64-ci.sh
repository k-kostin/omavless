#!/bin/bash
# SPDX-License-Identifier: MIT
# Build-only CI: no installed host, private fixture, release token or publishing.
set -euo pipefail
[[ $EUID -ne 0 && $(uname -m) == x86_64 ]] || exit 2
runtime_source=b7fd0a99b8b169f0933e5f43ea4389642015193a
checkout=$(git rev-parse --show-toplevel)
build_root=$(mktemp -d /home/packagebuilder/omavless-build.XXXXXX)
artifacts=/home/packagebuilder/omavless-artifacts
[[ ! -e $artifacts ]] || exit 2
mkdir -m 700 "$artifacts"
git worktree add --detach "$build_root/source" "$runtime_source"
cd "$build_root/source"
[[ $(git rev-parse HEAD) == "$runtime_source" ]] || exit 2
[[ -z $(git status --porcelain) ]] || exit 2
export CARGO_TARGET_DIR="$build_root/target"
rustup toolchain install 1.98.0 --profile minimal --component clippy,rustfmt
{
  printf 'runtimeSourceCommit=%s\n' "$runtime_source"
  printf 'builderSourceCommit=%s\n' "$(git -C "$checkout" rev-parse HEAD)"
  uname -m
  rustc -Vv
  cargo -V
  pacman -Q glibc gcc binutils rustup
} > "$artifacts/build-provenance.txt"
cargo build --release --locked -p omavless-runtime --bin omavless 2>&1 | tee "$artifacts/build.log"
"$CARGO_TARGET_DIR/release/omavless" --help > "$artifacts/cli-help.txt"
readelf -h "$CARGO_TARGET_DIR/release/omavless" > "$artifacts/elf-header.txt"
mkdir -m 700 "$build_root/assembled"
python3 packaging/release/build-candidate.py "$build_root/assembled" \
  "$CARGO_TARGET_DIR/release/omavless" "$runtime_source" --stable
package=omavless-0.8.0-1-x86_64.pkg.tar.zst
# Inspector runs on the native architecture; no installation or activation.
python3 - "$build_root/assembled/$package" <<'PY'
import importlib.util
import pathlib
import sys
spec = importlib.util.spec_from_file_location('inspection', 'tests/installed_native_package.py')
inspection = importlib.util.module_from_spec(spec)
spec.loader.exec_module(inspection)
info = inspection.inspect_archive(pathlib.Path(sys.argv[1]))
assert info['source'] == 'b7fd0a99b8b169f0933e5f43ea4389642015193a'
assert info['version'] == '0.8.0-1'
print('Native x86_64 package inspection PASS; installed host acceptance NOT RUN')
PY
cp --reflink=never --sparse=never "$build_root/assembled/$package" "$artifacts/$package"
cp --reflink=never --sparse=never "$build_root/assembled/release-candidate.json" "$artifacts/runtime-build.json"
bsdtar -tf "$artifacts/$package" > "$artifacts/package-files.txt"
bsdtar -xOf "$artifacts/$package" .PKGINFO > "$artifacts/package-info.txt"
cd "$artifacts"
sha256sum "$package" build-provenance.txt build.log cli-help.txt elf-header.txt \
  runtime-build.json package-files.txt package-info.txt > SHA256SUMS
sha256sum --check SHA256SUMS

