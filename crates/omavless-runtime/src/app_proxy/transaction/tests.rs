// SPDX-License-Identifier: MIT
use super::*;
use crate::app_proxy::{
    Error as PlannerError,
    fields::{FIELD_COUNT, Field, tests::states},
};
use std::{
    fs,
    os::unix::fs::DirBuilderExt,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "s1-txn-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        fs::DirBuilder::new()
            .mode(0o700)
            .create(path.join("app-proxy-fields"))
            .unwrap();
        Self(path)
    }
    fn record(&self) -> Vec<u8> {
        fs::read(self.0.join("app-proxy-fields/app-proxy-journal.json")).unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn binding() -> Binding {
    Binding {
        owner_instance: [1; 16],
        owner_generation: 1,
        boot: [2; 16],
        session: [3; 16],
        uid: nix::unistd::geteuid().as_raw(),
    }
}

struct Host {
    current: State,
    delayed: Option<(Field, super::super::fields::Value)>,
    drain_available: bool,
    lose_reply: bool,
    no_write: bool,
    panic_write: bool,
    calls: Vec<&'static str>,
}
impl Host {
    fn new(current: State) -> Self {
        Self {
            current,
            delayed: None,
            drain_available: true,
            lose_reply: false,
            no_write: false,
            panic_write: false,
            calls: Vec::new(),
        }
    }
}
impl FixedHost for Host {
    fn observe(&mut self, _: Binding) -> Result<State, HostError> {
        self.calls.push("observe");
        Ok(self.current.clone())
    }
    fn write(&mut self, _: Binding, effect: &Effect) -> Result<(), HostError> {
        self.calls.push("write");
        if !self.no_write {
            self.delayed = Some((effect.field, effect.replacement.clone()));
        }
        assert!(!self.panic_write, "synthetic executor panic");
        if self.lose_reply {
            Err(HostError::OutcomeUnknown)
        } else {
            Ok(())
        }
    }
    fn drain(&mut self, _: Binding) -> Result<(), HostError> {
        self.calls.push("drain");
        if !self.drain_available {
            return Err(HostError::OutcomeUnknown);
        }
        if let Some((field, value)) = self.delayed.take() {
            self.current = self.current.with(field, value).unwrap();
        }
        Ok(())
    }
}

fn transaction(temp: &Temp) -> Transaction<Host> {
    let (original, intended) = states();
    let journal = FieldJournal::create(&temp.0, binding(), original.clone(), intended).unwrap();
    Transaction::new(journal, Host::new(original), binding()).unwrap()
}

#[test]
fn all_fields_commit_only_after_drain_then_reverse_restore() {
    let temp = Temp::new();
    let mut txn = transaction(&temp);
    let (original, intended) = states();
    for _ in 0..FIELD_COUNT {
        txn.step().unwrap();
    }
    assert_eq!(txn.step(), Ok(Phase::Active));
    assert_eq!(txn.host.current, intended);
    assert_eq!(
        &txn.host.calls[..5],
        &["observe", "observe", "write", "drain", "observe"]
    );
    txn.begin_restore().unwrap();
    for _ in 0..FIELD_COUNT {
        txn.step().unwrap();
    }
    assert_eq!(txn.step(), Ok(Phase::Released));
    assert_eq!(txn.host.current, original);
}

#[test]
fn lost_reply_is_confirmable_only_after_actual_delayed_effect_drain() {
    let temp = Temp::new();
    let mut txn = transaction(&temp);
    txn.host.lose_reply = true;
    txn.host.drain_available = false;
    assert_eq!(txn.step(), Err(Error::Host(HostError::OutcomeUnknown)));
    let pending = temp.record();
    assert_eq!(txn.step(), Err(Error::DrainRequired));
    assert_eq!(
        txn.begin_restore(),
        Err(Error::Host(HostError::OutcomeUnknown))
    );
    assert_eq!(temp.record(), pending);
    assert!(txn.host.delayed.is_some());
    txn.host.drain_available = true;
    txn.begin_restore().unwrap();
    while txn.phase() != Phase::Released {
        txn.step().unwrap();
    }
    assert_eq!(txn.host.current, states().0);
}

#[test]
fn setter_success_with_original_readback_never_confirms_and_can_compensate() {
    let temp = Temp::new();
    let mut txn = transaction(&temp);
    txn.host.no_write = true;
    assert_eq!(
        txn.step(),
        Err(Error::Journal(JournalError::Planner(
            PlannerError::UnconfirmedEffect
        )))
    );
    assert_eq!(
        txn.step(),
        Err(Error::Journal(JournalError::Planner(
            PlannerError::PendingEffect
        )))
    );
    txn.begin_restore().unwrap();
    assert_eq!(txn.step(), Ok(Phase::Released));
}

#[test]
fn foreign_edit_preserves_record_and_never_writes_another_field() {
    let temp = Temp::new();
    let mut txn = transaction(&temp);
    txn.step().unwrap();
    let before = temp.record();
    let foreign_field = Field::all().nth(2).unwrap();
    txn.host.current = txn
        .host
        .current
        .with(foreign_field, states().1.value(foreign_field))
        .unwrap();
    let writes = txn
        .host
        .calls
        .iter()
        .filter(|call| **call == "write")
        .count();
    assert!(txn.step().is_err());
    assert!(txn.begin_restore().is_err());
    assert_eq!(temp.record(), before);
    assert_eq!(
        txn.host
            .calls
            .iter()
            .filter(|call| **call == "write")
            .count(),
        writes
    );
}

#[test]
fn panic_and_reopened_journal_cannot_reissue_unknown_effect() {
    let temp = Temp::new();
    let mut txn = transaction(&temp);
    txn.host.panic_write = true;
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| txn.step())).is_err());
    assert_eq!(txn.step(), Err(Error::DrainRequired));
    txn.host.panic_write = false;
    let mut txn = txn.reopen_for_compensation(&temp.0).unwrap();
    assert_eq!(txn.step(), Err(Error::DrainRequired));
    txn.begin_restore().unwrap();
    while txn.phase() != Phase::Released {
        txn.step().unwrap();
    }
    assert_eq!(txn.host.current, states().0);
    assert!(!format!("{txn:?}").contains("synthetic"));
}

#[test]
fn new_port_cannot_claim_a_reopened_predecessor_journal() {
    let temp = Temp::new();
    let mut txn = transaction(&temp);
    txn.host.drain_available = false;
    assert!(txn.step().is_err());
    let pending = temp.record();
    drop(txn);
    let reopened = FieldJournal::open(&temp.0, binding()).unwrap();
    assert!(matches!(
        Transaction::new(reopened, Host::new(states().0), binding()),
        Err(Error::Journal(JournalError::RecoveryRequired))
    ));
    assert_eq!(temp.record(), pending);
}

#[test]
fn reentry_rejects_an_older_valid_record_even_with_unchanged_binding() {
    let temp = Temp::new();
    let mut txn = transaction(&temp);
    let older = temp.record();
    txn.step().unwrap();
    fs::write(
        temp.0.join("app-proxy-fields/app-proxy-journal.json"),
        &older,
    )
    .unwrap();
    assert!(matches!(
        txn.reopen_for_compensation(&temp.0),
        Err(Error::Journal(JournalError::ForeignChange))
    ));
    assert_eq!(temp.record(), older);
}
