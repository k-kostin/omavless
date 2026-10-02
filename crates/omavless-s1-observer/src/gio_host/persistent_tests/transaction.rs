// SPDX-License-Identifier: MIT
//! Private installed-dconf experiment only. There is deliberately no exported
//! writer constructor, live endpoint discovery, packaged helper or IPC verb.
//! Dconf's internal changeset format is version-specific, not a stable host API.
use super::*;
use omavless_runtime::app_proxy::{
    Phase,
    codec::EnvironmentSnapshot,
    fields::{Effect, Field, State, Value},
    journal::{Binding, fields::FieldJournal},
    transaction::{FixedHost, HostError, Transaction},
};
use std::collections::BTreeMap;
use std::{cell::RefCell, rc::Rc};

const WRITER_PATH: &str = "/ca/desrt/dconf/Writer/user";
const WRITER_IFACE: &str = "ca.desrt.dconf.Writer";

fn binding() -> Binding {
    Binding {
        owner_instance: [1; 16],
        owner_generation: 1,
        boot: [2; 16],
        session: [3; 16],
        uid: nix::unistd::geteuid().as_raw(),
    }
}

struct DconfHost {
    fixture: Rc<RefCell<Fixture>>,
    connection: gio::DBusConnection,
    owner: String,
    last_tag: Option<u64>,
    timeout_ms: i32,
}

impl DconfHost {
    fn new(fixture: Rc<RefCell<Fixture>>) -> Self {
        let socket = gio::Socket::new(
            gio::SocketFamily::Unix,
            gio::SocketType::Stream,
            gio::SocketProtocol::Default,
        )
        .unwrap();
        socket.set_timeout(2);
        SocketExt::connect(
            &socket,
            &gio::UnixSocketAddress::new(&fixture.borrow().root.join("bus")),
            None::<&gio::Cancellable>,
        )
        .unwrap();
        let connection = crate::local_bus::authenticate(&socket).unwrap();
        connection.set_exit_on_close(false);
        let owner = Self::current_owner(&connection).unwrap();
        let directory = fixture.borrow().root.join("app-proxy-fields");
        if !directory.exists() {
            fs::DirBuilder::new().mode(0o700).create(directory).unwrap();
        }
        Self {
            fixture,
            connection,
            owner,
            last_tag: None,
            timeout_ms: 150,
        }
    }

    fn current_owner(connection: &gio::DBusConnection) -> Result<String, HostError> {
        let reply = connection
            .call_sync(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "GetNameOwner",
                Some(&("ca.desrt.dconf",).to_variant()),
                None,
                gio::DBusCallFlags::NO_AUTO_START,
                500,
                None::<&gio::Cancellable>,
            )
            .map_err(|_| HostError::Unavailable)?;
        reply
            .get::<(String,)>()
            .map(|(owner,)| owner)
            .ok_or(HostError::Unavailable)
    }

    fn revalidate(&self, supplied: Binding) -> Result<(), HostError> {
        if supplied != binding()
            || self.connection.is_closed()
            || Self::current_owner(&self.connection)? != self.owner
        {
            return Err(HostError::Changed);
        }
        Ok(())
    }

    /// One synchronous operation, addressed to the retained unique owner on
    /// this exact connection. Only fixed individual keys or an empty barrier
    /// changeset can reach this function. No GSettings optimistic queue exists.
    fn change(
        &mut self,
        changes: BTreeMap<String, Option<glib::Variant>>,
    ) -> Result<(), HostError> {
        let encoded = changes.to_variant();
        if encoded.type_().as_str() != "a{smv}" {
            return Err(HostError::Changed);
        }
        let bytes = encoded.data();
        let reply = self
            .connection
            .call_sync(
                Some(&self.owner),
                WRITER_PATH,
                WRITER_IFACE,
                "Change",
                Some(&(bytes,).to_variant()),
                None,
                gio::DBusCallFlags::NO_AUTO_START,
                self.timeout_ms,
                None::<&gio::Cancellable>,
            )
            .map_err(|_| HostError::OutcomeUnknown)?;
        let (tag,) = reply.get::<(String,)>().ok_or(HostError::OutcomeUnknown)?;
        let prefix = format!("{}:user:", self.owner);
        let count = tag
            .strip_prefix(&prefix)
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or(HostError::OutcomeUnknown)?;
        if self.last_tag.is_some_and(|last| count <= last) {
            return Err(HostError::OutcomeUnknown);
        }
        self.last_tag = Some(count);
        Ok(())
    }
}

