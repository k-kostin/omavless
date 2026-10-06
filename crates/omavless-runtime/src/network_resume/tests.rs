// SPDX-License-Identifier: MIT
use super::*;

#[test]
fn shared_startup_and_event_paths_use_one_owner_attempt_in_either_order() {
    let mut startup_first = Fixture::new();
    assert_eq!(startup_first.startup(2), 0);
    assert_eq!(startup_first.coordinator.host().observations, 0);
    assert_eq!(startup_first.startup(3), 1);
    assert_eq!(startup_first.phase(), Phase::Finished);
    startup_first.send(Kind::Resume, 4);
    assert_eq!(startup_first.poll(7, false, false), 0);
    assert_eq!(startup_first.startup(8), 0);
    assert_eq!(
        startup_first.coordinator.host().changes,
        ["prepare", "start", "commit"]
    );
    let mut event_first = Fixture::new();
    event_first.send(Kind::Resume, 10);
    assert_eq!(event_first.startup(11), 0); // queued hint retains its own quiet fence
    assert_eq!(event_first.coordinator.host().observations, 0);
    assert_eq!(event_first.poll(13, false, false), 1);
    assert_eq!(event_first.startup(14), 0);
    assert_eq!(
        event_first.coordinator.host().changes,
        ["prepare", "start", "commit"]
    );
}

#[test]
fn both_legacy_owner_entries_and_lowest_lifecycle_startup_refuse_installed_guard() {
    use crate::connection_transaction::ConnectionTransactionError;
    use crate::lifecycle::LifecycleError;
    let mut fixture = Fixture::new();
    assert_eq!(
        fixture.coordinator.reconcile_startup(),
        Err(ConnectionTransactionError::ManualRecoveryRequired)
    );
    let lease = MigrationLock::acquire(&fixture.cutover, fixture.uid).unwrap();
    assert_eq!(
        fixture.coordinator.reconcile_startup_locked(&lease),
        Err(ConnectionTransactionError::ManualRecoveryRequired)
    );
    assert_eq!(
        fixture.coordinator.resume_lowest_startup(),
        Err(LifecycleError::ManualRecoveryRequired)
    );
    drop(lease);
    fixture.unchanged();
    assert_eq!(fixture.coordinator.host().observations, 0);
    assert_eq!(fixture.startup(3), 1); // refusals did not create a second path or consume the proper path
}

#[test]
fn guard_cannot_be_installed_after_an_unrestricted_startup_already_entered() {
    let mut fixture = Fixture::with_guard(true, false);
    fixture.coordinator.reconcile_startup().unwrap();
    let calls = fixture.coordinator.host().changes.clone();
    assert!(
        fixture
            .coordinator
            .install_resume_barrier(BOOT, INSTANCE, 5, 0)
            .is_err()
    );
    assert!(fixture.coordinator.reconcile_startup().is_err());
    assert!(fixture.coordinator.resume_lowest_startup().is_err());
    assert_eq!(fixture.startup(3), 0);
    assert_eq!(fixture.coordinator.host().changes, calls);
}

#[test]
fn failed_startup_reservation_before_publication_blocks_events_and_reinstallation() {
    let mut fixture = Fixture::new();
    assert_eq!(
        fixture.coordinator.resume_startup(
            &mut fixture.source,
            3,
            true,
            false,
            Some(Phase::Reserved)
        ),
        0
    );
    assert_eq!(fixture.phase(), Phase::Ready);
    assert_eq!(fixture.status(), Status::ManualRecovery);
    assert!(
        fixture
            .coordinator
            .install_resume_barrier(BOOT, INSTANCE, 5, 4)
            .is_err()
    );
    let observations = fixture.coordinator.host().observations;
    fixture.send(Kind::Resume, 4);
    assert_eq!(fixture.poll(7, false, false), 0);
    assert_eq!(fixture.startup(8), 0);
    assert_eq!(fixture.coordinator.host().observations, observations);
    fixture.unchanged();
}

