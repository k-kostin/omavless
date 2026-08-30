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
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run --quiet --locked -p omavless-parity -- \
  compare tests/parity_cases/r0-reference.json tests/parity_cases/r0-candidate.json
