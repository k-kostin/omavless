#!/usr/bin/env bash
# Owned, effect-free client fixture. Preserve the selected ELF on every outcome.
set -euo pipefail
[[ $# == 1 && -f $1 && ! -L $1 ]] || exit 64
repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd -- "$repo_dir"
native_wire_parent="$(python3 -c 'import runpy; print(runpy.run_path("tests/test-tui-native-import-wire.py")["private_parent"]())')"
native_wire_dir="$(mktemp -d "$native_wire_parent/nw.XXXXXX")"
install -m 500 "$1" "$native_wire_dir/omavless"
native_wire_sha="$(sha256sum "$native_wire_dir/omavless")"
printf 'Retained hermetic client ELF: %s\n' "$native_wire_dir/omavless"
python3 tests/test-tui-native-import-wire.py "$native_wire_dir/omavless" --sha256 "${native_wire_sha%% *}"
