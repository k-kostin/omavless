// SPDX-License-Identifier: MIT
//! Real registry/worker/domain/fixed-writer integration. Invented private data
//! only; no installed provider, package, owner, TUN or service is involved.
use super::epoch_tests::{desired_paths, proof, receipt, witness};
use super::tests::{ordinary_edit, prepared};
use super::*;
use crate::desired::{DesiredState, OwnedObservation};
use crate::lifecycle::{HostStepError, LifecycleHost};
use crate::native_coordinator::{
    NativeOwnerError, NativeSubscriptionBatch, OfflineNativeCoordinator,
};
use crate::restore_successor_publication_candidate::tests::Fixture;
use crate::subscription_batch_work::{BatchWorkStep, BudgetedSubscriptionTransport};
use crate::subscription_transport::{HttpsSubscriptionTransport, SubscriptionTransportError};
use omavless_domain::private_store::{
    IncomingSubscriptionProfile, SubscriptionMutation, SubscriptionMutationContext,
};
use omavless_domain::subscription_feed::PrivateSubscriptionBody;
use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

const SUBSCRIPTION: &str = "60000000-0000-4000-8000-000000000001";
const PROFILE: &str = "60000000-0000-4000-8000-000000000002";
const URI: &str =
    "vless://11111111-1111-4111-8111-111111111111@192.0.2.60:443?security=none&type=tcp#Fixed";
#[derive(Default)]
struct OffHost;
impl LifecycleHost for OffHost {
    fn observe(&mut self, _: &DesiredState) -> Result<OwnedObservation, HostStepError> {
        Ok(OwnedObservation {
            service_active: false,
            controller_ready: false,
            core_count: 0,
            tun_count: 0,
            active_profile_matches: false,
        })
    }
    fn prepare(&mut self, _: &DesiredState) -> Result<(), HostStepError> {
        panic!("batch cannot prepare core")
    }
    fn start_prepared(&mut self) -> Result<(), HostStepError> {
        panic!("batch cannot start core")
    }
    fn commit_prepared(&mut self) -> Result<(), HostStepError> {
        panic!("batch cannot commit core")
    }
    fn stop_owned(&mut self) -> Result<(), HostStepError> {
        panic!("batch cannot stop core")
    }
    fn discard_prepared(&mut self) -> Result<(), HostStepError> {
        panic!("batch cannot clean core")
    }
}
fn new_owner(f: &Fixture) -> OfflineNativeCoordinator<OffHost> {
    let mut owner = OfflineNativeCoordinator::new_ownership_gated(
        OffHost,
        desired_paths(f),
        &f.config.join(LIVE[0]),
        f.paths.clone(),
        f.uid,
        2,
    );
    owner.initialize_batch_operations("batch-research").unwrap();
    owner
}
fn fixture(
    commit: bool,
    url: Option<&str>,
) -> (
    Fixture,
    OfflineNativeCoordinator<OffHost>,
    DetachedHistoricalBatch,
) {
    let (f, lock) = prepared(commit);
    ordinary_edit(&f);
    receipt(&f);
    if let Some(url) = url {
        crate::subscription_mutation::commit_subscription_mutation(
            &f.config.join(LIVE[0]),
            f.uid,
            SubscriptionMutation::Add {
                subscription_id: SUBSCRIPTION.into(),
                name: "Fixed source".into(),
                url: url.into(),
                entries: vec![IncomingSubscriptionProfile {
                    uri: URI.into(),
                    new_id: PROFILE.into(),
                }],
                updated_at: 1,
            },
            SubscriptionMutationContext::default(),
        )
        .unwrap();
    }
    let owner = new_owner(&f);
    let context = owner
        .detach_batch_research(
            witness(&f, &lock)
                .research(proof(&f, &lock), || true)
                .unwrap(),
        )
        .unwrap();
    drop(lock); // No borrowed lock survives into the worker/evidence object.
    (f, owner, context)
}
fn request(method: &str, id: &str) -> Value {
    json!({"api":"omavless.control","version":1,"id":"fixed-batch","method":method,"params":{"instanceId":"batch-research","operationId":id}})
}

