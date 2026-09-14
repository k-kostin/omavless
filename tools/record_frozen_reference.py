# SPDX-License-Identifier: MIT
"""Explicit synthetic-test recorder; never use with real profile input.

Requires a clean detached checkout of the fixed historical reference. It does
not fetch Git, execute Rust, accept arbitrary commands, or update expectations
when repeated reference behavior disagrees. Existing fixtures are immutable.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

from frozen_reference import REFERENCE, ROOT, LIMIT, request_key, fixture_path

# Closed list of reviewed, synthetic-only archive adapters. A tracked file is
# not sufficient authority to execute some other historical utility.
ORACLES = frozenset({
    "connection_pointer_parity.py",
    "control_protocol_parity.py",
    "core_setup_parity.py",
    "custom_rule_mutation_parity.py",
    "custom_rules_read_parity.py",
    "desktop_helpers_parity.py",
    "diagnostic_projection_parity.py",
    "global_selector_reference.py",
    "import_preview_parity.py",
    "mihomo_probe_response_parity.py",
    "non_vless_canonical_parity.py",
    "onboarding_completion_parity.py",
    "private_store_bootstrap_parity.py",
    "private_store_parity.py",
    "probe_plan_parity.py",
    "probe_resolver_parity.py",
    "profile_classification_parity.py",
    "profile_details_parity.py",
    "profile_edit_input_parity.py",
    "profile_export_parity.py",
    "profile_import_parity.py",
    "profile_mutation_parity.py",
    "route_check_parity.py",
    "route_observation_parity.py",
    "routing_domain_parity.py",
    "routing_preset_parity.py",
    "rule_provider_parity.py",
    "rule_provider_refresh_parity.py",
    "startup_preferences_parity.py",
    "store_state_parity.py",
    "subscription_feed_parity.py",
    "subscription_mutation_parity.py",
    "support_diagnostics_parity.py",
    "vless_authority_parity.py",
    "vless_canonical_parity.py",
    "vless_encryption_parity.py",
    "vless_flow_packet_parity.py",
    "vless_query_metadata_parity.py",
    "vless_reality_parity.py",
    "vless_transport_parity.py",
    "vless_xhttp_download_parity.py",
    "vless_xhttp_extra_shape_parity.py",
    "vless_xhttp_options_parity.py",
})


def record(tool, arguments, payload):
    if tool not in ORACLES:
        raise ValueError("reference_not_a_frozen_pure_oracle")
    key = request_key(arguments, payload)
    destination = fixture_path(tool, key)
    archive = Path(os.environ["OMAVLESS_REFERENCE_CHECKOUT"])
    if not archive.is_absolute() or archive.resolve(strict=True) != archive or archive == ROOT:
        raise ValueError("reference_checkout_invalid")
    def git(*args):
        return subprocess.check_output(["git", "-C", str(archive), *args],
                                       stderr=subprocess.DEVNULL, timeout=15)
    if (git("rev-parse", "HEAD").decode().strip() != REFERENCE
            or git("status", "--porcelain", "--untracked-files=normal")):
        raise ValueError("reference_checkout_changed")
    # Only a tracked regular oracle with a matching committed blob may run.
    source = archive / "tools" / tool
    if source.is_symlink() or not source.is_file():
        raise ValueError("reference_tool_invalid")
    if git("show", REFERENCE + ":tools/" + tool) != source.read_bytes():
        raise ValueError("reference_tool_changed")
    env = dict(os.environ)
    env.pop("OMAVLESS_RECORD_REFERENCE", None)
    result = subprocess.run([sys.executable, str(source), *arguments], input=payload,
                            capture_output=True, cwd=archive, env=env, timeout=45)
    if result.returncode not in (0, 1, 2) or len(result.stdout) + len(result.stderr) > LIMIT:
        raise ValueError("reference_result_invalid")
    value = dict(schemaVersion=1, referenceCommit=REFERENCE, tool=tool,
                 requestSha256=key, inputBytes=len(payload), argumentCount=len(arguments),
                 returncode=result.returncode, stdout=result.stdout.decode("utf-8", "strict"),
                 stderr=result.stderr.decode("utf-8", "strict"))
    encoded = (json.dumps(value, ensure_ascii=True, sort_keys=True, indent=2) + "\n").encode()
    if len(encoded) > LIMIT:
        raise ValueError("reference_result_invalid")
    destination.parent.mkdir(parents=True, exist_ok=True)
    # Publish complete bytes without replacement, even when two tests produce
    # the same vector concurrently. Never read another writer's partial file.
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=destination.parent, prefix='.record-', delete=False) as stream:
            temporary = Path(stream.name)
            stream.write(encoded)
            stream.flush()
            os.fsync(stream.fileno())
        try:
            os.link(temporary, destination)
        except FileExistsError:
            if destination.is_symlink() or destination.read_bytes() != encoded:
                raise ValueError("reference_nondeterministic_or_existing_fixture_changed")
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
    return value
