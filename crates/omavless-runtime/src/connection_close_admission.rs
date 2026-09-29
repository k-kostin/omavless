// SPDX-License-Identifier: MIT
//! Inactive, synthetic T3 confirmation model. Compiled only in unit tests.
//! No transport, wire schema, public API or executable close permit exists.
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const MAX_ROWS: usize = 128;
const MAX_TOKENS: usize = 1024;
const MAX_RECEIPTS: usize = 128;
const TTL_MS: u64 = 5_000;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Opaque([u8; 32]);

impl fmt::Debug for Opaque {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Opaque([private])")
    }
}

// Trusted owner observations, never client input. Digests are private too.
#[derive(Clone, PartialEq, Eq)]
struct Context {
    instance: [u8; 32],
    ownership_generation: u64,
    desired_generation: u64,
    revision: u64,
    desired_digest: [u8; 32], // includes connected/profile/mode
    store_digest: [u8; 32],
    config_digest: [u8; 32],
    child_incarnation: [u8; 32], // not merely a reusable PID
    peer_pid: u32,
    socket_device: u64,
    socket_inode: u64,
}

struct Observation {
    context: Context,
    committed_connected_owner: bool,
    fresh_owned_core_config_profile_tun: bool,
    mutation_idle: bool,
    recovery_clear: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct Identity {
    core_id: String,
    incarnation: [u8; 32],
    display_binding: [u8; 32],
}

impl Identity {
    fn valid(&self) -> bool {
        // Conservative synthetic grammar, NOT a claim about Mihomo IDs.
        !self.core_id.is_empty()
            && self.core_id.len() <= 128
            && self
                .core_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
            && self.incarnation != [0; 32]
            && self.display_binding != [0; 32]
    }
}

struct Row {
    handle: Opaque,
    identity: Identity,
}

struct Snapshot {
    context: Context,
    expires_at: u64,
    rows: Vec<Row>,
}

struct Confirmation {
    handle: Opaque,
    ticket: Opaque,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    Invalid,
    Capacity,
    Clock,
    Unavailable,
    Stale,
    UnknownTarget,
    Confirmation,
    OperationConflict,
    CoreIdentityGuaranteeMissing,
}

// There is deliberately no success/permit variant for confirmation.
#[derive(Default)]
struct Registry {
    clock: Option<(u64, u64)>, // trusted epoch and monotonic milliseconds
    poisoned: bool,
    snapshot: Option<Snapshot>,
    pending: Option<Confirmation>,
    used_tokens: BTreeSet<Opaque>,
    receipts: BTreeMap<String, (Opaque, Opaque)>,
}

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CloseRegistry([private])")
    }
}

impl Registry {
    fn invalidate(&mut self) {
        self.snapshot = None;
        self.pending = None;
    }

    fn tick(&mut self, epoch: u64, now: u64) -> Result<(), Refusal> {
        if self.poisoned
            || self
                .clock
                .is_some_and(|(old_epoch, old_now)| epoch != old_epoch || now < old_now)
        {
            self.poisoned = true;
            self.invalidate();
            return Err(Refusal::Clock);
        }
        self.clock = Some((epoch, now));
        Ok(())
    }

    fn observe(&mut self, observation: &Observation) -> Result<(), Refusal> {
        if !observation.committed_connected_owner
            || !observation.fresh_owned_core_config_profile_tun
            || !observation.mutation_idle
            || !observation.recovery_clear
        {
            self.invalidate();
            return Err(Refusal::Unavailable);
        }
        Ok(())
    }

    fn token(&mut self, entropy: &mut impl FnMut() -> Option<[u8; 32]>) -> Result<Opaque, Refusal> {
        if self.used_tokens.len() == MAX_TOKENS {
            return Err(Refusal::Capacity);
        }
        // A future adapter must supply CSPRNG output. Tests inject synthetic
        // bytes. Never derive a handle from a core ID, index or destination.
        let token = Opaque(entropy().ok_or(Refusal::Unavailable)?);
        if token.0 == [0; 32] || !self.used_tokens.insert(token) {
            return Err(Refusal::Invalid);
        }
        Ok(token)
    }