/// Genuine retained Off evidence from a different owner, solely for testing
/// rejection at the shared typed-batch entry while an actual close is pending.
/// No witness/permit constructor or injected proof Boolean is introduced.
pub(crate) fn foreign_empty_context_fixture() -> (Fixture, DetachedHistoricalBatch) {
    let (fixture, _owner, context) = fixture(false, None);
    (fixture, context)
}
fn status(owner: &OfflineNativeCoordinator<OffHost>, id: &str) -> Value {
    owner
        .subscription_batch_status(&request("operations.get", id))
        .unwrap()["operation"]
        .clone()
}
fn start(
    owner: &mut OfflineNativeCoordinator<OffHost>,
    context: &mut DetachedHistoricalBatch,
    id: &str,
) -> NativeSubscriptionBatch {
    owner
        .start_subscription_batch_research(&request("subscriptions.refresh_all", id), context)
        .unwrap()
        .unwrap()
}
struct FixedTransport<'a>(&'a Fixture);
impl BudgetedSubscriptionTransport for FixedTransport<'_> {
    fn fetch_with_budget(
        &self,
        _: &str,
        _: std::time::Duration,
    ) -> Result<PrivateSubscriptionBody, SubscriptionTransportError> {
        let lease = self.0.lock(); // Actual migration lease is free DURING fetch.
        drop(lease);
        Ok(PrivateSubscriptionBody::from_bytes(URI.as_bytes().to_vec()).unwrap())
    }
}
fn fetch(f: &Fixture, job: &mut NativeSubscriptionBatch) {
    assert_eq!(
        job.step(
            &FixedTransport(f),
            &crate::remote_fetch::RemoteFetchPool::default(),
            &mut || PROFILE.into()
        )
        .unwrap(),
        BatchWorkStep::Ready
    );
}
fn history(f: &Fixture) -> Vec<(Vec<u8>, Metadata)> {
    [CLOSURE_MEMBER, TICKET_MEMBER, COMPLETE_MEMBER]
        .into_iter()
        .map(|name| {
            let path = f.paths.state_directory.join(name);
            (fs::read(&path).unwrap(), fs::metadata(path).unwrap())
        })
        .collect()
}
fn assert_history(f: &Fixture, old: &[(Vec<u8>, Metadata)]) {
    for ((bytes, metadata), (now, current)) in old.iter().zip(history(f)) {
        assert_eq!(*bytes, now);
        assert!(same_member(metadata, &current));
    }
    assert!(crate::pending_private_transaction::pending_at(
        &f.paths.state_directory
    ));
}

#[test]
fn historical_batch_actual_commit_replay_second_fetch_preserves_history() {
    for commit in [false, true] {
        let (f, mut owner, mut context) = fixture(commit, Some("https://fixed.invalid/feed"));
        let old = history(&f);
        let desired = fs::read(f.paths.state_directory.join("desired.json")).unwrap();
        let template = fs::read(f.config.join(LIVE[1])).unwrap();
        for revision in 0..2 {
            let id = format!("commit-{revision}");
            let mut job = start(&mut owner, &mut context, &id);
            let before = fs::metadata(f.config.join(LIVE[0])).unwrap();
            fetch(&f, &mut job);
            owner.publish_subscription_batch_progress(&job).unwrap();
            let mut calls = 0;
            owner
                .complete_subscription_batch_research(
                    job,
                    || {
                        calls += 1;
                        20 + revision
                    },
                    &mut context,
                )
                .unwrap();
            assert_eq!(calls, 1);
            assert_eq!(owner.revision(), revision + 1);
            assert_eq!(status(&owner, &id)["state"], "succeeded");
            let after = fs::metadata(f.config.join(LIVE[0])).unwrap();
            assert_ne!(before.ino(), after.ino());
            assert!(
                owner
                    .start_subscription_batch_research(
                        &request("subscriptions.refresh_all", &id),
                        &mut context
                    )
                    .unwrap()
                    .is_none()
            );
            assert!(same_member(
                &after,
                &fs::metadata(f.config.join(LIVE[0])).unwrap()
            ));
            assert!(
                !owner
                    .cancel_subscription_batch(&request("operations.cancel", &id))
                    .unwrap()
            );
            assert_history(&f, &old);
        }
        assert_eq!(
            fs::read(f.paths.state_directory.join("desired.json")).unwrap(),
            desired
        );
        assert_eq!(fs::read(f.config.join(LIVE[1])).unwrap(), template);
        assert!(
            owner
                .start_subscription_batch(&request("subscriptions.refresh_all", "ordinary"))
                .is_err()
        );
        // Historical replay revalidates BEFORE the registry's cached retry.
        fs::write(f.paths.runtime_base.join("omavless-login.receipt"), b"torn").unwrap();
        assert!(
            owner
                .start_subscription_batch_research(
                    &request("subscriptions.refresh_all", "commit-1"),
                    &mut context
                )
                .is_err()
        );
    }
}