#[test]
fn failed_startup_effect_and_completion_uncertainty_never_reopen_the_event_path() {
    for start_failure in [true, false] {
        let mut fixture = Fixture::new();
        fixture.coordinator.host_mut().fail_start = start_failure;
        let dispatched = fixture.coordinator.resume_startup(
            &mut fixture.source,
            3,
            !start_failure,
            false,
            if start_failure {
                None
            } else {
                Some(Phase::Finished)
            },
        );
        assert_eq!(dispatched, 1);
        assert_eq!(fixture.phase(), Phase::Reserved);
        assert_eq!(fixture.status(), Status::ManualRecovery);
        let calls = fixture.coordinator.host().changes.clone();
        fixture.send(Kind::Resume, 4);
        assert_eq!(fixture.poll(7, false, false), 0);
        assert_eq!(fixture.startup(8), 0);
        assert_eq!(fixture.coordinator.host().changes, calls);
    }
}

#[test]
fn event_source_loss_gap_and_time_regression_share_permanent_startup_refusal() {
    for failure in 0..3 {
        let mut fixture = Fixture::new();
        fixture.send(Kind::Resume, 10);
        match failure {
            0 => {
                fixture.writer.shutdown(std::net::Shutdown::Write).unwrap();
                fixture.receive(11);
            }
            1 => fixture.send_sequence(Kind::Resume, 11, 3),
            _ => {
                assert_eq!(fixture.poll(9, false, false), 0);
            }
        }
        assert_eq!(fixture.status(), Status::SourceUnavailable);
        assert_eq!(fixture.startup(14), 0);
        assert!(
            fixture
                .coordinator
                .install_resume_barrier(BOOT, INSTANCE, 5, 15)
                .is_err()
        );
        fixture.unchanged();
        assert_eq!(fixture.phase(), Phase::Ready);
    }
}

#[test]
fn panic_during_extracted_event_guard_leaves_owner_inflight_never_absent() {
    let mut fixture = Fixture::new();
    fixture.send(Kind::Resume, 10);
    fixture.coordinator.host_mut().panic_observation_at = Some(1);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || fixture.poll(13, false, false)
        ))
        .is_err()
    );
    assert_eq!(fixture.phase(), Phase::Ready);
    assert_eq!(fixture.status(), Status::ManualRecovery);
    assert!(
        fixture
            .coordinator
            .install_resume_barrier(BOOT, INSTANCE, 5, 14)
            .is_err()
    );
    assert_eq!(fixture.startup(15), 0);
    assert!(fixture.coordinator.reconcile_startup().is_err());
    assert!(fixture.coordinator.resume_lowest_startup().is_err());
    fixture.unchanged();
}

#[test]
fn restart_existing_ready_reserved_finished_never_provisions_or_adopts_old_permit() {
    for phase in [Phase::Ready, Phase::Reserved, Phase::Finished] {
        let mut fixture = Fixture::new();
        let receipt = Receipt {
            schema: 1,
            fence: fixture.context.fence,
            phase,
        };
        atomic_replace_private(
            &fixture.receipt,
            &serde_json::to_vec(&receipt).unwrap(),
            fixture.uid,
        )
        .unwrap();
        fixture.restart([3; 16]);
        let (reader, writer) = UnixStream::pair().unwrap();
        fixture.writer = writer;
        fixture.source =
            Source::owned(reader, std::process::id(), fixture.uid, &fixture.context).unwrap();
        assert_eq!(fixture.startup(3), 0);
        fixture.send(Kind::Resume, 4);
        assert_eq!(fixture.poll(7, false, false), 0);
        assert_eq!(fixture.phase(), phase);
        fixture.unchanged();
        assert_eq!(fixture.coordinator.host().observations, 0);
    }
}

#[test]
fn healthy_second_observation_becoming_empty_cannot_hide_recovery_or_pointer_repair() {
    let mut fixture = Fixture::new();
    fixture.coordinator.host_mut().observed = healthy();
    fixture.coordinator.host_mut().change_observation_at = Some((2, empty()));
    let original = fs::read(fixture.root.join("profiles.json")).unwrap();
    assert_eq!(fixture.startup(3), 0);
    assert_eq!(fixture.status(), Status::ManualRecovery);
    assert_eq!(
        fs::read(fixture.root.join("profiles.json")).unwrap(),
        original
    );
    assert_eq!(fixture.phase(), Phase::Ready);
    fixture.unchanged();
}

