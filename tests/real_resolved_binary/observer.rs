// SPDX-License-Identifier: MIT
// Private test observer, not linked into broker/core or any normal workspace.
use serde::Serialize;
use zbus::blocking::{Connection, MessageIterator};
use zbus::zvariant::OwnedValue;

fn bus() -> Connection {
    zbus::blocking::connection::Builder::address(ADDRESS)
        .unwrap()
        .max_queued(1024)
        .method_timeout(Duration::from_secs(2))
        .build()
        .unwrap()
}
fn owner(connection: &Connection) -> Option<String> {
    connection
        .call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "GetNameOwner",
            &("org.freedesktop.resolve1",),
        )
        .ok()?
        .body()
        .deserialize()
        .ok()
}
fn credential(connection: &Connection, name: &str, method: &str) -> u32 {
    connection
        .call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            method,
            &(name,),
        )
        .unwrap()
        .body()
        .deserialize()
        .unwrap()
}
#[derive(Clone)]
struct RealResolver {
    connection: Connection,
    owner: String,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
struct Observation {
    servers: Servers,
    extended: Vec<(i32, Vec<u8>, u16, String)>,
    domains: Domains,
    route: bool,
    llmnr: String,
    mdns: String,
    tls: String,
    dnssec: String,
    anchors: Vec<String>,
}
impl Observation {
    fn empty(&self) -> bool {
        self.servers.is_empty()
            && self.extended.is_empty()
            && self.domains.is_empty()
            && !self.route
            && self.anchors.is_empty()
    }
    fn active(&self) -> bool {
        self.servers == [(2, vec![198, 18, 0, 2])]
            && self.extended == [(2, vec![198, 18, 0, 2], 0, String::new())]
            && self.domains == [(".".into(), true)]
            && self.route
    }
    fn same_non_dns(&self, baseline: &Self) -> bool {
        self.llmnr == baseline.llmnr
            && self.mdns == baseline.mdns
            && self.tls == baseline.tls
            && self.dnssec == baseline.dnssec
            && self.anchors == baseline.anchors
    }
}
impl RealResolver {
    fn new(pid: u32) -> Self {
        let connection = bus();
        let owner = owner(&connection).unwrap();
        assert_eq!(
            credential(&connection, &owner, "GetConnectionUnixProcessID"),
            pid
        );
        assert_eq!(
            credential(&connection, &owner, "GetConnectionUnixUser"),
            974
        );
        Self { connection, owner }
    }
    fn same_owner(&self) -> bool {
        owner(&self.connection).as_ref() == Some(&self.owner)
    }
    fn seed_unrelated(&self, index: i32) -> Observation {
        // Before monitor/broker release. DNSEx keeps the SetLinkDNS denial case
        // independent; the unrelated fixture has a distinct nonempty policy.
        assert!(self.same_owner());
        for (method, body) in [
            (
                "SetLinkDNSEx",
                zbus::zvariant::Structure::from((
                    index,
                    vec![(2_i32, vec![192_u8, 0, 2, 53], 0_u16, String::new())],
                )),
            ),
            (
                "SetLinkDomains",
                zbus::zvariant::Structure::from((
                    index,
                    vec![(String::from("unrelated.invalid"), true)],
                )),
            ),
            (
                "SetLinkDefaultRoute",
                zbus::zvariant::Structure::from((index, true)),
            ),
        ] {
            self.connection
                .call_method(
                    Some(self.owner.as_str()),
                    "/org/freedesktop/resolve1",
                    Some("org.freedesktop.resolve1.Manager"),
                    method,
                    &body,
                )
                .unwrap()
                .body()
                .deserialize::<()>()
                .unwrap();
        }
        let value = self.observe(index).unwrap();
        assert_eq!(value.servers, [(2, vec![192, 0, 2, 53])]);
        assert_eq!(value.extended, [(2, vec![192, 0, 2, 53], 0, String::new())]);
        assert_eq!(value.domains, [(String::from("unrelated.invalid"), true)]);
        assert!(value.route);
        value
    }
    fn observe(&self, index: i32) -> Option<Observation> {
        if !self.same_owner() {
            return None;
        }
        let path: OwnedObjectPath = self
            .connection
            .call_method(
                Some(self.owner.as_str()),
                "/org/freedesktop/resolve1",
                Some("org.freedesktop.resolve1.Manager"),
                "GetLink",
                &(index,),
            )
            .ok()?
            .body()
            .deserialize()
            .ok()?;
        if path.as_str() != format!("/org/freedesktop/resolve1/link/_3{index}") {
            return None;
        }
        let result = Observation {
            servers: self.property(&path, "DNS")?,
            extended: self.property(&path, "DNSEx")?,
            domains: self.property(&path, "Domains")?,
            route: self.property(&path, "DefaultRoute")?,
            llmnr: self.property(&path, "LLMNR")?,
            mdns: self.property(&path, "MulticastDNS")?,
            tls: self.property(&path, "DNSOverTLS")?,
            dnssec: self.property(&path, "DNSSEC")?,
            anchors: self.property(&path, "DNSSECNegativeTrustAnchors")?,
        };
        if !self.same_owner() {
            return None;
        }
        Some(result)
    }
    fn property<T: TryFrom<OwnedValue>>(&self, path: &OwnedObjectPath, name: &str) -> Option<T> {
        let reply = self
            .connection
            .call_method(
                Some(self.owner.as_str()),
                path.as_str(),
                Some("org.freedesktop.DBus.Properties"),
                "Get",
                &("org.freedesktop.resolve1.Link", name),
            )
            .ok()?;
        if reply.data().len() > 16384 || reply.header().sender()?.as_str() != self.owner {
            return None;
        }
        T::try_from(reply.body().deserialize::<OwnedValue>().ok()?).ok()
    }
}

#[derive(Serialize)]
struct Effect {
    method: String,
    serial: u32,
    sender: String,
    outcome: Option<String>,
}
struct Monitor {
    connection: Connection,
    freezer: Connection,
    worker: thread::JoinHandle<()>,
    effects: Arc<Mutex<Vec<Effect>>>,
}
impl Monitor {
    fn start(real: RealResolver, broker: u32) -> Self {
        let connection = bus();
        let iterator = MessageIterator::from(&connection);
        connection
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus.Monitoring"),
                "BecomeMonitor",
                &(Vec::<String>::new(), 0_u32),
            )
            .unwrap();
        let effects = Arc::new(Mutex::new(Vec::<Effect>::new()));
        let sink = effects.clone();
        let freezer = real.connection.clone();
        let freeze_sender = freezer.unique_name().unwrap().to_string();
        let worker = thread::spawn(move || {
            for (count, message) in iterator.enumerate() {
                assert!(count < 20000, "bounded private monitor");
                let message = match message {
                    Ok(value) => value,
                    Err(_) => break,
                };
                assert!(message.data().len() <= 16384);
                let header = message.header();
                if message.message_type() == zbus::message::Type::Signal
                    && header.sender().map(|x| x.as_str()) == Some(freeze_sender.as_str())
                    && header.interface().map(|x| x.as_str()) == Some("org.omavless.TestObserver")
                    && header.member().map(|x| x.as_str()) == Some("Freeze")
                {
                    message.body().deserialize::<()>().unwrap();
                    assert!(
                        sink.lock()
                            .unwrap()
                            .iter()
                            .all(|effect| effect.outcome.is_some())
                    );
                    return;
                }
                if message.message_type() == zbus::message::Type::MethodCall
                    && header.interface().map(|x| x.as_str())
                        == Some("org.freedesktop.resolve1.Manager")
                {
                    let method = header.member().unwrap().as_str();
                    if !matches!(
                        method,
                        "SetLinkDNS" | "SetLinkDomains" | "SetLinkDefaultRoute" | "RevertLink"
                    ) {
                        continue;
                    }
                    let sender = header.sender().unwrap().to_string();
                    assert_eq!(
                        credential(&real.connection, &sender, "GetConnectionUnixProcessID"),
                        broker
                    );
                    assert_eq!(
                        credential(&real.connection, &sender, "GetConnectionUnixUser"),
                        0
                    );
                    assert_eq!(header.destination().unwrap().as_str(), real.owner);
                    assert!(
                        !header
                            .primary()
                            .flags()
                            .contains(zbus::message::Flags::AllowInteractiveAuth)
                    );
                    let actual = index().unwrap();
                    match method {
                        "SetLinkDNS" => assert_eq!(
                            message.body().deserialize::<(i32, Servers)>().unwrap(),
                            (actual, vec![(2, vec![198, 18, 0, 2])])
                        ),
                        "SetLinkDomains" => assert_eq!(
                            message.body().deserialize::<(i32, Domains)>().unwrap(),
                            (actual, vec![(".".into(), true)])
                        ),
                        "SetLinkDefaultRoute" => assert_eq!(
                            message.body().deserialize::<(i32, bool)>().unwrap(),
                            (actual, true)
                        ),
                        "RevertLink" => {
                            assert_eq!(message.body().deserialize::<i32>().unwrap(), actual)
                        }
                        _ => unreachable!(),
                    }
                    let mut effects = sink.lock().unwrap();
                    assert!(effects.len() < 16);
                    effects.push(Effect {
                        method: method.into(),
                        serial: header.primary().serial_num().get(),
                        sender,
                        outcome: None,
                    });
                } else if matches!(
                    message.message_type(),
                    zbus::message::Type::MethodReturn | zbus::message::Type::Error
                ) {
                    let Some(serial) = header.reply_serial() else {
                        continue;
                    };
                    let Some(destination) = header.destination() else {
                        continue;
                    };
                    let mut effects = sink.lock().unwrap();
                    if let Some(effect) = effects
                        .iter_mut()
                        .find(|e| e.serial == serial.get() && e.sender == destination.as_str())
                    {
                        assert!(effect.outcome.is_none());
                        let sender = header.sender().unwrap().as_str();
                        if message.message_type() == zbus::message::Type::MethodReturn {
                            assert_eq!(sender, real.owner);
                            message.body().deserialize::<()>().unwrap();
                            effect.outcome = Some("settled_success".into());
                        } else {
                            assert!(sender == real.owner || sender == "org.freedesktop.DBus");
                            effect.outcome = Some(header.error_name().unwrap().as_str().to_owned());
                        }
                    }
                }
            }
            panic!("monitor ended without authenticated freeze");
        });
        Self {
            connection,
            freezer,
            worker,
            effects,
        }
    }
    fn stop(self) {
        assert!(!self.worker.is_finished());
        self.freezer
            .emit_signal(
                None::<&str>,
                "/org/omavless/TestObserver",
                "org.omavless.TestObserver",
                "Freeze",
                &(),
            )
            .unwrap();
        self.worker.join().unwrap();
        self.connection.close().unwrap();
    }
}