#[test]
fn historical_batch_empty_preserves_full_inode_no_clock_or_revision() {
    let (f, mut owner, mut context) = fixture(true, None);
    context.fault = Some(Box::new(|_| {
        panic!("empty batch must not reach writer checkpoints")
    }));
    let old = fs::metadata(f.config.join(LIVE[0])).unwrap();
    let bytes = fs::read(f.config.join(LIVE[0])).unwrap();
    let job = start(&mut owner, &mut context, "empty");
    owner
        .complete_subscription_batch_research(
            job,
            || panic!("empty must not read clock"),
            &mut context,
        )
        .unwrap();
    assert_eq!(owner.revision(), 0);
    assert_eq!(status(&owner, "empty")["state"], "succeeded");
    assert_eq!(fs::read(f.config.join(LIVE[0])).unwrap(), bytes);
    assert!(same_member(
        &old,
        &fs::metadata(f.config.join(LIVE[0])).unwrap()
    ));
    assert!(
        owner
            .start_subscription_batch_research(
                &request("subscriptions.refresh_all", "empty"),
                &mut context
            )
            .unwrap()
            .is_none()
    );
}

#[test]
fn historical_batch_same_text_instance_cannot_steal_or_release_other_owner_job() {
    let (f, mut original, mut evidence) = fixture(true, Some("https://fixed.invalid/feed"));
    let mut old_job = start(&mut original, &mut evidence, "old");
    fetch(&f, &mut old_job);
    let old_ticket = old_job.supervisor_ticket();
    let mut successor = new_owner(&f);
    let lease = f.lock();
    let mut successor_evidence = successor
        .detach_batch_research(
            witness(&f, &lease)
                .research(proof(&f, &lease), || true)
                .unwrap(),
        )
        .unwrap();
    drop(lease);
    let mut new_job = start(&mut successor, &mut successor_evidence, "new");
    // Both registries use the same text instance and first token. Arc identity,
    // not caller text/counter coincidence, prevents a cross-owner capability.
    assert!(
        successor
            .abort_subscription_batch_research(old_ticket.clone(), &mut evidence)
            .is_err()
    );
    assert!(
        successor
            .complete_subscription_batch_research(
                old_job,
                || panic!("wrong owner clock"),
                &mut successor_evidence
            )
            .is_err()
    );
    assert_eq!(status(&successor, "new")["state"], "running");
    fetch(&f, &mut new_job);
    successor
        .complete_subscription_batch_research(new_job, || 20, &mut successor_evidence)
        .unwrap();
    assert_eq!(status(&successor, "new")["state"], "succeeded");
    original
        .abort_subscription_batch_research(old_ticket, &mut evidence)
        .unwrap();
}

