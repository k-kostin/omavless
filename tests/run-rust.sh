#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd -- "$repo_dir"

if ! command -v cc >/dev/null 2>&1; then
  echo 'Rust checks require a C linker; on Omarchy run: omarchy pkg add gcc' >&2
  exit 2
fi

# Current virtual workspace members only. --all additionally traverses local
# dependency forks and rewrites immutable upstream source with our formatter.
cargo fmt -- --check
python3 -m unittest -v tests.test_k1_launch_acquisition_types
# This fixture forks a helper holding an inherited flock and tests a fixed
# cleanup budget. Running it beside unrelated process-heavy tests can spend
# that budget on scheduler contention, not the owned-group cleanup under test.
# Keep every assertion and run this one case separately, once, below.
cargo test --workspace --locked --exclude omavless-dns-broker --exclude omavless-dns-resolved -- \
  --skip core::tests::helper_resources_are_drained_even_after_leader_exit_or_term_spawn
cargo test --locked -p omavless-runtime --lib \
  core::tests::helper_resources_are_drained_even_after_leader_exit_or_term_spawn -- \
  --exact --test-threads=1
# These suites launch real private bus processes and assert short wire deadlines.
# Concurrent fork/exec can also briefly inherit another test's flock descriptor
# before CLOEXEC closes it, turning a malformed-journal test into an unrelated
# ownership refusal. Serialize these fixtures, not production operations; their
# explicit race/failure cases and all assertions remain enabled.
cargo test --locked -p omavless-dns-broker -p omavless-dns-resolved -- --test-threads=1
python3 tests/test-tui-terminal.py "${CARGO_TARGET_DIR:-target}/debug/examples/fixture_preview"
cargo clippy --workspace --all-targets --locked -- -D warnings
# Optional developer service is compiled/linted but never run/installed here.
cargo check --locked -p omavless-netguard --features netguard-service-core --all-targets
cargo clippy --locked -p omavless-netguard --features netguard-service-core --all-targets -- -D warnings
cargo test --locked -p omavless-netguard --features netguard-service-core service_core::tests:: -- --test-threads=1
cargo test --locked -p omavless-netguard --features netguard-service-core launch_acquisition::service_origin::tests:: -- --test-threads=1
cargo test --locked -p omavless-netguard --features netguard-service-core locked_state::tests::cold_restart_classifier -- --exact
cargo check --locked -p omavless-runtime --features tui
cargo test --locked -p omavless-runtime --features tui tui_commands_conform_to_canonical_mutation_parser
cargo clippy --locked -p omavless-runtime --features tui --all-targets -- -D warnings
cargo run --quiet --locked -p omavless-parity -- \
  compare tests/parity_cases/r0-reference.json tests/parity_cases/r0-candidate.json