    fn replace(
        &mut self,
        observation: &Observation,
        identities: Vec<Identity>,
        epoch: u64,
        now: u64,
        entropy: &mut impl FnMut() -> Option<[u8; 32]>,
    ) -> Result<Vec<Opaque>, Refusal> {
        // Failed refresh also revokes the old view. No fallback to old targets.
        self.invalidate();
        self.tick(epoch, now)?;
        self.observe(observation)?;
        if identities.len() > MAX_ROWS {
            return Err(Refusal::Capacity);
        }
        let mut ids = BTreeSet::new();
        if identities
            .iter()
            .any(|id| !id.valid() || !ids.insert(&id.core_id))
        {
            return Err(Refusal::Invalid);
        }
        let expires_at = now.checked_add(TTL_MS).ok_or(Refusal::Clock)?;
        let mut rows = Vec::with_capacity(identities.len());
        for identity in identities {
            rows.push(Row {
                handle: self.token(entropy)?,
                identity,
            });
        }
        let handles = rows.iter().map(|row| row.handle).collect();
        self.snapshot = Some(Snapshot {
            context: observation.context.clone(),
            expires_at,
            rows,
        });
        Ok(handles)
    }

    fn current(&mut self, observation: &Observation, epoch: u64, now: u64) -> Result<(), Refusal> {
        self.tick(epoch, now)?;
        self.observe(observation)?;
        if !self
            .snapshot
            .as_ref()
            .is_some_and(|s| s.context == observation.context && now < s.expires_at)
        {
            self.invalidate();
            return Err(Refusal::Stale);
        }
        Ok(())
    }

    fn prepare(
        &mut self,
        observation: &Observation,
        handle: Opaque,
        epoch: u64,
        now: u64,
        entropy: &mut impl FnMut() -> Option<[u8; 32]>,
    ) -> Result<Opaque, Refusal> {
        // New selection/prepare cancels any earlier confirmation.
        self.pending = None;
        self.current(observation, epoch, now)?;
        if !self
            .snapshot
            .as_ref()
            .unwrap()
            .rows
            .iter()
            .any(|r| r.handle == handle)
        {
            return Err(Refusal::UnknownTarget);
        }
        let ticket = self.token(entropy)?;
        self.pending = Some(Confirmation { handle, ticket });
        Ok(ticket)
    }

    fn confirm(
        &mut self,
        observation: &Observation,
        request: &Confirm,
        live_identity: Option<&Identity>,
        epoch: u64,
        now: u64,
    ) -> Refusal {
        if let Err(error) = self.current(observation, epoch, now) {
            return error;
        }
        if request.operation.is_empty()
            || request.operation.len() > 64
            || !request
                .operation
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        {
            return Refusal::Invalid;
        }
        if let Some(previous) = self.receipts.get(&request.operation) {
            return if *previous == (request.handle, request.ticket) {
                Refusal::CoreIdentityGuaranteeMissing
            } else {
                Refusal::OperationConflict
            };
        }
        if self.receipts.len() == MAX_RECEIPTS {
            return Refusal::Capacity;
        }
        let Some(pending) = self.pending.take() else {
            return Refusal::Confirmation;
        };
        if pending.handle != request.handle || pending.ticket != request.ticket {
            return Refusal::Confirmation;
        }
        let snapshot = self.snapshot.as_mut().unwrap();
        let Some(index) = snapshot
            .rows
            .iter()
            .position(|row| row.handle == request.handle)
        else {
            return Refusal::UnknownTarget;
        };
        // One-use handle as well as ticket, including a failed identity check.
        let row = snapshot.rows.remove(index);
        if live_identity != Some(&row.identity) {
            return Refusal::UnknownTarget;
        }
        self.receipts
            .insert(request.operation.clone(), (request.handle, request.ticket));
        // A second GET cannot rule out ID reuse before DELETE. No caller flag
        // can bypass this missing core-side guarantee; do not return the ID.
        Refusal::CoreIdentityGuaranteeMissing
    }
}

