#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd -- "$repo_dir"

if ! command -v cc >/dev/null 2>&1; then
  echo 'Rust checks require a C linker; on Omarchy run: omarchy pkg add gcc' >&2
  exit 2
fi

cargo fmt --all -- --check
cargo test --workspace --locked
python3 tests/test-tui-terminal.py "${CARGO_TARGET_DIR:-target}/debug/examples/fixture_preview"
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo check --locked -p omavless-runtime --features tui
cargo test --locked -p omavless-runtime --features tui tui_commands_conform_to_canonical_mutation_parser
cargo clippy --locked -p omavless-runtime --features tui --all-targets -- -D warnings
cargo run --quiet --locked -p omavless-parity -- \
  compare tests/parity_cases/r0-reference.json tests/parity_cases/r0-candidate.json