#[test]
fn historical_batch_valid_job_with_foreign_bound_context_preserves_both_owners() {
    let (f, mut original, mut evidence) = fixture(true, Some("https://fixed.invalid/feed"));
    let mut original_job = start(&mut original, &mut evidence, "original");
    fetch(&f, &mut original_job);
    let original_ticket = original_job.supervisor_ticket();
    let mut successor = new_owner(&f);
    let lease = f.lock();
    let mut successor_evidence = successor
        .detach_batch_research(
            witness(&f, &lease)
                .research(proof(&f, &lease), || true)
                .unwrap(),
        )
        .unwrap();
    drop(lease);
    let mut successor_job = start(&mut successor, &mut successor_evidence, "successor");
    fetch(&f, &mut successor_job);
    let before_original = status(&original, "original");
    let before_successor = status(&successor, "successor");
    let bytes = fs::read(f.config.join(LIVE[0])).unwrap();
    let metadata = fs::metadata(f.config.join(LIVE[0])).unwrap();
    let old = history(&f);

    // The job is authentic for the receiving owner. Only the genuinely bound
    // context is foreign; equal instance/token/revision text is not authority.
    assert!(matches!(
        original.complete_subscription_batch_research(
            original_job,
            || panic!("foreign context cannot read clock"),
            &mut successor_evidence,
        ),
        Err(NativeOwnerError::OwnershipUnavailable)
    ));
    assert_eq!(status(&original, "original"), before_original);
    assert_eq!(status(&successor, "successor"), before_successor);
    assert_eq!(original.revision(), 0);
    assert_eq!(successor.revision(), 0);
    assert_eq!(fs::read(f.config.join(LIVE[0])).unwrap(), bytes);
    assert!(same_member(
        &metadata,
        &fs::metadata(f.config.join(LIVE[0])).unwrap()
    ));
    assert_history(&f, &old);
    // A by-value rejected job has been consumed, but its genuine supervisor
    // ticket can still terminate precisely that owner's unmodified operation.
    original
        .abort_subscription_batch_research(original_ticket, &mut evidence)
        .unwrap();
    assert_eq!(status(&original, "original")["state"], "failed");
    successor
        .complete_subscription_batch_research(successor_job, || 20, &mut successor_evidence)
        .unwrap();
    assert_eq!(status(&successor, "successor")["state"], "succeeded");
    assert_eq!(successor.revision(), 1);
    assert_history(&f, &old);
}

#[test]
fn historical_batch_valid_ticket_with_foreign_bound_context_preserves_both_owners() {
    let (f, mut original, mut evidence) = fixture(true, None);
    let original_job = start(&mut original, &mut evidence, "original");
    let mut successor = new_owner(&f);
    let lease = f.lock();
    let mut successor_evidence = successor
        .detach_batch_research(
            witness(&f, &lease)
                .research(proof(&f, &lease), || true)
                .unwrap(),
        )
        .unwrap();
    drop(lease);
    let successor_job = start(&mut successor, &mut successor_evidence, "successor");
    let before_original = status(&original, "original");
    let before_successor = status(&successor, "successor");
    let bytes = fs::read(f.config.join(LIVE[0])).unwrap();
    let metadata = fs::metadata(f.config.join(LIVE[0])).unwrap();
    let old = history(&f);
    assert!(matches!(
        successor
            .abort_subscription_batch_research(successor_job.supervisor_ticket(), &mut evidence),
        Err(NativeOwnerError::OwnershipUnavailable)
    ));
    assert_eq!(status(&original, "original"), before_original);
    assert_eq!(status(&successor, "successor"), before_successor);
    for (owner, context, job, id) in [
        (&mut original, &mut evidence, original_job, "original"),
        (
            &mut successor,
            &mut successor_evidence,
            successor_job,
            "successor",
        ),
    ] {
        owner
            .complete_subscription_batch_research(job, || panic!("empty clock"), context)
            .unwrap();
        assert_eq!(status(owner, id)["state"], "succeeded");
        assert_eq!(owner.revision(), 0);
    }
    assert_eq!(fs::read(f.config.join(LIVE[0])).unwrap(), bytes);
    assert!(same_member(
        &metadata,
        &fs::metadata(f.config.join(LIVE[0])).unwrap()
    ));
    assert_history(&f, &old);
}

#[test]
fn historical_batch_valid_start_with_foreign_bound_context_preserves_original() {
    let (f, mut original, mut evidence) = fixture(true, Some("https://fixed.invalid/feed"));
    let mut original_job = start(&mut original, &mut evidence, "original");
    fetch(&f, &mut original_job);
    let before_original = status(&original, "original");
    let bytes = fs::read(f.config.join(LIVE[0])).unwrap();
    let metadata = fs::metadata(f.config.join(LIVE[0])).unwrap();
    let old = history(&f);
    let mut successor = new_owner(&f);
    assert!(matches!(
        successor.start_subscription_batch_research(
            &request("subscriptions.refresh_all", "foreign-start"),
            &mut evidence,
        ),
        Err(NativeOwnerError::OwnershipUnavailable)
    ));
    assert!(
        successor
            .subscription_batch_status(&request("operations.get", "foreign-start"))
            .is_err()
    );
    assert_eq!(status(&original, "original"), before_original);
    assert_eq!(successor.revision(), 0);
    assert_eq!(original.revision(), 0);
    assert_eq!(fs::read(f.config.join(LIVE[0])).unwrap(), bytes);
    assert!(same_member(
        &metadata,
        &fs::metadata(f.config.join(LIVE[0])).unwrap()
    ));
    assert_history(&f, &old);
    // No recapture/new context repairs the foreign call: the original exact
    // retained evidence and already prepared job must still complete normally.
    original
        .complete_subscription_batch_research(original_job, || 20, &mut evidence)
        .unwrap();
    assert_eq!(status(&original, "original")["state"], "succeeded");
    assert_eq!(original.revision(), 1);
    assert_history(&f, &old);
}