struct Confirm {
    handle: Opaque,
    ticket: Opaque,
    operation: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation() -> Observation {
        Observation {
            context: Context {
                instance: [1; 32],
                ownership_generation: 1,
                desired_generation: 2,
                revision: 3,
                desired_digest: [4; 32],
                store_digest: [5; 32],
                config_digest: [6; 32],
                child_incarnation: [7; 32],
                peer_pid: 8,
                socket_device: 9,
                socket_inode: 10,
            },
            committed_connected_owner: true,
            fresh_owned_core_config_profile_tun: true,
            mutation_idle: true,
            recovery_clear: true,
        }
    }

    fn identity(id: &str) -> Identity {
        Identity {
            core_id: id.into(),
            incarnation: [20; 32],
            display_binding: [21; 32],
        }
    }

    fn entropy() -> impl FnMut() -> Option<[u8; 32]> {
        let mut counter = 0_u64;
        move || {
            counter += 1;
            let mut bytes = [0; 32];
            bytes[..8].copy_from_slice(&counter.to_be_bytes());
            Some(bytes)
        }
    }

    fn prepared() -> (Registry, Observation, Identity, Confirm) {
        let mut registry = Registry::default();
        let observation = observation();
        let identity = identity("private-core-id");
        let mut entropy = entropy();
        let handle = registry
            .replace(&observation, vec![identity.clone()], 1, 100, &mut entropy)
            .unwrap()[0];
        let ticket = registry
            .prepare(&observation, handle, 1, 101, &mut entropy)
            .unwrap();
        (
            registry,
            observation,
            identity,
            Confirm {
                handle,
                ticket,
                operation: "op-1".into(),
            },
        )
    }

    #[test]
    fn valid_confirmation_never_produces_permission_and_exact_retry_is_effect_free() {
        let (mut registry, observation, identity, mut request) = prepared();
        assert_eq!(
            registry.confirm(&observation, &request, Some(&identity), 1, 102),
            Refusal::CoreIdentityGuaranteeMissing
        );
        assert_eq!(
            registry.confirm(&observation, &request, None, 1, 103),
            Refusal::CoreIdentityGuaranteeMissing
        );
        request.ticket = Opaque([80; 32]);
        assert_eq!(
            registry.confirm(&observation, &request, Some(&identity), 1, 104),
            Refusal::OperationConflict
        );
        request.operation = "op-2".into();
        assert_eq!(
            registry.confirm(&observation, &request, Some(&identity), 1, 105),
            Refusal::Confirmation
        );
        assert!(registry.snapshot.as_ref().unwrap().rows.is_empty());
    }

    #[test]
    fn every_owner_context_component_is_fenced() {
        let changes: [fn(&mut Context); 11] = [
            |c| c.instance[0] += 1,
            |c| c.ownership_generation += 1,
            |c| c.desired_generation += 1,
            |c| c.revision += 1,
            |c| c.desired_digest[0] += 1,
            |c| c.store_digest[0] += 1,
            |c| c.config_digest[0] += 1,
            |c| c.child_incarnation[0] += 1,
            |c| c.peer_pid += 1,
            |c| c.socket_device += 1,
            |c| c.socket_inode += 1,
        ];
        for change in changes {
            let (mut registry, mut observation, identity, request) = prepared();
            change(&mut observation.context);
            assert_eq!(
                registry.confirm(&observation, &request, Some(&identity), 1, 102),
                Refusal::Stale
            );
            assert!(registry.pending.is_none());
        }
    }

    #[test]
    fn unavailable_owner_facts_busy_and_recovery_revoke_confirmation() {
        let changes: [fn(&mut Observation); 4] = [
            |o| o.committed_connected_owner = false,
            |o| o.fresh_owned_core_config_profile_tun = false,
            |o| o.mutation_idle = false,
            |o| o.recovery_clear = false,
        ];
        for change in changes {
            let (mut registry, mut observation, identity, request) = prepared();
            change(&mut observation);
            assert_eq!(
                registry.confirm(&observation, &request, Some(&identity), 1, 102),
                Refusal::Unavailable
            );
            assert!(registry.snapshot.is_none());
        }
    }

