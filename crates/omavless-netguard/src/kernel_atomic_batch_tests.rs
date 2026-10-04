// SPDX-License-Identifier: MIT
//! Pure structural tests: no socket, namespace, kernel effect or authority.
use super::*;

const PORT: u32 = 42;
fn batches() -> [AtomicBatch; 3] {
    [
        full_batch(7, 100, None).unwrap(),
        full_batch(7, 100, Some(9)).unwrap(),
        delete_batch(7, 9, 100).unwrap(),
    ]
}
fn ack(request: &[u8], code: i32, capped: bool) -> Vec<u8> {
    message(
        2,
        if capped { 0x100 } else { 0 },
        u32n(&request[8..12]).unwrap(),
        PORT,
        &[
            code.to_ne_bytes().to_vec(),
            if code == 0 || capped {
                request[..16].to_vec()
            } else {
                request.to_vec()
            },
        ]
        .concat(),
    )
}
fn push(c: &mut AtomicReplies, bytes: &[u8]) -> Result<()> {
    c.receive(bytes, Some(NetlinkAddr::new(0, 0)), MsgFlags::empty())
}
fn assert_permanent(c: &mut AtomicReplies, requests: &AtomicBatch) {
    assert_eq!(c.status(), UntrustedStatus::Uncertain);
    for request in requests.iter() {
        assert!(push(c, &ack(request, 0, false)).is_err());
        assert_eq!(c.status(), UntrustedStatus::Uncertain);
    }
}

#[test]
fn every_ack_subset_and_next_transition_covers_all_orderings_and_duplicates() {
    // Inductive state-space coverage, not a claim to enumerate 15! transcripts:
    // every subset and every possible next ACK is visited. The accepted state
    // is exactly the union of that subset and ACK, independent of ordering.
    for batch in batches() {
        let packets: Vec<_> = batch.iter().map(|r| ack(r, 0, false)).collect();
        let full = (1usize << batch.len()) - 1;
        for mask in 0..=full {
            let mut prefix = AtomicReplies::new(batch.clone(), PORT).unwrap();
            for (index, packet) in packets.iter().enumerate() {
                if mask & (1 << index) != 0 {
                    push(&mut prefix, packet).unwrap();
                }
            }
            assert_eq!(prefix.complete(), mask == full);
            for (index, packet) in packets.iter().enumerate() {
                let mut next = prefix.clone();
                if mask & (1 << index) != 0 {
                    assert!(push(&mut next, packet).is_err());
                    assert_eq!(next.status(), UntrustedStatus::Uncertain);
                } else {
                    push(&mut next, packet).unwrap();
                    let expected = mask | (1 << index);
                    for (bit, acknowledged) in next.acks().iter().enumerate() {
                        assert_eq!(*acknowledged, expected & (1 << bit) != 0);
                    }
                    assert_eq!(next.complete(), expected == full);
                }
            }
        }
    }
}

#[test]
fn missing_any_begin_operation_or_end_seals_uncertain_without_late_repair() {
    for batch in batches() {
        for missing in 0..batch.len() {
            let mut c = AtomicReplies::new(batch.clone(), PORT).unwrap();
            let bytes: Vec<_> = batch
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != missing)
                .flat_map(|(_, r)| ack(r, 0, true))
                .collect();
            push(&mut c, &bytes).unwrap();
            assert_eq!(c.status(), UntrustedStatus::Incomplete);
            // Transport deadline/cancellation may only invalidate, not retry.
            c.poison();
            assert_permanent(&mut c, &batch);
        }
    }
}

#[test]
fn exact_error_echoes_only_begin_restart_before_any_success_is_refusal() {
    for batch in batches() {
        for capped in [false, true] {
            for (index, request) in batch.iter().enumerate() {
                for code in [-85, -1, -2, -22, -95, 1] {
                    let mut c = AtomicReplies::new(batch.clone(), PORT).unwrap();
                    let result = push(&mut c, &ack(request, code, capped));
                    if index == 0 && code == -85 {
                        result.unwrap();
                        assert_eq!(c.status(), UntrustedStatus::GenerationRefused);
                        // Terminal refusal is not a reusable collector.
                        assert!(push(&mut c, &ack(request, 0, false)).is_err());
                    } else {
                        assert!(result.is_err());
                    }
                    assert_permanent(&mut c, &batch);
                }
            }
            let restart = ack(&batch[0], -85, capped);
            for offset in 20..restart.len() {
                let mut altered = restart.clone();
                altered[offset] ^= 1;
                let mut c = AtomicReplies::new(batch.clone(), PORT).unwrap();
                assert!(push(&mut c, &altered).is_err());
                assert_permanent(&mut c, &batch);
            }
            for request in batch.iter() {
                let mut c = AtomicReplies::new(batch.clone(), PORT).unwrap();
                push(&mut c, &ack(request, 0, false)).unwrap();
                assert!(push(&mut c, &restart).is_err());
                assert_permanent(&mut c, &batch);
            }
            let mut c = AtomicReplies::new(batch.clone(), PORT).unwrap();
            assert!(
                push(
                    &mut c,
                    &[restart.clone(), ack(&batch[1], 0, false)].concat()
                )
                .is_err()
            );
            assert_permanent(&mut c, &batch);
        }
    }
}