impl FixedHost for DconfHost {
    fn observe(&mut self, supplied: Binding) -> Result<State, HostError> {
        self.revalidate(supplied)?;
        let path = self.fixture.borrow().root.join("transaction-observation");
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(HostError::Unavailable),
        }
        self.fixture.borrow().run("transaction-observe");
        let bytes = fs::read(&path).map_err(|_| HostError::Unavailable)?;
        let snapshot = omavless_runtime::app_proxy::Snapshot::new(Some(bytes))
            .map_err(|_| HostError::Unavailable)?;
        let desktop = DesktopSnapshot::decode(&snapshot).map_err(|_| HostError::Unavailable)?;
        self.revalidate(supplied)?;
        // This fixture does not read or write any real manager environment.
        let manager: EnvironmentSnapshot = crate::project_manager_environment(&[]).unwrap();
        Ok(State::new(desktop, manager))
    }

    fn write(&mut self, supplied: Binding, effect: &Effect) -> Result<(), HostError> {
        self.revalidate(supplied)?;
        let (Field::Desktop(key), Value::Desktop(expected), Value::Desktop(replacement)) =
            (effect.field, &effect.expected, &effect.replacement)
        else {
            return Err(HostError::Changed);
        };
        if expected.key != key
            || replacement.key != key
            || !expected.writable
            || !replacement.writable
            || expected.default != replacement.default
        {
            return Err(HostError::Changed);
        }
        let (schema, name, _) = key.schema_key_type();
        let (_, path, _) = SCHEMAS
            .iter()
            .find(|(id, _, _)| *id == schema)
            .ok_or(HostError::Changed)?;
        let value = match &replacement.user {
            Override::Absent => None,
            Override::Present(value) => Some(variant(value)),
        };
        self.change(BTreeMap::from([(format!("{path}{name}"), value)]))?;
        self.revalidate(supplied)
    }

    fn drain(&mut self, supplied: Binding) -> Result<(), HostError> {
        self.revalidate(supplied)?;
        // The reviewed dconf writer handles Change synchronously on its one
        // dispatch context. Same-connection FIFO plus same-owner reply places
        // this empty changeset after every earlier request, even a lost reply.
        // It neither resets keys nor reissues the pending value. This is a
        // fixture/source-dependent barrier, not production service provenance.
        self.change(BTreeMap::new())?;
        self.revalidate(supplied)
    }
}

fn intended(original: &State) -> State {
    let desktop = DesktopSnapshot::decode(&original.encode().unwrap()[0]).unwrap();
    let entries = desktop
        .entries()
        .iter()
        .map(|entry| {
            let mut replacement = entry.clone();
            let value = match entry.key.schema_key_type().2 {
                "s" if entry.key == DesktopKey::Mode => DesktopValue::String("manual".into()),
                "s" => DesktopValue::String("synthetic literal='value'\nvalue".into()),
                "b" => DesktopValue::Bool(!matches!(entry.effective, DesktopValue::Bool(true))),
                "i" => DesktopValue::Int(12345),
                "as" => DesktopValue::Strings(vec!["localhost".into(), "synthetic.invalid".into()]),
                _ => unreachable!(),
            };
            replacement.effective = value.clone();
            replacement.user = Override::Present(value);
            replacement
        })
        .collect();
    State::new(
        DesktopSnapshot::capture(entries).unwrap(),
        crate::project_manager_environment(&[]).unwrap(),
    )
}

fn setup() -> (Transaction<DconfHost>, State, Rc<RefCell<Fixture>>) {
    let fixture = Rc::new(RefCell::new(Fixture::new()));
    fixture.borrow().run("seed");
    let mut host = DconfHost::new(fixture.clone());
    let original = host.observe(binding()).unwrap();
    let journal = FieldJournal::create(
        &fixture.borrow().root,
        binding(),
        original.clone(),
        intended(&original),
    )
    .unwrap();
    (
        Transaction::new(journal, host, binding()).unwrap(),
        original,
        fixture,
    )
}