    #[test]
    fn expiry_and_clock_discontinuity_fail_closed() {
        for (epoch, now, expected) in [
            (1, 5100, Refusal::Stale),
            (1, 100, Refusal::Clock),
            (2, 102, Refusal::Clock),
        ] {
            let (mut registry, observation, identity, request) = prepared();
            assert_eq!(
                registry.confirm(&observation, &request, Some(&identity), epoch, now),
                expected
            );
            assert!(registry.pending.is_none());
            if expected == Refusal::Clock {
                assert_eq!(
                    registry.replace(&observation, vec![], 1, 6000, &mut entropy()),
                    Err(Refusal::Clock)
                );
            }
        }
        let (mut registry, observation, identity, request) = prepared();
        assert_eq!(
            registry.confirm(&observation, &request, Some(&identity), 1, 5099),
            Refusal::CoreIdentityGuaranteeMissing
        );
        assert_eq!(
            registry.replace(&observation, vec![], 1, u64::MAX, &mut entropy()),
            Err(Refusal::Clock)
        );
    }

    #[test]
    fn refresh_reorder_and_identical_destinations_never_retarget_old_handles() {
        let (mut registry, observation, identity, request) = prepared();
        let other = Identity {
            core_id: "other-core-id".into(),
            ..identity.clone()
        };
        let mut sequence = 90_u8;
        let mut entropy = || {
            sequence += 1;
            Some([sequence; 32])
        };
        let fresh = registry
            .replace(
                &observation,
                vec![other.clone(), identity.clone()],
                1,
                102,
                &mut entropy,
            )
            .unwrap()[0];
        assert_ne!(fresh, request.handle);
        assert_eq!(
            registry.confirm(&observation, &request, Some(&other), 1, 103),
            Refusal::Confirmation
        );
        assert_eq!(
            registry.prepare(&observation, request.handle, 1, 104, &mut entropy),
            Err(Refusal::UnknownTarget)
        );
        // Original connection is still present, now second. Neither its old
        // index nor identical display metadata may select the new first row.
        let ticket = registry
            .prepare(&observation, fresh, 1, 105, &mut entropy)
            .unwrap();
        let fresh_request = Confirm {
            handle: fresh,
            ticket,
            operation: "op-fresh".into(),
        };
        assert_eq!(
            registry.confirm(&observation, &fresh_request, Some(&identity), 1, 106),
            Refusal::UnknownTarget
        );
    }

    #[test]
    fn target_disappearance_id_reuse_and_changed_display_consume_ticket() {
        for variant in 0..4 {
            let (mut registry, observation, identity, request) = prepared();
            let mut changed = identity.clone();
            match variant {
                1 => changed.core_id = "other-id".into(),
                2 => changed.incarnation[0] += 1,
                3 => changed.display_binding[0] += 1,
                _ => (),
            }
            let live = (variant != 0).then_some(&changed);
            assert_eq!(
                registry.confirm(&observation, &request, live, 1, 102),
                Refusal::UnknownTarget
            );
            assert_eq!(
                registry.confirm(&observation, &request, Some(&identity), 1, 103),
                Refusal::Confirmation
            );
        }
    }

    #[test]
    fn wrong_ticket_cancellation_and_new_selection_cannot_confirm_old_target() {
        let (mut registry, observation, identity, mut request) = prepared();
        request.ticket = Opaque([99; 32]);
        assert_eq!(
            registry.confirm(&observation, &request, Some(&identity), 1, 102),
            Refusal::Confirmation
        );
        let (mut registry, observation, identity, request) = prepared();
        registry.invalidate(); // cancel / leave page / lost connection
        assert_eq!(
            registry.confirm(&observation, &request, Some(&identity), 1, 102),
            Refusal::Stale
        );
        let (mut registry, observation, identity, request) = prepared();
        registry
            .prepare(&observation, request.handle, 1, 102, &mut || Some([98; 32]))
            .unwrap();
        assert_eq!(
            registry.confirm(&observation, &request, Some(&identity), 1, 103),
            Refusal::Confirmation
        );
    }