#[test]
fn healthy_and_settled_off_startup_are_narrow_observation_only() {
    for connected in [true, false] {
        let mut fixture = Fixture::with_intent(connected);
        fixture.coordinator.host_mut().observed = if connected { healthy() } else { empty() };
        let original = fs::read(fixture.root.join("profiles.json")).unwrap();
        assert_eq!(fixture.startup(3), 0);
        assert_eq!(fixture.status(), Status::ObserveOnly);
        assert_eq!(fixture.phase(), Phase::Ready);
        assert_eq!(fixture.coordinator.revision(), 0);
        assert_eq!(
            fs::read(fixture.root.join("profiles.json")).unwrap(),
            original
        );
        fixture.unchanged();
    }
    let mut residual = Fixture::with_intent(false);
    residual.coordinator.host_mut().observed = healthy();
    assert_eq!(residual.startup(3), 0);
    assert_eq!(residual.status(), Status::ManualRecovery);
    residual.unchanged();
}

#[test]
fn last_lifecycle_observation_intent_change_refuses_before_prepare() {
    for startup in [false, true] {
        let mut fixture = Fixture::new();
        let mut off = fixture.context.desired.clone();
        off.connected = false;
        off.profile_id.clear();
        fixture.coordinator.host_mut().change_after_observation =
            Some((4, fixture.desired.clone(), fixture.uid, off));
        if startup {
            fixture.startup(3);
        } else {
            fixture.send(Kind::Resume, 10);
            fixture.poll(13, false, false);
        }
        assert_eq!(fixture.status(), Status::ManualRecovery);
        assert_eq!(fixture.phase(), Phase::Reserved);
        assert!(fixture.coordinator.host().changes.is_empty());
        assert!(
            !read_desired(&fixture.desired, fixture.uid)
                .unwrap()
                .connected
        );
        assert_eq!(fixture.startup(14), 0);
    }
}

#[test]
fn startup_enrollment_quiet_deadline_and_all_target_fences_refuse_without_effects() {
    for fence in 0..8 {
        let mut fixture = Fixture::new();
        match fence {
            0 => {
                let mut target = fixture.context.desired.clone();
                target.mode = RoutingMode::Global;
                write_desired(&fixture.desired, fixture.uid, &target).unwrap();
            }
            1 => {
                let mut target = fixture.context.desired.clone();
                target.generation += 1;
                write_desired(&fixture.desired, fixture.uid, &target).unwrap();
            }
            2 => {
                let mut target = fixture.context.desired.clone();
                target.profile_id = "22222222-2222-4222-8222-222222222222".into();
                write_desired(&fixture.desired, fixture.uid, &target).unwrap();
            }
            3 => fixture.coordinator.resume_fixture_revision_change(),
            4 => {
                let path = fixture.root.join("profiles.json");
                let mut store: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                store["profiles"][0]["name"] = json!("Changed");
                atomic_replace_private(&path, &serde_json::to_vec(&store).unwrap(), fixture.uid)
                    .unwrap();
            }
            5 => {
                atomic_replace_private(
                    &fixture.cutover.ownership_marker,
                    br#"{"schemaVersion":1,"generation":8,"phase":"rust"}"#,
                    fixture.uid,
                )
                .unwrap();
            }
            6 => fixture.source.boot = [9; 16],
            _ => {
                fs::remove_file(&fixture.receipt).unwrap();
            }
        }
        assert_eq!(fixture.startup(3), 0);
        assert_eq!(fixture.coordinator.host().observations, 0);
        assert!(fixture.coordinator.host().changes.is_empty());
        assert_eq!(fixture.startup(4), 0);
        if fence != 7 {
            assert_eq!(fixture.phase(), Phase::Ready);
        }
    }
    let mut deadline = Fixture::new();
    assert_eq!(deadline.startup(61), 0);
    deadline.unchanged();
    assert_eq!(deadline.startup(3), 0); // cannot regain eligibility by moving time backwards
    let mut quiet = Fixture::new();
    assert_eq!(quiet.startup(0), 0);
    assert_eq!(quiet.startup(2), 0);
    quiet.unchanged();
    assert_eq!(quiet.coordinator.host().observations, 0);
    assert_eq!(quiet.startup(3), 1);
}

