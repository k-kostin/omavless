#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd -- "$repo_dir"

if ! command -v cc >/dev/null 2>&1; then
  echo 'Rust checks require a C linker; on Omarchy run: omarchy pkg add gcc' >&2
  exit 2
fi

cargo fmt --all -- --check
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
cargo check --locked -p omavless-runtime --features tui
cargo test --locked -p omavless-runtime --features tui tui_commands_conform_to_canonical_mutation_parser
cargo clippy --locked -p omavless-runtime --features tui --all-targets -- -D warnings
# The opt-in client is not in default packages. These filters run only memory/
# parser/TestBackend controls; resource/terminal gates remain ignored.
rustfmt --edition 2024 --check \
  crates/omavless-runtime/src/native_coordinator/connection_close_client_integration.rs \
  crates/omavless-runtime/src/native_coordinator/connection_close_client_terminal.rs \
  crates/omavless-runtime/src/native_coordinator/connection_close_real_cli_foot.rs
cargo test --locked -p omavless-tui --features developer-conditional-close
cargo test --locked -p omavless-runtime --features developer-conditional-close --lib developer_connection_close::tests
cargo test --locked -p omavless-runtime --features developer-conditional-close --lib \
  native_coordinator::connection_close::tests::terminal_demo_is_closed_and_original_operation_cannot_be_replaced_or_resent -- --exact
cargo test --locked -p omavless-runtime --features developer-conditional-close --lib \
  native_coordinator::connection_close::tests::client_reply_diagnostic_discards_private_values_and_reports_only_closed_enums -- --exact
cargo test --locked -p omavless-runtime --features developer-conditional-close --lib real_ui_
cargo test --locked -p omavless-runtime --lib managed_close_receipt::tests
cargo test --locked -p omavless-runtime --features developer-conditional-close --lib release_pair::tests
cargo test --locked -p omavless-runtime --lib managed_pair::tests
# Backup-only is a distinct artifact configuration, not inferred from the
# product+T4 union below. These are ordinary source controls; VM gates stay ignored.
cargo test --locked -p omavless-tui --features private-backup
cargo test --locked -p omavless-tui --features developer-conditional-close,private-backup
cargo clippy --locked -p omavless-tui --all-targets --features developer-conditional-close,private-backup -- -D warnings
cargo check --locked -p omavless-runtime --no-default-features --features t4-manager-actor-service
cargo clippy --locked -p omavless-runtime --all-targets --features t4-manager-actor-service -- -D warnings
cargo test --locked -p omavless-runtime --lib --features t4-manager-actor-service normal_pair
cargo test --locked -p omavless-runtime --lib --features t4-manager-actor-service private_backup
# Internal 0.9.8 assembly: compilation selects both opt-ins, not host activation.
# Run ordinary synthetic/private-file tests only; all VM/resource gates stay ignored.
cargo check --locked -p omavless-runtime --all-targets --features product-image-witness,t4-manager-actor-service
cargo clippy --locked -p omavless-runtime --all-targets --features product-image-witness,t4-manager-actor-service -- -D warnings
cargo test --locked -p omavless-runtime --lib --features product-image-witness,t4-manager-actor-service -- \
  --skip core::tests::helper_resources_are_drained_even_after_leader_exit_or_term_spawn
cargo run --quiet --locked -p omavless-parity -- \
  compare tests/parity_cases/r0-reference.json tests/parity_cases/r0-candidate.json
