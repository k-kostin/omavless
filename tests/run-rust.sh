#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd -- "$repo_dir"

if ! command -v cc >/dev/null 2>&1; then
  echo 'Rust checks require a C linker; on Omarchy run: omarchy pkg add gcc' >&2
  exit 2
fi

# Optimize only SHA-256 in test builds. Keep runtime code at its normal debug
# profile and retain assertions/overflow checks even in the optimized dependency.
# CLI test-profile overrides cannot affect cargo check/clippy/run or release builds.
sha2_test_profile=(
  --config profile.test.package.sha2.opt-level=3
  --config profile.test.package.sha2.debug-assertions=true
  --config profile.test.package.sha2.overflow-checks=true
)

cargo fmt --all -- --check
# This fixture forks a helper holding an inherited flock and tests a fixed
# cleanup budget. Running it beside unrelated process-heavy tests can spend
# that budget on scheduler contention, not the owned-group cleanup under test.
# Keep every assertion and run this one case separately, once, below.
# The exact-proof drain fixture likewise checks short owner/lease deadlines;
# its private controller/process work must not compete with unrelated forks.
# Keep semantic assertions and the short held-proof bound. Successful complete
# Disconnect also includes an independently budgeted real owned-child stop.
close_drain_tests=(
  'conditional_close_candidate::tests::cancel_and_drain_held_exact_lease_times_out_and_permanently_cancels'
  'conditional_close_candidate::tests::cancel_and_drain_released_exact_lease_acknowledges_only_after_unlock'
  'native_coordinator::connection_close::tests::actual_owner_disconnect_drains_exact_proof_lease_or_returns_bounded_busy'
  'native_coordinator::connection_close::tests::actual_owner_disconnect_slow_core_preserves_graceful_stop_and_cleanup'
  'native_coordinator::connection_close::tests::actual_owner_disconnect_paused_proof_unwind_releases_before_fixture_cleanup'
)
close_drain_skips=()
for close_drain_test in "${close_drain_tests[@]}"; do
  close_drain_skips+=(--skip "$close_drain_test")
done
cargo test "${sha2_test_profile[@]}" --workspace --locked --exclude omavless-dns-broker --exclude omavless-dns-resolved -- \
  --skip core::tests::helper_resources_are_drained_even_after_leader_exit_or_term_spawn \
  "${close_drain_skips[@]}"
cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --lib \
  core::tests::helper_resources_are_drained_even_after_leader_exit_or_term_spawn -- \
  --exact --test-threads=1
for close_drain_test in "${close_drain_tests[@]}"; do
  cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --lib "$close_drain_test" -- --exact --test-threads=1
done
# These suites launch real private bus processes and assert short wire deadlines.
# Concurrent fork/exec can also briefly inherit another test's flock descriptor
# before CLOEXEC closes it, turning a malformed-journal test into an unrelated
# ownership refusal. Serialize these fixtures, not production operations; their
# explicit race/failure cases and all assertions remain enabled.
cargo test "${sha2_test_profile[@]}" --locked -p omavless-dns-broker -p omavless-dns-resolved -- --test-threads=1
python3 tests/test-tui-terminal.py "${CARGO_TARGET_DIR:-target}/debug/examples/fixture_preview"
cargo build --locked -p omavless-tui --example private_import_preview
python3 tests/test-tui-import-terminal.py "${CARGO_TARGET_DIR:-target}/debug/examples/private_import_preview"
# Real ordinary client, fixed same-UID synthetic peer; not daemon/store/VPN acceptance.
cargo build --locked -p omavless-runtime --bin omavless --no-default-features --features tui
bash tests/run-tui-native-import-wire.sh "${CARGO_TARGET_DIR:-target}/debug/omavless"
# Retain this immutable artifact on either outcome; never rebuild an active ELF.
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo check --locked -p omavless-runtime --features tui
cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --features tui tui_commands_conform_to_canonical_mutation_parser
cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --features tui tui_private_import_uses_existing_canonical
cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --bin omavless --features tui tui_import_entry::tests
cargo clippy --locked -p omavless-runtime --features tui --all-targets -- -D warnings
# The opt-in client is not in default packages. These filters run only memory/
# parser/TestBackend controls; resource/terminal gates remain ignored.
rustfmt --edition 2024 --check \
  crates/omavless-runtime/src/native_coordinator/connection_close_client_integration.rs \
  crates/omavless-runtime/src/native_coordinator/connection_close_client_terminal.rs \
  crates/omavless-runtime/src/native_coordinator/connection_close_real_cli_foot.rs