#[test]
fn healthy_last_observer_off_change_refuses_without_cache_adoption_or_repair() {
    let mut fixture = Fixture::new();
    fixture.coordinator.host_mut().observed = healthy();
    let mut off = fixture.context.desired.clone();
    off.connected = false;
    off.profile_id.clear();
    fixture.coordinator.host_mut().change_after_observation =
        Some((2, fixture.desired.clone(), fixture.uid, off));
    let store = fs::read(fixture.root.join("profiles.json")).unwrap();
    assert_eq!(fixture.startup(3), 0);
    assert_eq!(fixture.status(), Status::ManualRecovery);
    assert_eq!(
        fixture.coordinator.actual(),
        crate::lifecycle::ActualState::Disconnected
    );
    assert_eq!(fs::read(fixture.root.join("profiles.json")).unwrap(), store);
    assert!(fixture.coordinator.host().changes.is_empty());
}

#[test]
fn whole_frame_deadline_stops_a_slow_source_even_when_each_byte_arrives_in_time() {
    let mut fixture = Fixture::new();
    let mut writer = fixture.writer.try_clone().unwrap();
    let sender = std::thread::spawn(move || {
        // Every individual gap is below the old 100-ms per-read timeout. The
        // complete malformed frame takes over half a second without a whole
        // frame deadline. Only this owned Unix pair is touched.
        for byte in b"{abcdefgh\n" {
            if writer.write_all(&[*byte]).is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(75));
        }
    });
    let before = Instant::now();
    fixture.receive(10);
    let elapsed = before.elapsed();
    assert_eq!(fixture.status(), Status::SourceUnavailable);
    assert!(elapsed < Duration::from_millis(400));
    assert_eq!(fixture.poll(13, false, false), 0);
    fixture.unchanged();
    sender.join().unwrap();
}

#[cfg(feature = "network-resume-fixture")]
#[test]
fn fixed_developer_entry_returns_only_the_owned_fixture_projection() {
    assert_eq!(
        run_developer_fixture(),
        json!({"schema":1,"scope":"owned_fixture",
        "state":"recovered","effectCalls":1,"revision":1,"intentPreserved":true})
    );
}

#[test]
fn final_newline_return_after_deadline_never_admits_an_event() {
    let mut fixture = Fixture::new();
    // Whole authentic frame is available immediately. Simulate the reader's
    // scheduling pause after its final read, before accepting that newline.
    fixture.source.after_newline_pause = Duration::from_millis(120);
    fixture.send(Kind::Resume, 10);
    assert_eq!(fixture.status(), Status::SourceUnavailable);
    assert_eq!(fixture.poll(13, false, false), 0);
    assert_eq!(fixture.coordinator.host().observations, 0);
    assert_eq!(fixture.phase(), Phase::Ready);
    fixture.unchanged();
}

#[test]
fn authenticated_source_runs_original_coordinator_once_after_a_quiet_burst() {
    let mut f = Fixture::new();
    f.send(Kind::Resume, 100);
    f.send(Kind::NetworkChanged, 101);
    f.send(Kind::NetworkChanged, 102);
    assert_eq!(f.poll(104, false, false), 0);
    f.unchanged();
    assert_eq!(f.coordinator.host().observations, 0);
    assert_eq!(f.poll(105, false, false), 1);
    assert_eq!(f.status(), Status::Recovered);
    assert_eq!(f.phase(), Phase::Finished);
    assert_eq!(f.coordinator.host().changes, ["prepare", "start", "commit"]);
    assert_eq!(f.coordinator.revision(), 1);
    assert!(read_desired(&f.desired, f.uid).unwrap() == f.context.desired);
    let pointers: serde_json::Value =
        serde_json::from_slice(&fs::read(f.root.join("profiles.json")).unwrap()).unwrap();
    assert_eq!(pointers["activeId"], PROFILE);
    assert_eq!(pointers["lastId"], PROFILE);
    f.send(Kind::Resume, 106);
    assert_eq!(f.poll(109, false, false), 0);
    assert_eq!(f.coordinator.host().changes.len(), 3);
}