#[test]
fn malformed_truncated_padding_and_receive_metadata_permanently_poison() {
    for batch in batches() {
        for request in batch.iter() {
            let packet = ack(request, 0, false);
            let mut bad_packets: Vec<_> = (0..packet.len()).map(|n| packet[..n].to_vec()).collect();
            for offset in [0, 4, 6, 8, 12, 16, 20, 24, 28, 32] {
                let mut bad = packet.clone();
                bad[offset] ^= 1;
                bad_packets.push(bad);
            }
            for byte in [0, 1] {
                let mut bad = packet.clone();
                bad.extend_from_slice(&[0, 0, 0, byte]);
                bad[..4].copy_from_slice(&37u32.to_ne_bytes());
                bad_packets.push(bad);
            }
            for bad in bad_packets {
                let mut c = AtomicReplies::new(batch.clone(), PORT).unwrap();
                assert!(push(&mut c, &bad).is_err());
                assert_permanent(&mut c, &batch);
            }
            for sender in [
                None,
                Some(NetlinkAddr::new(1, 0)),
                Some(NetlinkAddr::new(0, 1)),
            ] {
                let mut c = AtomicReplies::new(batch.clone(), PORT).unwrap();
                assert!(c.receive(&packet, sender, MsgFlags::empty()).is_err());
                assert_permanent(&mut c, &batch);
            }
            for flags in [
                MsgFlags::MSG_TRUNC,
                MsgFlags::MSG_CTRUNC,
                MsgFlags::MSG_TRUNC | MsgFlags::MSG_CTRUNC,
            ] {
                let mut c = AtomicReplies::new(batch.clone(), PORT).unwrap();
                assert!(
                    c.receive(&packet, Some(NetlinkAddr::new(0, 0)), flags)
                        .is_err()
                );
                assert_permanent(&mut c, &batch);
            }
        }
    }
}

#[test]
fn bounds_exhaustion_and_post_completion_are_terminal() {
    for batch in batches() {
        assert!(AtomicReplies::new(batch.clone(), 0).is_err());
        let bytes: Vec<_> = batch.iter().flat_map(|r| ack(r, 0, false)).collect();
        let mut c = AtomicReplies::new(batch.clone(), PORT).unwrap();
        push(&mut c, &bytes).unwrap();
        assert_eq!(c.status(), UntrustedStatus::AllAcknowledged);
        assert!(push(&mut c, &bytes).is_err());
        assert_permanent(&mut c, &batch);
        for total in [LIMIT, usize::MAX] {
            let mut c = AtomicReplies::new(batch.clone(), PORT).unwrap();
            c.total = total;
            assert!(push(&mut c, &bytes).is_err());
            assert_permanent(&mut c, &batch);
        }
        let mut c = AtomicReplies::new(batch.clone(), PORT).unwrap();
        c.datagrams = if batch.len() == 3 { 8 } else { 16 };
        assert!(push(&mut c, &bytes).is_err());
        assert_permanent(&mut c, &batch);
    }
    for old in [None, Some(9)] {
        assert!(full_batch(1, u32::MAX - 15, old).is_ok());
        assert!(full_batch(1, u32::MAX - 14, old).is_err());
        assert!(full_batch(0, 1, old).is_err());
        assert!(full_batch(1, 0, old).is_err());
    }
    assert!(full_batch(1, 1, Some(0)).is_err());
    assert!(delete_batch(1, 1, u32::MAX - 3).is_ok());
    assert!(delete_batch(1, 1, u32::MAX - 2).is_err());
    assert!(delete_batch(0, 1, 1).is_err());
    assert!(delete_batch(1, 0, 1).is_err());
    assert!(delete_batch(1, 1, 0).is_err());
}