#[test]
fn historical_batch_cancel_abort_shutdown_never_write_and_stale_ticket_cannot_revoke() {
    for stage in ["before", "prepared", "abort", "shutdown"] {
        let (f, mut owner, mut context) = fixture(true, Some("https://fixed.invalid/feed"));
        let bytes = fs::read(f.config.join(LIVE[0])).unwrap();
        let mut job = start(&mut owner, &mut context, "cancel");
        let ticket = job.supervisor_ticket();
        if stage != "before" {
            fetch(&f, &mut job);
        }
        if stage == "abort" {
            owner
                .abort_subscription_batch_research(ticket.clone(), &mut context)
                .unwrap();
            let mut next = start(&mut owner, &mut context, "successor");
            assert!(
                owner
                    .abort_subscription_batch_research(ticket, &mut context)
                    .is_err()
            );
            fetch(&f, &mut next);
            owner
                .complete_subscription_batch_research(next, || 20, &mut context)
                .unwrap();
            assert_eq!(status(&owner, "successor")["state"], "succeeded");
            continue;
        } else if stage == "shutdown" {
            owner.stop_batch_operations().unwrap();
        } else {
            assert!(
                owner
                    .cancel_subscription_batch(&request("operations.cancel", "cancel"))
                    .unwrap()
            );
        }
        assert!(
            owner
                .complete_subscription_batch_research(
                    job,
                    || panic!("revoked batch cannot read clock"),
                    &mut context
                )
                .is_err()
        );
        assert_eq!(owner.revision(), 0);
        assert_eq!(fs::read(f.config.join(LIVE[0])).unwrap(), bytes);
        assert_eq!(
            status(&owner, "cancel")["state"],
            if stage == "shutdown" {
                "failed"
            } else {
                "cancelled"
            }
        );
    }
}

