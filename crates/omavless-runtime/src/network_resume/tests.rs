// SPDX-License-Identifier: MIT
use super::*;

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
    fixture
        .owner
        .receive(&mut fixture.source, &fixture.context, 10);
    let elapsed = before.elapsed();
    assert_eq!(fixture.owner.status, Status::SourceUnavailable);
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
fn authenticated_source_runs_original_coordinator_once_after_a_quiet_burst() {
    let mut f = Fixture::new();
    f.send(Kind::Resume, 100);
    f.send(Kind::NetworkChanged, 101);
    f.send(Kind::NetworkChanged, 102);
    assert_eq!(f.poll(104, false, false), 0);
    f.unchanged();
    assert_eq!(f.coordinator.host().observations, 0);
    assert_eq!(f.poll(105, false, false), 1);
    assert_eq!(f.owner.status, Status::Recovered);
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
    assert_eq!(f.owner.status, Status::Paused);
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
            f.owner.status,
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
    }
    let mut f = Fixture::new();
    let mut off = f.context.desired.clone();
    off.connected = false;
    off.profile_id.clear();
    write_desired(&f.desired, f.uid, &off).unwrap();
    f.context = f.coordinator.resume_context(BOOT, INSTANCE, 5).unwrap();
    f.send(Kind::Resume, 10);
    assert_eq!(f.owner.status, Status::Cancelled);
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
        f.owner.receive(&mut f.source, &f.context, 11);
        assert_eq!(f.owner.status, Status::SourceUnavailable);
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
    assert_eq!(f.owner.pending.as_ref().unwrap().hint.last_hint_tick, 10);
    assert_eq!(f.poll(13, false, false), 1);
    let mut f = Fixture::new();
    f.send(Kind::Resume, 10);
    assert_eq!(f.poll(9, false, false), 0);
    assert_eq!(f.owner.status, Status::SourceUnavailable);
    f.unchanged();
    let mut f = Fixture::new();
    for tick in 0..=61 {
        f.send(Kind::NetworkChanged, tick);
    }
    assert_eq!(f.poll(64, false, false), 0);
    assert_eq!(f.owner.status, Status::Cancelled);
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
        assert_eq!(f.owner.status, Status::ManualRecovery);
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
        assert_eq!(f.coordinator.host().changes.len(), changes);
        // New runtime identity cannot adopt the old Ready, even when the
        // attempted reservation failed before any file was changed.
        f.context.fence.owner_instance = [3; 16];
        f.owner = EventOwner::new(&f.context).unwrap();
        let (reader, writer) = UnixStream::pair().unwrap();
        f.writer = writer;
        f.sequence = 0;
        f.source = Source::owned(reader, std::process::id(), f.uid, &f.context).unwrap();
        f.send(Kind::Resume, 20);
        assert_eq!(f.poll(23, false, false), 0);
        assert_eq!(f.coordinator.host().changes.len(), changes);
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
    assert_eq!(f.owner.status, Status::ManualRecovery);
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
        assert_eq!(f.owner.status, Status::ManualRecovery);
    }
}

#[test]
fn original_mutation_lease_contention_never_turns_into_a_queued_connect() {
    let mut f = Fixture::new();
    f.send(Kind::Resume, 10);
    let lease = MigrationLock::acquire(&f.cutover, f.uid).unwrap();
    assert_eq!(f.poll(13, false, false), 0);
    assert_eq!(f.owner.status, Status::ManualRecovery);
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
fn changed_profile_store_content_or_deleted_target_cancels_without_observation() {
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
        assert_eq!(f.owner.status, Status::SourceUnavailable);
        f.unchanged();
        assert_eq!(f.coordinator.host().observations, 0);
    }
}

#[test]
fn status_projection_is_coarse_and_contains_no_identity_or_target() {
    let mut f = Fixture::new();
    f.send(Kind::Resume, 10);
    assert_eq!(
        f.owner.status_projection(),
        json!({"schema":1,"state":"checking"})
    );
    assert_eq!(f.poll(13, false, false), 1);
    assert_eq!(
        f.owner.status_projection(),
        json!({"schema":1,"state":"recovered"})
    );
}

#[test]
fn completion_publication_failure_never_repeats_a_completed_coordinator_operation() {
    for after in [false, true] {
        let mut f = Fixture::new();
        f.send(Kind::Resume, 10);
        assert_eq!(f.poll_fault(13, !after, after, Some(Phase::Finished)), 1);
        assert_eq!(f.owner.status, Status::ManualRecovery);
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