cargo test "${sha2_test_profile[@]}" --locked -p omavless-tui --features developer-conditional-close
cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --features developer-conditional-close --lib developer_connection_close::tests
cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --features developer-conditional-close --lib \
  native_coordinator::connection_close::tests::terminal_demo_is_closed_and_original_operation_cannot_be_replaced_or_resent -- --exact
cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --features developer-conditional-close --lib \
  native_coordinator::connection_close::tests::client_reply_diagnostic_discards_private_values_and_reports_only_closed_enums -- --exact
cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --features developer-conditional-close --lib real_ui_
cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --lib managed_close_receipt::tests
cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --features developer-conditional-close --lib release_pair::tests
cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --lib managed_pair::tests
# Backup-only is a distinct artifact configuration, not inferred from the
# product+T4 union below. These are ordinary source controls; VM gates stay ignored.
cargo test "${sha2_test_profile[@]}" --locked -p omavless-tui --features private-backup
cargo test "${sha2_test_profile[@]}" --locked -p omavless-tui --features developer-conditional-close,private-backup
cargo clippy --locked -p omavless-tui --all-targets --features developer-conditional-close,private-backup -- -D warnings
cargo check --locked -p omavless-runtime --no-default-features --features t4-manager-actor-service
cargo clippy --locked -p omavless-runtime --all-targets --features t4-manager-actor-service -- -D warnings
cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --lib --features t4-manager-actor-service normal_pair
cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --lib --features t4-manager-actor-service backup_tui_
cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --lib --features t4-manager-actor-service restore_tui_
# Public 0.9.8 narrows this research implementation to Backup. Retain full
# T4-only Restore/fence tests above and check the product restriction separately.
cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --features product-private-backup --lib product_scope
cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --features product-private-backup --lib production_owner::tests::
cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --features product-private-backup --lib normal_pair_promoted_candidate
cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --features product-private-backup --bin omavless normal_tui_
cargo clippy --locked -p omavless-runtime --all-targets --features product-private-backup -- -D warnings
# Released setup helper is a separate executable feature. Backup-only tests
# alone do not compile or exercise its retained update/preparation boundaries.
# These are owned inert source fixtures, not installed host/DNS acceptance.
cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --features product-private-backup,setup-upgrade082 --lib setup_upgrade082
cargo clippy --locked -p omavless-runtime --all-targets --features product-private-backup,setup-upgrade082 -- -D warnings
cargo test "${sha2_test_profile[@]}" --locked -p omavless-dns-broker --features release-package --lib maintenance_mask -- --test-threads=1
cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --lib --features product-private-backup,product-image-witness product_scope
cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --lib --features product-private-backup,product-image-witness normal_pair_promoted_candidate
# Internal 0.9.8 assembly: compilation selects both opt-ins, not host activation.
# Run ordinary synthetic/private-file tests only; all VM/resource gates stay ignored.
cargo check --locked -p omavless-runtime --all-targets --features product-image-witness,t4-manager-actor-service
cargo clippy --locked -p omavless-runtime --all-targets --features product-image-witness,t4-manager-actor-service -- -D warnings
# This real-crypto fixture checks downstream canonical-stage retention after
# authentication. Keep its five-second budget and every assertion, but avoid
# spending that budget beside unrelated crypto/process-heavy union tests.
# It exists only with t4-manager-actor-service, not in the default runtime.
canonical_stage_retention_test='manager_actor_service::transfer::tests::failed_canonical_stage_keeps_auth_and_normalized_private_returns'
cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --lib --features product-image-witness,t4-manager-actor-service -- \
  --skip core::tests::helper_resources_are_drained_even_after_leader_exit_or_term_spawn \
  --skip "$canonical_stage_retention_test" \
  "${close_drain_skips[@]}"
for close_drain_test in "${close_drain_tests[@]}"; do
  cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --lib --features product-image-witness,t4-manager-actor-service \
    "$close_drain_test" -- --exact --test-threads=1
done
cargo test "${sha2_test_profile[@]}" --locked -p omavless-runtime --lib --features product-image-witness,t4-manager-actor-service \
  "$canonical_stage_retention_test" -- --exact --test-threads=1
cargo run --quiet --locked -p omavless-parity -- \
  compare tests/parity_cases/r0-reference.json tests/parity_cases/r0-candidate.json