#[test]
fn pause_ignores_link_notifications_until_resume_without_disconnect() {
    let mut f = Fixture::new();
    f.send(Kind::NetworkChanged, 10);
    f.send(Kind::Suspend, 11);
    f.send(Kind::NetworkChanged, 12);
    assert_eq!(f.poll(15, false, false), 0);
    assert_eq!(f.status(), Status::Paused);
    f.unchanged();
    f.send(Kind::Resume, 20);
    assert_eq!(f.poll(23, false, false), 1);
}

#[test]
fn healthy_never_restarts_and_incomplete_foreign_or_unsafe_facts_never_connect() {
    for case in 0..7 {
        let mut f = Fixture::new();
        match case {
            0 => f.coordinator.host_mut().observed = healthy(),
            1 => f.coordinator.host_mut().unavailable = true,
            2 => f.coordinator.host_mut().observed.core_count = 1,
            3 => f.coordinator.host_mut().observed.tun_count = 1,
            4 => f.coordinator.host_mut().safe = false,
            5 => {
                f.coordinator.host_mut().observed = OwnedObservation {
                    core_count: 2,
                    ..healthy()
                }
            }
            _ => {
                f.coordinator.host_mut().observed = OwnedObservation {
                    active_profile_matches: false,
                    ..healthy()
                }
            }
        }
        f.send(Kind::Resume, 10);
        assert_eq!(f.poll(13, false, false), 0);
        f.unchanged();
        assert_eq!(f.phase(), Phase::Ready);
        assert_eq!(
            f.status(),
            if case == 0 {
                Status::ObserveOnly
            } else {
                Status::ManualRecovery
            }
        );
    }
}

#[test]
fn every_desired_change_and_off_cancel_before_any_host_effect() {
    for case in 0..5 {
        let mut f = Fixture::new();
        f.send(Kind::Resume, 10);
        let mut changed = f.context.desired.clone();
        match case {
            0 => {
                changed.connected = false;
                changed.profile_id.clear();
            } // ManualOff/Disconnect/Quit intent
            1 => changed.mode = RoutingMode::Global,
            2 => changed.profile_id = "22222222-2222-4222-8222-222222222222".into(),
            3 => changed.generation += 1,
            _ => {
                let mut marker = fs::read(&f.cutover.ownership_marker).unwrap();
                marker = String::from_utf8(marker)
                    .unwrap()
                    .replace("\"generation\":7", "\"generation\":8")
                    .into_bytes();
                atomic_replace_private(&f.cutover.ownership_marker, &marker, f.uid).unwrap();
            }
        }
        if case != 4 {
            write_desired(&f.desired, f.uid, &changed).unwrap();
        }
        assert_eq!(f.poll(13, false, false), 0);
        assert!(f.coordinator.host().changes.is_empty());
        assert_eq!(f.coordinator.host().observations, 0);
        assert_eq!(f.phase(), Phase::Ready);
        assert_eq!(
            f.status(),
            if matches!(case, 0 | 1 | 3) {
                Status::Cancelled
            } else {
                Status::ManualRecovery
            }
        );
        assert_eq!(f.startup(14), 0);
    }
    let mut f = Fixture::new();
    let mut off = f.context.desired.clone();
    off.connected = false;
    off.profile_id.clear();
    write_desired(&f.desired, f.uid, &off).unwrap();
    f.context = f.coordinator.resume_context(BOOT, INSTANCE, 5).unwrap();
    f.send(Kind::Resume, 10);
    assert_eq!(f.status(), Status::Cancelled);
    assert_eq!(f.poll(13, false, false), 0);
    assert!(f.coordinator.host().changes.is_empty());
}