fn read_independently(fixture: &Fixture) -> DesktopSnapshot {
    let path = fixture.root.join("transaction-observation");
    fs::remove_file(&path).unwrap();
    fixture.run("transaction-observe");
    DesktopSnapshot::decode(
        &omavless_runtime::app_proxy::Snapshot::new(Some(fs::read(path).unwrap())).unwrap(),
    )
    .unwrap()
}

fn record(fixture: &Fixture) -> Vec<u8> {
    fs::read(fixture.root.join("app-proxy-fields/app-proxy-journal.json")).unwrap()
}

// Host/journal handles remain private to the fixture. Helpers below require
// the executor to retain those handles; no runtime takeover authority exists.
#[test]
#[ignore = "installed dconf; only a private bus/database; never host proxy or manager environment"]
fn fixed_dconf_transaction_restores_every_layer_and_presence() {
    let (mut txn, original, fixture) = setup();
    for _ in 0..16 {
        txn.step().unwrap();
    }
    assert_eq!(txn.step(), Ok(Phase::Active));
    txn.begin_restore().unwrap();
    for _ in 0..16 {
        txn.step().unwrap();
    }
    assert_eq!(txn.step(), Ok(Phase::Released));
    assert_eq!(
        read_independently(&fixture.borrow()).encode().unwrap(),
        original.encode().unwrap()[0]
    );
    // The terminal result already compares the entire independently observed
    // snapshot to the saved original, including all unchanged manager fields.
    assert_eq!(
        original.encode().unwrap()[1],
        crate::project_manager_environment(&[])
            .unwrap()
            .encode()
            .unwrap()
    );
}

#[test]
#[ignore = "private dconf; pauses only its fixture-owned service to lose replies"]
fn delayed_dconf_commit_requires_same_connection_owner_drain_before_restore() {
    let (mut txn, original, fixture) = setup();
    fixture
        .borrow_mut()
        .service
        .as_mut()
        .unwrap()
        .signal(Signal::SIGSTOP);
    assert!(txn.step().is_err());
    let pending = record(&fixture.borrow());
    assert_eq!(
        txn.step(),
        Err(omavless_runtime::app_proxy::transaction::Error::DrainRequired)
    );
    assert!(txn.begin_restore().is_err());
    assert_eq!(record(&fixture.borrow()), pending);
    // A fresh reader still seeing the original cannot release the journal:
    // the admitted request remains queued on the stopped service.
    assert_eq!(
        read_independently(&fixture.borrow()).encode().unwrap(),
        original.encode().unwrap()[0]
    );
    fixture
        .borrow_mut()
        .service
        .as_mut()
        .unwrap()
        .signal(Signal::SIGCONT);
    txn.begin_restore().unwrap();
    while txn.phase() != Phase::Released {
        txn.step().unwrap();
    }
    assert_eq!(
        read_independently(&fixture.borrow()).encode().unwrap(),
        original.encode().unwrap()[0]
    );
}

#[test]
#[ignore = "private dconf; kills only its fixture-owned service after an unknown effect"]
fn lost_dconf_owner_retains_unknown_journal_even_if_original_is_on_disk() {
    let (mut txn, original, fixture) = setup();
    fixture
        .borrow_mut()
        .service
        .as_mut()
        .unwrap()
        .signal(Signal::SIGSTOP);
    assert!(txn.step().is_err());
    let pending = record(&fixture.borrow());
    drop(fixture.borrow_mut().service.take());
    assert!(txn.begin_restore().is_err());
    assert_eq!(record(&fixture.borrow()), pending);
    assert_eq!(
        read_independently(&fixture.borrow()).encode().unwrap(),
        original.encode().unwrap()[0]
    );
}

#[test]
#[ignore = "private dconf; independent fixture writer supplies a foreign edit"]
fn foreign_dconf_edit_is_preserved_without_another_owned_write() {
    let (mut txn, _, fixture) = setup();
    txn.step().unwrap();
    let pending = record(&fixture.borrow());
    fixture.borrow().run("apply");
    let foreign = read_independently(&fixture.borrow());
    assert!(txn.begin_restore().is_err());
    assert!(txn.step().is_err());
    assert_eq!(record(&fixture.borrow()), pending);
    assert_eq!(read_independently(&fixture.borrow()), foreign);
}