    #[test]
    fn malformed_duplicate_or_over_capacity_refresh_revokes_previous_snapshot() {
        let cases = vec![
            vec![identity("")],
            vec![identity("../../connections")],
            vec![identity(&"a".repeat(129))],
            vec![identity("a"), identity("a")],
            vec![identity("a"); MAX_ROWS + 1],
            vec![Identity {
                incarnation: [0; 32],
                ..identity("a")
            }],
        ];
        for identities in cases {
            let (mut registry, observation, _, _) = prepared();
            assert!(
                registry
                    .replace(&observation, identities, 1, 102, &mut entropy())
                    .is_err()
            );
            assert!(registry.snapshot.is_none());
            assert!(registry.pending.is_none());
        }
    }

    #[test]
    fn entropy_failures_collisions_and_exhaustion_do_not_reuse_authority() {
        for bytes in [None, Some([0; 32])] {
            let mut registry = Registry::default();
            assert!(
                registry
                    .replace(&observation(), vec![identity("a")], 1, 0, &mut || bytes)
                    .is_err()
            );
            assert!(registry.snapshot.is_none());
        }
        let mut registry = Registry::default();
        let mut entropy = entropy();
        for _ in 0..MAX_TOKENS {
            registry.token(&mut entropy).unwrap();
        }
        assert_eq!(registry.token(&mut entropy), Err(Refusal::Capacity));
        let (mut registry, observation, _, request) = prepared();
        assert_eq!(
            registry.replace(&observation, vec![identity("a")], 1, 102, &mut || Some(
                request.handle.0
            )),
            Err(Refusal::Invalid)
        );
        assert!(registry.snapshot.is_none());
    }

    #[test]
    fn receipt_capacity_refuses_without_eviction_or_dispatch() {
        let (mut registry, observation, identity, request) = prepared();
        for index in 0..MAX_RECEIPTS {
            registry
                .receipts
                .insert(format!("old-{index}"), (request.handle, request.ticket));
        }
        assert_eq!(
            registry.confirm(&observation, &request, Some(&identity), 1, 102),
            Refusal::Capacity
        );
        assert_eq!(registry.receipts.len(), MAX_RECEIPTS);
    }

    #[test]
    fn invalid_operation_identity_is_never_recorded() {
        for operation in [String::new(), "a".repeat(65), "op\nprivate".into()] {
            let (mut registry, observation, identity, mut request) = prepared();
            request.operation = operation;
            assert_eq!(
                registry.confirm(&observation, &request, Some(&identity), 1, 102),
                Refusal::Invalid
            );
            assert!(registry.receipts.is_empty());
        }
    }

    #[test]
    fn diagnostics_and_existing_rows_do_not_expose_identity_or_tickets() {
        let (registry, _, _, request) = prepared();
        assert_eq!(format!("{registry:?}"), "CloseRegistry([private])");
        assert_eq!(format!("{:?}", request.handle), "Opaque([private])");
        let rows =
            crate::connection_rows::project(crate::connection_rows::extract(&serde_json::json!({
                "connections":[{"id":"private-core-id", "metadata":{"host":"example.org"}}]
            })));
        assert!(!rows.to_string().contains("private-core-id"));
        assert!(rows["rows"][0].get("handle").is_none());
        assert!(rows["rows"][0].get("ticket").is_none());
        for methods in [crate::NATIVE_READ_METHODS, crate::NATIVE_MUTATION_METHODS] {
            assert!(
                !methods
                    .iter()
                    .any(|method| method.contains("connection_close")
                        || method.contains("connections.close"))
            );
        }
    }
}