#[test]
fn historical_batch_late_original_evidence_drift_refuses_without_rebase() {
    for kind in [
        "c1",
        "transient",
        "receipt",
        "manager",
        "inode",
        "owner",
        "ownership",
    ] {
        let (f, mut owner, mut context) = fixture(true, Some("https://fixed.invalid/feed"));
        let mut job = start(&mut owner, &mut context, "drift");
        fetch(&f, &mut job);
        let path = f.config.join(LIVE[0]);
        let bytes = fs::read(&path).unwrap();
        match kind {
            "c1" => {
                fs::write(f.paths.state_directory.join(CLOSURE_MEMBER), b"changed").unwrap();
            }
            "transient" => {
                fs::write(f.paths.state_directory.join(NEXT_CLOSURE_MEMBER), b"late").unwrap();
            }
            "receipt" | "manager" => {
                fs::write(
                    f.paths.runtime_base.join("omavless-login.receipt"),
                    b"old-manager-or-torn",
                )
                .unwrap();
            }
            "inode" => {
                let replacement = f.config.join("fixed-next");
                fs::write(&replacement, &bytes).unwrap();
                fs::set_permissions(&replacement, fs::Permissions::from_mode(0o600)).unwrap();
                fs::rename(replacement, &path).unwrap();
            }
            "owner" => {
                let mut successor = new_owner(&f);
                assert!(
                    successor
                        .complete_subscription_batch_research(
                            job,
                            || panic!("wrong owner clock"),
                            &mut context
                        )
                        .is_err()
                );
                continue;
            }
            "ownership" => {
                fs::write(
                    f.paths
                        .state_directory
                        .join(crate::cutover::OWNERSHIP_MARKER_NAME),
                    b"changed",
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            owner
                .complete_subscription_batch_research(
                    job,
                    || panic!("late evidence cannot read clock"),
                    &mut context
                )
                .is_err()
        );
        assert_eq!(owner.revision(), 0);
        assert_eq!(status(&owner, "drift")["state"], "failed");
        assert_eq!(fs::read(path).unwrap(), bytes);
        assert!(context.current.is_none());
    }
}

#[test]
fn historical_batch_true_manager_change_and_stale_revision_refuse_before_clock() {
    let (f, lock) = prepared(true);
    ordinary_edit(&f);
    receipt(&f);
    let mut owner = new_owner(&f);
    let changed = Arc::new(AtomicBool::new(false));
    let flag = changed.clone();
    let epoch_proof = crate::login_activation::epoch_candidate::CurrentEpochProof::synthetic(
        &f.paths,
        f.uid,
        2,
        &lock,
        move |_| {
            Ok(if flag.load(Ordering::SeqCst) {
                "22222222222222222222222222222222"
            } else {
                "11111111111111111111111111111111"
            }
            .into())
        },
    )
    .unwrap();
    let mut context = owner
        .detach_batch_research(witness(&f, &lock).research(epoch_proof, || true).unwrap())
        .unwrap();
    drop(lock);
    let job = start(&mut owner, &mut context, "manager");
    changed.store(true, Ordering::SeqCst);
    assert!(
        owner
            .complete_subscription_batch_research(
                job,
                || panic!("changed epoch clock"),
                &mut context
            )
            .is_err()
    );
    assert_eq!(owner.revision(), 0);
    assert!(context.current.is_none());
    // A real synchronous historical favorite mutates revision/store while detached.
    let (f, mut owner, mut context) = fixture(true, Some("https://fixed.invalid/feed"));
    let mut job = start(&mut owner, &mut context, "stale");
    fetch(&f, &mut job);
    let lease = f.lock();
    let mut favorite = witness(&f, &lease)
        .research(proof(&f, &lease), || true)
        .unwrap()
        .into_profile()
        .unwrap();
    let store: Value = serde_json::from_slice(&fs::read(f.config.join(LIVE[0])).unwrap()).unwrap();
    owner.execute_profile_research(&json!({"api":"omavless.control","version":1,"id":"fixed","method":"profiles.favorite","params":{"profileId":store["profiles"][0]["id"],"enabled":!store["profiles"][0]["favorite"].as_bool().unwrap_or(false),"operationId":"edit","expectedRevision":0}}),&mut favorite).unwrap();
    drop(favorite);
    drop(lease);
    assert_eq!(owner.revision(), 1);
    assert!(
        owner
            .complete_subscription_batch_research(job, || panic!("stale clock"), &mut context)
            .is_err()
    );
    assert_eq!(owner.revision(), 1);
    assert_eq!(status(&owner, "stale")["state"], "failed");
}

#[test]
fn historical_batch_before_after_real_writer_fences_poison_and_latch() {
    for checkpoint in [
        BatchWriteCheckpoint::Before,
        BatchWriteCheckpoint::AfterWriterBeforePin,
        BatchWriteCheckpoint::After,
    ] {
        for kind in ["transient", "same-inode-bytes", "lost-success"] {
            let (f, mut owner, mut context) = fixture(true, Some("https://fixed.invalid/feed"));
            let old = fs::read(f.config.join(LIVE[0])).unwrap();
            let mut job = start(&mut owner, &mut context, "late");
            fetch(&f, &mut job);
            let state = f.paths.state_directory.clone();
            let path = f.config.join(LIVE[0]);
            context.fault = Some(Box::new(move |at| {
                if at == checkpoint {
                    match kind {
                        "transient" => {
                            fs::write(state.join(NEXT_CLOSURE_MEMBER), b"late").unwrap();
                        }
                        "same-inode-bytes" => {
                            let bytes = fs::read(&path).unwrap();
                            let next = path.with_file_name("fixed-next");
                            fs::write(&next, bytes).unwrap();
                            fs::set_permissions(&next, fs::Permissions::from_mode(0o600)).unwrap();
                            fs::rename(next, &path).unwrap();
                        }
                        "lost-success" => return Err(()),
                        _ => unreachable!(),
                    }
                }
                Ok(())
            }));
            assert!(
                owner
                    .complete_subscription_batch_research(job, || 20, &mut context)
                    .is_err()
            );
            assert!(context.current.is_none());
            assert_eq!(owner.revision(), 0);
            assert_eq!(status(&owner, "late")["state"], "failed");
            let bytes = fs::read(f.config.join(LIVE[0])).unwrap();
            assert_eq!(bytes == old, checkpoint == BatchWriteCheckpoint::Before);
            context.fault = None;
            assert!(
                owner
                    .start_subscription_batch_research(
                        &request("subscriptions.refresh_all", "retry"),
                        &mut context
                    )
                    .is_err()
            );
            assert_eq!(fs::read(f.config.join(LIVE[0])).unwrap(), bytes);
            if kind == "transient" {
                fs::remove_file(f.paths.state_directory.join(NEXT_CLOSURE_MEMBER)).unwrap();
            }
            // A freshly acquired same-manager context cannot erase the owner
            // latch even when current bytes are now semantically valid.
            let lease = f.lock();
            let mut fresh = owner
                .detach_batch_research(
                    witness(&f, &lease)
                        .research(proof(&f, &lease), || true)
                        .unwrap(),
                )
                .unwrap();
            drop(lease);
            assert!(
                owner
                    .start_subscription_batch_research(
                        &request("subscriptions.refresh_all", "fresh"),
                        &mut fresh
                    )
                    .is_err()
            );
        }
    }
}

#[test]
fn historical_batch_actual_loopback_transport_fetch_has_no_migration_lease() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!(
        "http://127.0.0.1:{}/fixed",
        listener.local_addr().unwrap().port()
    );
    let (f, mut owner, mut context) = fixture(true, Some(&url));
    let paths = f.paths.clone();
    let uid = f.uid;
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let mut request = [0_u8; 2048];
        assert!(stream.read(&mut request).unwrap() > 0);
        let lease = MigrationLock::acquire(&paths, uid).unwrap();
        drop(lease);
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            URI.len(),
            URI
        )
        .unwrap();
    });
    let mut job = start(&mut owner, &mut context, "loopback");
    assert_eq!(
        job.step(
            &HttpsSubscriptionTransport::new(),
            &crate::remote_fetch::RemoteFetchPool::default(),
            &mut || PROFILE.into()
        )
        .unwrap(),
        BatchWorkStep::Ready
    );
    server.join().unwrap();
    owner
        .complete_subscription_batch_research(job, || 20, &mut context)
        .unwrap();
    assert_eq!(owner.revision(), 1);
    assert_eq!(status(&owner, "loopback")["state"], "succeeded");
}

