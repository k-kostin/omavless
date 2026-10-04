#!/bin/bash
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"

python3 -m unittest -v \
  "$here/test_six_library_mapping_boundaries_bridge.py" \
  "$here/test_six_library_mapping_boundaries_transport.py" \
  "$here/test_six_library_mapping_boundaries_fixture.py" \
  "$here/test_six_library_mapping_boundaries_lifecycle.py" \
  "$here/test_six_library_mapping_boundary_events.py" \
  "$here/test_six_library_copy_admission.py" \
  "$here/test_six_library_static_closure.py" \
  "$here/test_six_library_catalog_diagnostic.py" \
  "$here/test_six_library_exact_package_directory.py" \
  "$here/test_encoder_libm_mapping_boundaries_bridge.py" \
  "$here/test_encoder_libm_mapping_boundaries_transport.py" \
  "$here/test_encoder_libm_mapping_boundaries_fixture.py" \
  "$here/test_encoder_libm_mapping_boundaries_lifecycle.py" \
  "$here/test_encoder_libm_mapping_boundary_events.py" \
  "$here/test_encoder_libm_closure.py" \
  "$here/test_encoder_libm_copy_admission.py" \
  "$here/test_encoder_unlisted.py" \
  "$here/test_encoder_boundary_provenance.py" \
  "$here/test_encoder_boundary_owned.py" \
  "$here/test_encoder_boundary_transport.py" \
  "$here/test_encoder_boundary_events.py" \
  "$here/test_brotli_encoder_provenance.py" \
  "$here/test_brotli_encoder_owned.py" \
  "$here/test_brotli_encoder_transport.py" \
  "$here/test_brotli_decoder_provenance.py" \
  "$here/test_brotli_decoder_owned.py" \
  "$here/test_brotli_decoder_transport.py" \
  "$here/test_frozen_reference.py" \
  "$here/test_backend_launcher.py" \
  "$here/test_native_launcher_no_python.py" \
  "$here/test_install_picker_policy.py" \
  "$here/test_marketplace_setup.py" \
  "$here/test_mihomo_validation_effects.py" \
  "$here/test_local_arch_package.py" \
  "$here/test_release_candidate.py" \
  "$here/test_frontend_pair.py" \
  "$here/test_core_connections_adapter.py" \
  "$here/test_core_dns_interop.py" \
  "$here/test_composed_dns_binary.py" \
  "$here/test_real_resolved_binary.py" \
  "$here/test_real_resolved_inventory.py" \
  "$here/test_private_tmpfs_elf.py" \
  "$here/test_reviewed_tmpfs_elf.py" \
  "$here/test_decoder_copy_admission.py" \
  "$here/test_reviewed_tmpfs_bridge.py" \
  "$here/test_reviewed_tmpfs_fixture.py" \
  "$here/test_live_fd_bridge.py" \
  "$here/test_live_fd_enumeration.py" \
  "$here/test_live_fd_fixture.py" \
  "$here/test_live_fd_transport.py" \
  "$here/test_live_fd_review2_fixture.py" \
  "$here/test_static_elf_provenance.py" \
  "$here/test_static_elf_four_mib.py" \
  "$here/test_static_capture_supervisor.py" \
  "$here/test_static_receipt_validator.py" \
  "$here/test_alpm_files_diagnostic.py" \
  "$here/test_package_bound_diagnostic.py" \
  "$here/test_core_artifact_export.py" \
  "$here/test_installed_native_acceptance.py" \
  "$here/test_installed_native_domain.py" \
  "$here/test_installed_native_bridge.py" \
  "$here/test_installed_native_package.py" \
  "$here/test_installed_python_mask.py" \
  "$here/test_human_authorization.py" \
  "$here/test_native_service_acceptance.py" \
  "$here/test_native_live_protocol_validation.py" \
  "$here/test_native_dns_readback.py" \
  "$here/test_staged_native_unit_acceptance.py" \
  "$here/test_control_protocol_parity.py" \
  "$here/test_profile_classification_parity.py" \
  "$here/test_vless_authority_parity.py" \
  "$here/test_vless_query_metadata_parity.py" \
  "$here/test_vless_flow_packet_parity.py" \
  "$here/test_vless_reality_parity.py" \
  "$here/test_vless_encryption_parity.py" \
  "$here/test_vless_transport_parity.py" \
  "$here/test_vless_canonical_parity.py"
if command -v node >/dev/null 2>&1; then
  node "$here/test-documentation-navigation.js"
  node "$here/test-i18n.js"
  node "$here/test-tui-i18n.js"
  node "$here/test-marketplace-setup.js"
  node "$here/test-panel-search.js"
  node "$here/test-native-snapshot.js"
  node "$here/test-native-actions.js"
  node "$here/test-native-onboarding.js"
  node "$here/test-native-main-panel.js"
  node "$here/test-marketplace-assets.js"
  node "$here/test-native-ipc-controls.js"
  node "$here/test-native-ipc-reads.js"
  node "$here/test-native-no-python.js"
  node "$here/test-native-connection-test.js"
  node "$here/test-probe-semantics.js"
  node "$here/test-native-import.js"
  node "$here/test-native-subscriptions.js"
  node "$here/test-native-routing.js"
  node "$here/test-native-batch.js"
  node "$here/test-native-support.js"
  node "$here/test-native-settings-readiness.js"
  node "$here/test-native-open-app.js"
  node "$here/test-native-startup-ui.js"
  node "$here/test-native-file-export.js"
  node "$here/test-native-routing-panel.js"
  node "$here/test-native-diagnostics.js"
  node "$here/test-native-core-setup.js"
  node "$here/test-native-quit.js"
  node "$here/test-native-profile-details.js"
  node "$here/test-native-qr.js"
  node "$here/test-native-editor.js"
  node "$here/test-native-presentation.js"
  node "$here/test-traffic-reference.js"
  node "$here/test-native-traffic.js"
  node "$here/test-native-ping.js"
else
  echo "node unavailable: i18n runtime tests not run" >&2
  exit 1
fi
bash "$here/test-qml.sh"
if command -v omarchy >/dev/null 2>&1; then
  omarchy plugin validate "$here/.."
fi