#[test]
fn lost_gapped_bad_or_wrong_boot_source_is_terminal() {
    for case in 0..7 {
        let mut f = Fixture::new();
        f.send(Kind::Resume, 10);
        match case {
            0 => {
                f.writer.shutdown(std::net::Shutdown::Write).unwrap();
            }
            1 => {
                f.writer
                    .write_all(b"{\"sequence\":3,\"kind\":\"Resume\"}\n")
                    .unwrap();
            }
            2 => {
                f.writer
                    .write_all(b"{\"sequence\":2,\"sequence\":2,\"kind\":\"Resume\"}\n")
                    .unwrap();
            }
            3 => {
                f.writer.write_all(&[b'x'; 257]).unwrap();
            }
            4 => {
                f.source.boot = [9; 16];
            }
            5 => {
                f.source.instance = [9; 16];
            }
            _ => {} // timeout is also unavailable, never fabricated absence
        }
        f.receive(11);
        assert_eq!(f.status(), Status::SourceUnavailable);
        assert_eq!(f.poll(14, false, false), 0);
        f.unchanged();
        assert_eq!(f.phase(), Phase::Ready);
    }
}

#[test]
fn duplicate_reordered_events_clock_reversal_and_endless_burst_cannot_extend_permit() {
    let mut f = Fixture::new();
    f.send(Kind::Resume, 10);
    f.send_sequence(Kind::NetworkChanged, 11, 1);
    assert_eq!(f.coordinator.resume_pending_tick().unwrap(), 10);
    assert_eq!(f.poll(13, false, false), 1);
    let mut f = Fixture::new();
    f.send(Kind::Resume, 10);
    assert_eq!(f.poll(9, false, false), 0);
    assert_eq!(f.status(), Status::SourceUnavailable);
    f.unchanged();
    let mut f = Fixture::new();
    for tick in 0..=61 {
        f.send(Kind::NetworkChanged, tick);
    }
    assert_eq!(f.poll(64, false, false), 0);
    assert_eq!(f.status(), Status::Cancelled);
    f.unchanged();
}

#[test]
fn failed_or_unacknowledged_reservation_and_failed_recovery_never_rearm() {
    for (before, after, failed_start, failed_stop) in [
        (true, false, false, false),
        (false, true, false, false),
        (false, false, true, false),
        (false, false, true, true),
    ] {
        let mut f = Fixture::new();
        f.coordinator.host_mut().fail_start = failed_start;
        f.coordinator.host_mut().fail_stop = failed_stop;
        f.send(Kind::Resume, 10);
        assert_eq!(f.poll(13, before, after), usize::from(failed_start));
        assert_eq!(f.status(), Status::ManualRecovery);
        assert_eq!(
            f.phase(),
            if before {
                Phase::Ready
            } else {
                Phase::Reserved
            }
        );
        let changes = f.coordinator.host().changes.len();
        f.send(Kind::Resume, 14);
        assert_eq!(f.poll(17, false, false), 0);
        assert_eq!(f.startup(18), 0);
        assert_eq!(f.coordinator.host().changes.len(), changes);
        // New runtime identity cannot adopt the old Ready, even when the
        // attempted reservation failed before any file was changed.
        f.context.fence.owner_instance = [3; 16];
        f.restart([3; 16]);
        let (reader, writer) = UnixStream::pair().unwrap();
        f.writer = writer;
        f.sequence = 0;
        f.source = Source::owned(reader, std::process::id(), f.uid, &f.context).unwrap();
        f.send(Kind::Resume, 20);
        assert_eq!(f.poll(23, false, false), 0);
        assert!(f.coordinator.host().changes.is_empty());
    }
}

#[test]
fn intent_race_after_reservation_burns_receipt_with_zero_recovery() {
    let mut f = Fixture::new();
    let mut off = f.context.desired.clone();
    off.connected = false;
    off.profile_id.clear();
    // Third observation is post-reservation revalidation. The following effect
    // boundary rereads exact desired state under the same original lease.
    f.coordinator.host_mut().change_after_observation = Some((3, f.desired.clone(), f.uid, off));
    f.send(Kind::Resume, 10);
    assert_eq!(f.poll(13, false, false), 0);
    assert_eq!(f.status(), Status::ManualRecovery);
    assert_eq!(f.phase(), Phase::Reserved);
    assert!(f.coordinator.host().changes.is_empty());
}

