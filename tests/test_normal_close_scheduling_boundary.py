"""Source-retention guards; behavioral evidence belongs to Rust owner tests."""
from pathlib import Path
import re
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "crates/omavless-runtime/src"


class NormalSchedulingBoundary(unittest.TestCase):
    def test_product_preview_is_inert_and_final_confirm_is_new_original_guarded_work(self):
        preview=(SRC/'native_coordinator/connection_close_preview.rs').read_text()
        inert=preview.split('struct InertPreview {',1)[1].split('}',1)[0]
        for forbidden in ('Session','CloseObservation','File','EffectProof','Worker'):
            self.assertNotIn(forbidden,inert)
        discovery=preview.split('pub(crate) fn observe_preview(',1)[1].split('pub(crate) enum ProductConfirmAdmission',1)[0]
        self.assertLess(discovery.index('retire_before_effect()'),discovery.index('Ok(ClosePreviewReady'))
        admit=preview.split('pub(crate) fn admit_product_confirm(',1)[1].split('pub(crate) fn complete_product_confirm(',1)[0]
        self.assertLess(admit.index('reserve_connection_close('),admit.index('capture_connection_close_inner(Some(&token))'))
        self.assertIn('self.coordinator.external_close_reservation_current(&token)',preview)
        session=(SRC/'conditional_close_candidate.rs').read_text()
        selection=session.split('pub(crate) fn matches_preview(',1)[1].split('pub(crate) fn bind(',1)[0]
        for literal in ('same_metadata','row.target.target.id == selected.target.id',
                        'row.target.target.token == selected.target.token','row.display == selected.display',
                        'Arc::ptr_eq(&row.target.session_identity, &self.identity)'):
            self.assertIn(literal,selection)
        server=(SRC/'developer_connection_close.rs').read_text()
        self.assertIn('(*task).observe()',server)
        self.assertIn('(*discovery).observe_preview()',server)
        host=(SRC/'native_host.rs').read_text()
        self.assertRegex(host,r'#\[cfg\(all\(test, feature = "product-image-witness"\)\)\]\s*pub\(crate\) fn install_product_preview_fixture_for_test')
    def test_snapshot_wire_fixture_covers_actual_final_dispatch_wrapper(self):
        import json
        source = (SRC / 'developer_connection_close.rs').read_text()
        dispatch = source.split('pub(super) fn dispatch_developer_close(', 1)[1].split('#[cfg(test)]', 1)[0]
        self.assertIn('snapshot_success_response(id, owner.revision(), &self.instance_id, result)', dispatch)
        wrapper = source.split('fn snapshot_success_response(', 1)[1].split('impl<H:', 1)[0]
        self.assertIn('result["instanceId"] = json!(instance)', wrapper)
        self.assertIn('success_response(id, revision, result)', wrapper)
        self.assertNotIn('observe(', wrapper)
        self.assertNotIn('permit', wrapper)
        test = source.split('fn snapshot_final_server_serialization_matches_client_fixture()', 1)[1].split('#[test]', 1)[0]
        self.assertIn('omavless_control_protocol::encode_response(&wire)', test)
        self.assertIn('omavless_control_protocol::decode_response(&encoded)', test)
        fixture = json.loads((ROOT / 'tests/fixtures/t3_snapshot_server_wire.json').read_bytes())
        self.assertEqual(set(fixture['result']), {'schemaVersion', 'scope', 'instanceId', 'rows'})
        self.assertEqual(fixture['result']['instanceId'], 'same-original')

    def test_product_epoch_source_is_optional_and_not_a_default_daemon_flag(self):
        manifest = tomllib.loads((SRC.parent / "Cargo.toml").read_text())
        self.assertEqual(manifest["features"]["product-image-witness"],
                         ["developer-image-witness", "omavless-image-witness/product-epochs"])
        self.assertNotIn("product-image-witness", manifest["features"]["default"])
        helper = SRC.parent.parent / "omavless-image-witness"
        self.assertEqual(tomllib.loads((helper / "Cargo.toml").read_text())
                         ["features"]["product-epochs"], ["developer-helper"])
        self.assertRegex((SRC / "main.rs").read_text(),
                         r'#\[cfg\(feature = "product-image-witness"\)\]\s*if arguments == \["daemon", "--product-image-witness"\]')
        for value in ("/var/lib/omavless-image-product/runtime.enrollment",
                      "/run/omavless-image-product/control.sock",
                      "omavless-product-current-image-v1"):
            self.assertIn(value, (helper / "src/class.rs").read_text())

    def test_original_worker_publication_not_helper_ack_orders_new_epoch(self):
        source = (SRC / "conditional_close_candidate.rs").read_text()
        worker = source.split("impl Scheduler {", 1)[1].split("impl Worker {", 1)[0]
        worker = worker.split(".spawn(move || {", 1)[1]
        self.assertLess(worker.index("product_retirement_ready(outcome)"), worker.index("drop(session)"))
        self.assertLess(worker.index("drop(session)"), worker.index("drop(slot)"))
        self.assertLess(worker.index("drop(slot)"), worker.index("sender.try_send"))
        monitor = source.split("impl CloseEpochCompletion {", 1)[1].split("impl Session {", 1)[0]
        for value in ("Arc::ptr_eq(&self.original.identity", "Arc::ptr_eq(&self.original.lifetime",
                      "gate.live && gate.reservation.is_none()", "self.outcome == Outcome::Closed"):
            self.assertIn(value, monitor)
        close = (SRC / "native_coordinator/connection_close.rs").read_text()
        publication = close.split("pub(crate) fn poll_connection_close(", 1)[1].split("pub(crate) fn connection_close_receipt(", 1)[0]
        self.assertLess(publication.index("finish_external_close"), publication.index("complete_close_epoch"))

    def test_product_capacity_and_busy_checks_precede_old_snapshot_invalidation(self):
        close = (SRC / "native_coordinator/connection_close.rs").read_text()
        capture = close.split("pub(crate) fn capture_connection_close(", 1)[1].split("pub(crate) fn retain_connection_close(", 1)[0]
        self.assertLess(capture.index("close_epoch_admission()"), capture.index("invalidate_connection_close()"))
        self.assertLess(capture.index("ENTROPY_LIMIT -"), capture.index("invalidate_connection_close()"))
        self.assertLess(capture.index("close_receipt_capacity_available()"), capture.index("invalidate_connection_close()"))
        self.assertLess(capture.index("invalidate_connection_close()"), capture.index("self.batch_lock()"))
        host = (SRC / "native_host.rs").read_text()
        capture = host.split("pub(crate) fn capture_connection_close(", 1)[1].split("fn capture_original_close(", 1)[0]
        self.assertLess(capture.index("epochs.reserve()?"), capture.index("CloseImageCapture::ProductWitness"))
        self.assertNotIn(".finish(", capture)
        self.assertNotIn("Client::bind", capture)

    def test_detached_retirement_is_original_offlock_and_drain_precedes_marker(self):
        source = (SRC / "conditional_close_candidate.rs").read_text()
        retire = source.split("pub(crate) fn retire_before_effect(", 1)[1].split("pub(crate) fn refuse_retirement(", 1)[0]
        self.assertLess(retire.index("r.proofs == 0"), retire.index("phase = Phase::Retiring"))
        self.assertLess(retire.index("phase = Phase::Retiring"), retire.index("self.finish_image_witness(deadline)?"))
        self.assertLess(retire.index("self.finish_image_witness(deadline)?"), retire.index("drop(self)"))
        self.assertLess(retire.index("drop(self)"), retire.index("Ok(CloseEpochRetirement"))
        after_drop = retire.split("drop(self);", 1)[1]
        self.assertLess(after_drop.index("remaining(deadline)"), after_drop.index("Ok(CloseEpochRetirement"))
        self.assertIn("self.check_retirement_locked(&mut gate)?", retire)
        self.assertNotIn("self.image_proof_flight", retire)
        self.assertNotIn("self.deadline =", retire)
        self.assertIn("Instant::now() + budget", retire)
        self.assertIn("RETIREMENT_BUDGET", retire)
        self.assertNotIn("Instant::now() + BUDGET", retire)
        close = (SRC / "native_coordinator/connection_close.rs").read_text()
        admit = close.split("pub(crate) fn admit_connection_close_snapshot(", 1)[1].split("pub(crate) fn complete_connection_close_retirement(", 1)[0]
        self.assertLess(admit.index("retiring = Some"), admit.index(".snapshot\n            .take()"))
        complete = close.split("pub(crate) fn complete_connection_close_retirement(", 1)[1].split("pub(super) fn invalidate_connection_close(", 1)[0]
        self.assertGreaterEqual(complete.count("retired.original.retirement_deadline()"), 3)
        self.assertNotIn("retired.expiry", complete)
        self.assertLess(complete.index("close_context_matches"), complete.index("complete_close_retirement"))
        self.assertLess(complete.index("Instant::now() < deadline"), complete.index("retiring = None"))
        self.assertNotIn(".finish(", admit)
        self.assertNotIn(".retire(", admit)
        server = (SRC / "developer_connection_close.rs").read_text()
        server = server.split("pub(super) fn dispatch_developer_close(", 1)[1]
        task = server.split("Admission::Retire(task) =>", 1)[1].split("Admission::Respond(_)", 1)[0]
        self.assertLess(task.index("(*task).retire()"), task.index("self.dispatcher.try_lock()"))
        self.assertLess(task.index("self.dispatcher.try_lock()"), task.index("owner.developer_close_retired(result)"))

    def test_current_image_helper_is_separate_default_off_and_passive_gate_unpromoted(self):
        manifest=tomllib.loads((SRC.parent/"Cargo.toml").read_text())
        self.assertEqual(manifest["features"]["developer-image-witness"],
            ["developer-conditional-close","dep:omavless-image-witness","omavless-image-witness/developer-helper","dep:rustix"])
        self.assertNotIn("developer-image-witness",manifest["features"]["default"])
        helper=SRC.parent.parent/"omavless-image-witness"
        spec=tomllib.loads((helper/"Cargo.toml").read_text())
        self.assertEqual(spec["features"]["default"],[])
        self.assertEqual(spec["bin"][0]["required-features"],["developer-helper"])
        source=(helper/"src/kernel.rs").read_text()
        classes=(helper/"src/class.rs").read_text()
        self.assertIn('"/usr/lib/omavless-image/development-runtime-tests"',classes)
        self.assertIn("PIDFS",source)
        passive=(SRC/"native_coordinator/connection_close_image_witness.rs").read_text()
        self.assertNotIn("adopt_owned_close_fixture(",passive)
        self.assertNotIn("CandidateEffectPermit",passive)
        self.assertNotIn("confirm_connection_close(",passive)
        self.assertIn("getppid().as_raw(), 1",passive)

    def test_installed_development_class_is_fixed_explicit_and_same_owner(self):
        helper=SRC.parent.parent/"omavless-image-witness"
        classes=(helper/"src/class.rs").read_text()
        for value in ('"/usr/bin/omavless"','"/run/omavless-image-runtime/control.sock"','"/var/lib/omavless-image/development-runtime-enrollment-v1"','"omavless-development-installed-runtime-current-image-v1"'):
            self.assertIn(value,classes)
        main=(SRC/"main.rs").read_text()
        self.assertRegex(main,r'#\[cfg\(feature = "developer-image-witness"\)\]\s*if arguments == \["daemon", "--developer-image-witness"\]')
        owner=(SRC/"production_owner.rs").read_text()
        self.assertIn("Self::current_with_image(runtime_paths, CloseImageSelection::Direct)",owner)
        self.assertIn("Self::current_with_image(runtime_paths, CloseImageSelection::InstalledDevelopment)",owner)
        host=(SRC/"native_host.rs").read_text()
        method=host.split("    pub(crate) fn capture_connection_close(",1)[1].split("    fn capture_original_close(",1)[0]
        self.assertLess(method.index("self.development_image = DevelopmentImageState::Consumed"),method.index("CloseImageCapture::InstalledRuntimeWitness"))
        self.assertIn("DevelopmentImageState::Consumed => return Err",method)
        self.assertNotIn("bind_original",method)
        self.assertNotIn(".observe(",method)

    def test_current_image_rpc_is_off_gate_before_lease_and_held_through_effect(self):
        session=(SRC/"conditional_close_candidate.rs").read_text()
        local=session.split("    fn check_locked(&self",1)[1].split("    fn connected(",1)[0]
        self.assertNotIn("client.observe(",local)
        self.assertNotIn("Client::bind_original(",local)
        flight=session.split("    fn image_proof_flight(",1)[1].split("    fn check_with_flight(",1)[0]
        self.assertLess(flight.index("self.begin_proof_flight()?"),flight.index("Client::bind_original("))
        effect=session.split("    fn effect_lease(",1)[1].split("pub(crate) fn pause_proof_after(",1)[0]
        self.assertLess(effect.index("self.image_proof_flight()?"),effect.index(".lease()"))
        exchange=session.split("    fn exchange_request(",1)[1].split("    fn pause(",1)[0]
        self.assertLess(exchange.index("self.effect_lease(false)?"),exchange.index("stream.write(pending)"))
        self.assertLess(exchange.index("stream.write(pending)"),exchange.index("drop(effect_lease)"))
        terminal=session.split("    fn finish(&mut self",1)[1].split("    pub(crate) fn discover(",1)[0]
        self.assertLess(terminal.index("self.effect_lease(true)"),terminal.index("r.phase = Phase::Finished"))

    def test_image_flight_releases_current_fd_before_drain_and_no_fallback(self):
        session=(SRC/"conditional_close_candidate.rs").read_text()
        drop=session.split("impl Drop for ProofFlight",1)[1].split("struct EffectLease",1)[0]
        self.assertLess(drop.index("drop(self.current.take())"),drop.index("r.proofs ="))
        refresh=session.split("    fn image_proof_flight(",1)[1].split("    fn check_with_flight(",1)[0]
        self.assertNotIn("ExecutableEvidence::image(",refresh)
        self.assertIn("self.revoke_image()",refresh)
        for leaf in ("conditional_developer_pair.rs","conditional_release_pair.rs"):
            pair=(SRC/leaf).read_text()
            self.assertIn("check_current(",pair)
            self.assertIn("flight.current.as_ref()",pair)
            self.assertNotIn("image.check(pid)",pair)

    def test_prepared_helper_session_is_test_only_one_shot_not_recreated(self):
        host=(SRC/"native_host.rs").read_text()
        self.assertRegex(host,r'#\[cfg\(all\(test, feature = "developer-image-witness"\)\)\]\s*pub\(crate\) fn prepare_image_witness_close_fixture')
        constructor=host.split("    pub(crate) fn prepare_image_witness_close_fixture(",1)[1].split("    fn validate_prepared_image(",1)[0]
        self.assertLess(constructor.index("self.image_fixture_attempted = true"),constructor.index("self.capture_original_close("))
        self.assertLess(constructor.index("observation.observe()?"),constructor.index("self.prepared_image = Some(prepared)"))
        self.assertIn("observation.fixture_permit().is_some()",constructor)
        self.assertIn("prepared.desired != desired",host)
        self.assertIn("self.paths.store != prepared.store_path",host)
        self.assertIn("self.prepared_image.take()",host)
        self.assertGreaterEqual(host.count("self.revoke_prepared_image();"),9)

    def test_qualified_package_is_same_session_private_and_nondefault_not_boolean_promotion(self):
        session=(SRC/"conditional_close_candidate.rs").read_text()
        self.assertRegex(session,r'#\[cfg\(feature = "developer-conditional-close"\)\]\s*pub\(crate\) fn qualified_pair_permit')
        self.assertIn("qualification_attempted = true",session)
        self.assertIn("self.check_qualified_pair(flight.current.as_ref())?",session.split("fn effect_lease",1)[1].split("#[cfg(test)]",1)[0])
        qualification=(SRC/"conditional_release_pair.rs").read_text()
        self.assertNotIn("CandidateEffectPermit",qualification)
        self.assertIn("image.image_identity != self.core.identity",qualification)
        self.assertIn("image.source_identity != self.core.identity",qualification)
        self.assertIn("SELECTION_BYTES",qualification)
        self.assertIn("self.original.refuse()",qualification)
        self.assertIn("return Ok(None)",qualification)
        receipt=(SRC/"managed_close_receipt.rs").read_text()
        self.assertIn('"omavless-managed-dns-close-pair-v1"',receipt)
        self.assertIn("deny_unknown_fields",receipt)
        self.assertIn("r.conditional_close_abi != 1",receipt)
        self.assertIn("r.patch_sha256.close != CLOSE_PATCH",receipt)

    def test_shared_transition_is_normal_compiled_private_and_permit_bound(self):
        text = (SRC / "native_coordinator/connection_close.rs").read_text()
        name = "schedule_permitted_connection_close"
        body = text.split(f"    fn {name}(", 1)[1].split("    pub(crate) fn poll_connection_close", 1)[0]
        self.assertNotRegex(text, rf"#\[cfg\(test\)\]\s*(?://[^\n]*\n\s*)*fn {name}")
        self.assertNotRegex(text, rf"pub(?:\([^)]*\))?\s+fn {name}")
        self.assertIn("permit: crate::conditional_close_candidate::CandidateEffectPermit", body)
        self.assertIn("lease: &MigrationLock", body)
        self.assertLess(body.index("lease.authorizes("), body.index(".into_session()"))
        self.assertLess(body.index("session.authorize_effect("), body.index(".start(session, selected.target, permit)"))
        self.assertIn("EffectProof(snapshot.context)", body)
        self.assertIn("snapshot.expiry", body)

    def test_fixture_permit_constructor_and_admission_remain_test_only(self):
        text = (SRC / "conditional_close_candidate.rs").read_text()
        self.assertRegex(text, r"#\[cfg\(test\)\]\s*impl CandidateEffectPermit")
        self.assertIn("struct CandidateEffectPermit {\n    _private: (),", text)
        host = (SRC / "native_host.rs").read_text()
        self.assertRegex(host, r"#\[cfg\(test\)\]\s*pub\(crate\) fn fixture_permit")
        self.assertNotRegex(host, r"#\[cfg\(test\)\]\s*pub\(crate\) fn into_session")
        close = (SRC / "native_coordinator/connection_close.rs").read_text()
        admission = close.split("    fn confirm_connection_close_reserved(", 1)[1].split("    fn schedule_permitted_connection_close", 1)[0]
        self.assertRegex(admission, r"#\[cfg\(test\)\]\s*if let Some\(permit\)")
        self.assertIn("ExternalCloseOutcome::MissingAttestation", admission)
        self.assertEqual(admission.count(".scheduler"), 0)
        self.assertIn("schedule_permitted_connection_close(snapshot, selected, token, permit, &_lease)", re.sub(r"\s+", " ", admission))

    def test_developer_pair_is_a_distinct_nondefault_private_constructor(self):
        manifest = tomllib.loads((SRC.parent / "Cargo.toml").read_text())
        self.assertEqual(manifest["features"]["developer-conditional-close"],
                         ["omavless-tui?/developer-conditional-close"])
        self.assertNotIn("developer-conditional-close", manifest["features"]["default"])
        text = (SRC / "conditional_close_candidate.rs").read_text()
        self.assertRegex(text, r'#\[cfg\(feature = "developer-conditional-close"\)\]\s*pub\(crate\) fn developer_pair_permit')
        package = (SRC / "conditional_package_evidence.rs").read_text()
        self.assertIn('mod developer_pair;', package)
        developer = (SRC / "conditional_developer_pair.rs").read_text()
        self.assertIn('"omavless-developer-conditional-pair-v1"', developer)
        self.assertIn('|| r.production_adoption', developer)
        close = (SRC / "native_coordinator/connection_close.rs").read_text()
        admission = close.split("    fn confirm_connection_close_reserved(", 1)[1].split("    fn schedule_permitted_connection_close", 1)[0]
        self.assertIn(".proves_live_for_scheduling()", admission)
        self.assertNotIn(".proves_live()", admission)

    def test_developer_client_is_weakly_propagated_and_explicitly_dual_gated(self):
        manifest = tomllib.loads((SRC.parent / "Cargo.toml").read_text())
        feature = manifest["features"]["developer-conditional-close"]
        # Exact weak propagation must not pull the optional TUI into a headless
        # build or place developer methods into a default build.
        self.assertEqual(feature, ["omavless-tui?/developer-conditional-close"])
        self.assertNotIn("developer-conditional-close", manifest["features"]["default"])
        tui = SRC.parent.parent / "omavless-tui"
        self.assertEqual(tomllib.loads((tui / "Cargo.toml").read_text())
                         ["features"]["developer-conditional-close"], [])
        main = (SRC / "main.rs").read_text()
        self.assertRegex(main, r'#\[cfg\(all\(feature = "tui", feature = "developer-conditional-close"\)\)\]\s*if arguments == \["tui", "--developer-conditional-close"\]')
        self.assertRegex((tui / "src/lib.rs").read_text(),
                         r'#\[cfg\(feature = "developer-conditional-close"\)\]\s*pub mod developer_close;')


if __name__ == "__main__":
    unittest.main()