#[test]
#[ignore = "private dconf; every partial write reopens only under its retained same-owner port"]
fn every_partial_dconf_journal_reentry_restores_exact_original() {
    for confirmed in 0..=16 {
        let (mut txn, original, fixture) = setup();
        for _ in 0..confirmed {
            txn.step().unwrap();
        }
        let root = fixture.borrow().root.clone();
        let mut txn = txn.reopen_for_compensation(&root).unwrap();
        assert_eq!(
            txn.step(),
            Err(omavless_runtime::app_proxy::transaction::Error::DrainRequired)
        );
        txn.begin_restore().unwrap();
        while txn.phase() != Phase::Released {
            txn.step().unwrap();
        }
        assert_eq!(
            read_independently(&fixture.borrow()).encode().unwrap(),
            original.encode().unwrap()[0]
        );
    }
}

pub(super) fn crash_child(root: &Path, schemas: &[gio::SettingsSchema]) {
    let desktop = read_desktop(schemas).unwrap();
    let original = State::new(desktop, crate::project_manager_environment(&[]).unwrap());
    let mut journal =
        FieldJournal::create(root, binding(), original.clone(), intended(&original)).unwrap();
    let effect = journal.begin_next(binding(), &original).unwrap().unwrap();
    assert_eq!(effect.field, Field::Desktop(DesktopKey::Mode));
    let socket = gio::Socket::new(
        gio::SocketFamily::Unix,
        gio::SocketType::Stream,
        gio::SocketProtocol::Default,
    )
    .unwrap();
    socket.set_timeout(2);
    SocketExt::connect(
        &socket,
        &gio::UnixSocketAddress::new(&root.join("bus")),
        None::<&gio::Cancellable>,
    )
    .unwrap();
    let connection = crate::local_bus::authenticate(&socket).unwrap();
    connection.set_exit_on_close(false);
    let owner = DconfHost::current_owner(&connection).unwrap();
    let changes = BTreeMap::from([("/system/proxy/mode".to_owned(), Some("manual".to_variant()))])
        .to_variant();
    assert_eq!(changes.type_().as_str(), "a{smv}");
    let outcome = connection.call_sync(
        Some(&owner),
        WRITER_PATH,
        WRITER_IFACE,
        "Change",
        Some(&(changes.data(),).to_variant()),
        None,
        gio::DBusCallFlags::NO_AUTO_START,
        150,
        None::<&gio::Cancellable>,
    );
    assert!(outcome.is_err());
    private_file(&root.join("transaction-crashed"), b"pending-no-drain");
    std::process::exit(33);
}

#[test]
#[ignore = "private dconf; exits only its journal writer child while an admitted request is queued"]
fn process_crash_cannot_replace_the_predecessor_drain_connection() {
    let fixture = Rc::new(RefCell::new(Fixture::new()));
    let mut host = DconfHost::new(fixture.clone());
    let original = host.observe(binding()).unwrap();
    fixture
        .borrow_mut()
        .service
        .as_mut()
        .unwrap()
        .signal(Signal::SIGSTOP);
    let mut writer = fixture.borrow().child("transaction-crash");
    fixture
        .borrow()
        .wait_for(|| fixture.borrow().root.join("transaction-crashed").exists());
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = writer.0.try_wait().unwrap() {
            assert_eq!(status.code(), Some(33));
            break;
        }
        assert!(Instant::now() < deadline, "private crash child timeout");
        std::thread::sleep(Duration::from_millis(10));
    }
    let pending = record(&fixture.borrow());
    let reopened = FieldJournal::open(&fixture.borrow().root, binding()).unwrap();
    // This parent's otherwise valid connection is not the exited writer's
    // origin. It cannot make that writer's journal eligible for compensation.
    assert!(matches!(
        Transaction::new(reopened, host, binding()),
        Err(omavless_runtime::app_proxy::transaction::Error::Journal(
            omavless_runtime::app_proxy::journal::Error::RecoveryRequired
        ))
    ));
    assert_eq!(record(&fixture.borrow()), pending);
    assert_eq!(
        read_independently(&fixture.borrow()).encode().unwrap(),
        original.encode().unwrap()[0]
    );
    fixture
        .borrow_mut()
        .service
        .as_mut()
        .unwrap()
        .signal(Signal::SIGCONT);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if read_independently(&fixture.borrow()).encode().unwrap() != original.encode().unwrap()[0]
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "delayed private crash request did not commit"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(record(&fixture.borrow()), pending);
}