#[test]
fn fixture_peer_credentials_and_lost_or_unsafe_receipts_fail_closed() {
    let f = Fixture::new();
    let (reader, _) = UnixStream::pair().unwrap();
    assert!(Source::owned(reader, std::process::id() + 1, f.uid, &f.context).is_err());
    for case in 0..5 {
        let mut f = Fixture::new();
        match case {
            0 => fs::remove_file(&f.receipt).unwrap(),
            1 => atomic_replace_private(&f.receipt, b"{}", f.uid).unwrap(),
            2 => fs::set_permissions(&f.receipt, fs::Permissions::from_mode(0o644)).unwrap(),
            3 => {
                fs::remove_file(&f.receipt).unwrap();
                std::os::unix::fs::symlink(f.root.join("profiles.json"), &f.receipt).unwrap();
            }
            _ => {
                let receipt = Receipt {
                    schema: 1,
                    phase: Phase::Reserved,
                    fence: f.context.fence,
                };
                atomic_replace_private(&f.receipt, &serde_json::to_vec(&receipt).unwrap(), f.uid)
                    .unwrap();
            }
        }
        f.send(Kind::Resume, 10);
        assert_eq!(f.poll(13, false, false), 0);
        f.unchanged();
        assert_eq!(f.status(), Status::ManualRecovery);
    }
}

#[test]
fn original_mutation_lease_contention_never_turns_into_a_queued_connect() {
    let mut f = Fixture::new();
    f.send(Kind::Resume, 10);
    let lease = MigrationLock::acquire(&f.cutover, f.uid).unwrap();
    assert_eq!(f.poll(13, false, false), 0);
    assert_eq!(f.status(), Status::ManualRecovery);
    drop(lease);
    f.send(Kind::Resume, 14);
    assert_eq!(f.poll(17, false, false), 0);
    f.unchanged();
}

#[test]
fn disconnect_through_original_owner_wins_and_receipt_cannot_reconnect() {
    use crate::mutation::MutationDigest;
    use crate::owner::{OwnerAction, OwnerRequest};
    let mut f = Fixture::new();
    f.send(Kind::Resume, 10);
    let result = f
        .coordinator
        .execute_connection(OwnerRequest::new(
            OwnerAction::Disconnect,
            Some("fixture-disconnect"),
            Some(0),
            MutationDigest::new([7; 32]),
        ))
        .unwrap();
    assert!(matches!(
        result,
        crate::native_coordinator::NativeOwnerExecution::Applied { outcome: Ok(_), .. }
    ));
    assert_eq!(f.coordinator.revision(), 1);
    let changes = f.coordinator.host().changes.clone();
    assert_eq!(f.poll(13, false, false), 0);
    assert_eq!(f.coordinator.host().changes, changes);
    assert!(!read_desired(&f.desired, f.uid).unwrap().connected);
    assert_eq!(f.phase(), Phase::Ready);
}

#[test]
fn valid_changed_store_cancels_but_deleted_target_requires_manual_without_observation() {
    for delete in [false, true] {
        let mut f = Fixture::new();
        f.send(Kind::Resume, 10);
        let path = f.root.join("profiles.json");
        let mut store: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        if delete {
            store["profiles"] = json!([]);
            store["lastId"] = json!("");
        } else {
            store["profiles"][0]["name"] = json!("Changed");
        }
        atomic_replace_private(&path, &serde_json::to_vec(&store).unwrap(), f.uid).unwrap();
        assert_eq!(f.poll(13, false, false), 0);
        assert!(f.coordinator.host().changes.is_empty());
        assert_eq!(f.coordinator.host().observations, 0);
        assert_eq!(
            f.status(),
            if delete {
                Status::ManualRecovery
            } else {
                Status::Cancelled
            }
        );
        assert_eq!(f.phase(), Phase::Ready);
        assert_eq!(f.startup(14), 0);
    }
}

#[test]
fn valid_revision_change_cancels_without_observation_or_rearming() {
    let mut f = Fixture::new();
    f.send(Kind::Resume, 10);
    f.coordinator.resume_fixture_revision_change();
    assert_eq!(f.poll(13, false, false), 0);
    assert_eq!(f.status(), Status::Cancelled);
    assert_eq!(f.coordinator.host().observations, 0);
    assert!(f.coordinator.host().changes.is_empty());
    assert_eq!(f.phase(), Phase::Ready);
    f.send(Kind::Resume, 14);
    assert_eq!(f.poll(17, false, false), 0);
    assert_eq!(f.startup(18), 0);
}