#[test]
fn historical_batch_actual_fixed_writer_slot_exhaustion_keeps_old_bytes_but_latches() {
    let (f, mut owner, mut context) = fixture(true, Some("https://fixed.invalid/feed"));
    let path = f.config.join(LIVE[0]);
    let bytes = fs::read(&path).unwrap();
    let metadata = fs::metadata(&path).unwrap();
    let mut job = start(&mut owner, &mut context, "exhaustion");
    fetch(&f, &mut job);
    let directory = f.config.clone();
    context.fault = Some(Box::new(move |at| {
        if at == BatchWriteCheckpoint::Before {
            for nonce in 0..128 {
                let path = directory.join(format!(".profiles.json.{}.{nonce}", std::process::id()));
                fs::write(&path, b"fixed-owned-slot").unwrap();
                fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
            }
        }
        Ok(())
    }));
    assert!(
        owner
            .complete_subscription_batch_research(job, || 20, &mut context)
            .is_err()
    );
    assert_eq!(status(&owner, "exhaustion")["state"], "failed");
    assert_eq!(owner.revision(), 0);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert!(same_member(&metadata, &fs::metadata(&path).unwrap()));
    assert!(context.current.is_none());
    for nonce in 0..128 {
        fs::remove_file(
            f.config
                .join(format!(".profiles.json.{}.{nonce}", std::process::id())),
        )
        .unwrap();
    }
    let lease = f.lock();
    let mut fresh = owner
        .detach_batch_research(
            witness(&f, &lease)
                .research(proof(&f, &lease), || true)
                .unwrap(),
        )
        .unwrap();
    drop(lease);
    assert!(
        owner
            .start_subscription_batch_research(
                &request("subscriptions.refresh_all", "matching-old-bytes"),
                &mut fresh
            )
            .is_err()
    );
}