#[test]
fn incomplete_invalid_io_and_owner_manual_fences_never_become_safe_cancellation() {
    for fault in 0..9 {
        let mut f = Fixture::new();
        f.send(Kind::Resume, 10);
        match fault {
            0 => fs::remove_file(&f.desired.file).unwrap(),
            1 => atomic_replace_private(&f.desired.file, b"invalid", f.uid).unwrap(),
            2 => fs::remove_file(f.root.join("profiles.json")).unwrap(),
            3 => atomic_replace_private(&f.root.join("profiles.json"), b"invalid", f.uid).unwrap(),
            4 => fs::set_permissions(&f.desired.file, fs::Permissions::from_mode(0o644)).unwrap(),
            5 => fs::set_permissions(
                f.root.join("profiles.json"),
                fs::Permissions::from_mode(0o644),
            )
            .unwrap(),
            6 => atomic_replace_private(
                &f.cutover.ownership_marker,
                br#"{"schemaVersion":1,"generation":8,"phase":"rust"}"#,
                f.uid,
            )
            .unwrap(),
            7 => atomic_replace_private(
                &f.cutover.ownership_marker,
                br#"{"schemaVersion":1,"generation":7,"phase":"legacy"}"#,
                f.uid,
            )
            .unwrap(),
            _ => f.coordinator.resume_fixture_manual_block(),
        }
        assert_eq!(f.poll(13, false, false), 0);
        assert_eq!(f.status(), Status::ManualRecovery);
        assert_eq!(f.coordinator.host().observations, 0);
        assert!(f.coordinator.host().changes.is_empty());
        assert_eq!(f.phase(), Phase::Ready);
        assert_eq!(f.startup(14), 0);
        // Restoration cannot re-create the consumed owner eligibility.
        if fault == 0 {
            write_desired(&f.desired, f.uid, &f.context.desired).unwrap();
            f.send(Kind::Resume, 15);
            assert_eq!(f.poll(18, false, false), 0);
        }
    }
}

#[test]
fn source_loss_or_unprocessed_new_event_at_due_tick_blocks_old_event() {
    for queued in [false, true] {
        let mut f = Fixture::new();
        f.send(Kind::Resume, 10);
        if queued {
            f.writer
                .write_all(b"{\"sequence\":2,\"kind\":\"Suspend\"}\n")
                .unwrap();
        } else {
            f.writer.shutdown(std::net::Shutdown::Write).unwrap();
        }
        assert_eq!(f.poll(13, false, false), 0);
        assert_eq!(f.status(), Status::SourceUnavailable);
        f.unchanged();
        assert_eq!(f.coordinator.host().observations, 0);
    }
}

#[test]
fn status_projection_is_coarse_and_contains_no_identity_or_target() {
    let mut f = Fixture::new();
    f.send(Kind::Resume, 10);
    assert_eq!(f.projection(), json!({"schema":1,"state":"checking"}));
    assert_eq!(f.poll(13, false, false), 1);
    assert_eq!(f.projection(), json!({"schema":1,"state":"recovered"}));
}

#[test]
fn completion_publication_failure_never_repeats_a_completed_coordinator_operation() {
    for after in [false, true] {
        let mut f = Fixture::new();
        f.send(Kind::Resume, 10);
        assert_eq!(f.poll_fault(13, !after, after, Some(Phase::Finished)), 1);
        assert_eq!(f.status(), Status::ManualRecovery);
        assert_eq!(
            f.phase(),
            if after {
                Phase::Finished
            } else {
                Phase::Reserved
            }
        );
        assert_eq!(f.coordinator.host().changes, ["prepare", "start", "commit"]);
        assert_eq!(f.coordinator.revision(), 1);
        f.send(Kind::Resume, 14);
        assert_eq!(f.poll(17, false, false), 0);
        assert_eq!(f.coordinator.host().changes.len(), 3);
    }
}
