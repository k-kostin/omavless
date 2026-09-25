//! Opt-in real Host integration. The only DNS server is a private mock bus.
use super::*;
use omavless_dns_channel::Listener;
use omavless_dns_tun::HeldTun;
use rustix::net::{self, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags};
use std::{
    fs,
    io::IoSliceMut,
    mem::MaybeUninit,
    os::unix::net::UnixDatagram,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use zbus::{fdo, zvariant::OwnedObjectPath};

const UNIT: &str = "/org/freedesktop/systemd1/unit/omavless_2ddns_2dbroker_2eservice";
type Servers = Vec<(i32, Vec<u8>)>;
type Domains = Vec<(String, bool)>;
type Entry = (String, u32, u32, u32, u64, u32, u32, String, u32);

#[derive(Default)]
struct State {
    index: i32,
    servers: Servers,
    domains: Domains,
    route: bool,
    calls: Vec<&'static str>,
    stored: Option<OwnedFd>,
    notifications: Vec<String>,
    case: String,
    drift: bool,
}
struct Manager(Arc<Mutex<State>>);
#[zbus::interface(name = "org.freedesktop.systemd1.Service")]
impl Manager {
    #[zbus(property, name = "MainPID")]
    fn main_pid(&self) -> u32 {
        std::process::id()
    }
    #[zbus(property)]
    fn notify_access(&self) -> &str {
        "main"
    }
    #[zbus(property)]
    fn file_descriptor_store_max(&self) -> u32 {
        1
    }
    #[zbus(property)]
    fn file_descriptor_store_preserve(&self) -> &str {
        "yes"
    }
    #[zbus(property)]
    fn runtime_directory_preserve(&self) -> &str {
        "yes"
    }
    #[zbus(property, name = "NFileDescriptorStore")]
    fn n_file_descriptor_store(&self) -> u32 {
        u32::from(self.0.lock().unwrap().stored.is_some())
    }
    fn dump_file_descriptor_store(&self) -> Vec<Entry> {
        let state = self.0.lock().unwrap();
        let Some(fd) = &state.stored else {
            return vec![];
        };
        let s = rustix::fs::fstat(fd).unwrap();
        let flags =
            rustix::fs::fcntl_getfl(fd).unwrap().bits() & !rustix::fs::OFlags::LARGEFILE.bits();
        vec![(
            "omavless-tun-lease".into(),
            s.st_mode,
            rustix::fs::major(s.st_dev),
            rustix::fs::minor(s.st_dev),
            s.st_ino,
            rustix::fs::major(s.st_rdev),
            rustix::fs::minor(s.st_rdev),
            "/dev/net/tun".into(),
            flags,
        )]
    }
}

struct Resolve(Arc<Mutex<State>>);
#[zbus::interface(name = "org.freedesktop.resolve1.Manager")]
impl Resolve {
    fn get_link(&self, index: i32) -> OwnedObjectPath {
        assert_eq!(index, self.0.lock().unwrap().index);
        OwnedObjectPath::try_from(format!("/org/freedesktop/resolve1/link/_{index}")).unwrap()
    }
    #[zbus(name = "SetLinkDNS")]
    fn set_link_dns(
        &self,
        index: i32,
        servers: Servers,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) {
        assert!(
            !header
                .primary()
                .flags()
                .contains(zbus::message::Flags::AllowInteractiveAuth)
        );
        // Verify actual durable intent and manager-side FD before the first write.
        let record: serde_json::Value =
            serde_json::from_slice(&fs::read("/run/omavless-dns/private/lease.json").unwrap())
                .unwrap();
        assert_eq!(record["phase"], "applying");
        let delay = {
            let mut state = self.0.lock().unwrap();
            assert_eq!(index, state.index);
            assert_eq!(servers, [(2, vec![198, 18, 0, 2])]);
            assert!(state.stored.is_some());
            state.calls.push("dns");
            state.case == "timeout"
        };
        if delay {
            thread::sleep(Duration::from_millis(2300));
        }
        self.0.lock().unwrap().servers = servers;
    }
    fn set_link_domains(&self, index: i32, domains: Domains) -> fdo::Result<()> {
        let mut state = self.0.lock().unwrap();
        assert_eq!(index, state.index);
        assert_eq!(domains, [(".".into(), true)]);
        state.calls.push("domains");
        if state.case == "denial" {
            return Err(fdo::Error::AccessDenied(
                "synthetic private diagnostic".into(),
            ));
        }
        state.domains = domains;
        Ok(())
    }
    fn set_link_default_route(&self, index: i32, enabled: bool) {
        let mut state = self.0.lock().unwrap();
        assert_eq!(index, state.index);
        assert!(enabled);
        state.calls.push("route");
        state.route = enabled && state.case != "mismatch";
        state.drift = state.case == "apply_drift";
    }
    fn revert_link(&self, index: i32) {
        let mut state = self.0.lock().unwrap();
        assert_eq!(index, state.index);
        assert!(state.stored.is_some());
        let record: serde_json::Value =
            serde_json::from_slice(&fs::read("/run/omavless-dns/private/lease.json").unwrap())
                .unwrap();
        assert_eq!(record["phase"], "releasing");
        state.calls.push("revert");
        state.servers.clear();
        state.domains.clear();
        state.route = false;
    }
}
struct Link(Arc<Mutex<State>>);
#[zbus::interface(name = "org.freedesktop.resolve1.Link")]
impl Link {
    #[zbus(property, name = "DNS")]
    fn dns(&self) -> Servers {
        self.0.lock().unwrap().servers.clone()
    }
    #[zbus(property, name = "DNSEx")]
    fn dns_ex(&self) -> Vec<(i32, Vec<u8>, u16, String)> {
        self.0
            .lock()
            .unwrap()
            .servers
            .iter()
            .map(|(family, bytes)| (*family, bytes.clone(), 0, String::new()))
            .collect()
    }
    #[zbus(property)]
    fn domains(&self) -> Domains {
        self.0.lock().unwrap().domains.clone()
    }
    #[zbus(property)]
    fn default_route(&self) -> bool {
        self.0.lock().unwrap().route
    }
    #[zbus(property, name = "LLMNR")]
    fn llmnr(&self) -> &str {
        if self.0.lock().unwrap().drift {
            "no"
        } else {
            "yes"
        }
    }
    #[zbus(property, name = "MulticastDNS")]
    fn mdns(&self) -> &str {
        "no"
    }
    #[zbus(property, name = "DNSOverTLS")]
    fn dns_over_tls(&self) -> &str {
        "no"
    }
    #[zbus(property, name = "DNSSEC")]
    fn dnssec(&self) -> &str {
        if self.0.lock().unwrap().case == "policy" {
            "yes"
        } else {
            "no"
        }
    }
    #[zbus(property, name = "DNSSECNegativeTrustAnchors")]
    fn nta(&self) -> Vec<String> {
        vec![]
    }
}

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct Notify {
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Notify {
    fn new(receiver: UnixDatagram, state: Arc<Mutex<State>>) -> Self {
        receiver
            .set_read_timeout(Some(Duration::from_millis(30)))
            .unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let worker = thread::spawn(move || {
            while !stopped.load(Ordering::Acquire) {
                let mut bytes = [0; 256];
                let mut storage =
                    [MaybeUninit::uninit(); rustix::cmsg_aligned_space!(ScmRights(1))];
                let mut ancillary = RecvAncillaryBuffer::new(&mut storage);
                let received = match net::recvmsg(
                    &receiver,
                    &mut [IoSliceMut::new(&mut bytes)],
                    &mut ancillary,
                    RecvFlags::CMSG_CLOEXEC,
                ) {
                    Ok(value) => value,
                    Err(rustix::io::Errno::AGAIN) => continue,
                    Err(error) => panic!("private notification receive: {error:?}"),
                };
                assert!(
                    !received
                        .flags
                        .intersects(net::ReturnFlags::TRUNC | net::ReturnFlags::CTRUNC)
                );
                let mut fds = vec![];
                for record in ancillary.drain() {
                    match record {
                        RecvAncillaryMessage::ScmRights(rights) => fds.extend(rights),
                        _ => panic!("unexpected private ancillary"),
                    }
                }
                let message = std::str::from_utf8(&bytes[..received.bytes]).unwrap();
                let mut state = state.lock().unwrap();
                state.notifications.push(message.into());
                match message {
                    "FDSTORE=1\nFDNAME=omavless-tun-lease\nFDPOLL=0" => {
                        assert_eq!(fds.len(), 1);
                        assert!(state.stored.is_none());
                        state.stored = fds.pop();
                    }
                    "BARRIER=1" => {
                        assert_eq!(fds.len(), 1);
                    }
                    "FDSTOREREMOVE=1\nFDNAME=omavless-tun-lease" => {
                        assert!(fds.is_empty());
                        let record: serde_json::Value = serde_json::from_slice(
                            &fs::read("/run/omavless-dns/private/lease.json").unwrap(),
                        )
                        .unwrap();
                        assert_eq!(record["phase"], "cleanup_verified");
                        assert!(
                            state.servers.is_empty() && state.domains.is_empty() && !state.route
                        );
                        state.stored = None;
                    }
                    _ => panic!("non-fixed notification"),
                }
            }
        });
        Self {
            stop,
            worker: Some(worker),
        }
    }
}
impl Drop for Notify {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let joined = self.worker.take().unwrap().join();
        if !thread::panicking() {
            assert!(joined.is_ok());
        }
    }
}

fn guard() -> String {
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    for name in ["net", "user", "pid", "mnt"] {
        let original = std::env::var(format!("OMAVLESS_TEST_NS_{}", name.to_uppercase())).unwrap();
        assert!(original.starts_with(&format!("{name}:[")) && original.ends_with(']'));
        assert_ne!(
            fs::read_link(format!("/proc/self/ns/{name}"))
                .unwrap()
                .to_str()
                .unwrap(),
            original
        );
    }
    let case = std::env::var("OMAVLESS_TEST_CASE").unwrap();
    assert!(
        [
            "success",
            "denial",
            "timeout",
            "drop",
            "policy",
            "mismatch",
            "apply_drift",
            "release_drift",
            "dns_drift",
            "release_dns_drift"
        ]
        .contains(&case.as_str())
    );
    case
}

fn tun_exists() -> bool {
    let socket = net::socket(net::AddressFamily::INET, net::SocketType::DGRAM, None).unwrap();
    net::netdevice::name_to_index(&socket, "Meta").is_ok()
}

#[test]
#[ignore = "requires guarded disposable namespaces; tests/dns_broker_composition_probe.py"]
fn actual_host_composition() {
    let case = guard();
    let root = tempfile::tempdir().unwrap();
    let listener = Listener::bind(&root.path().join("channel"), 0).unwrap();
    // Production channel authenticates SCM_CREDENTIALS and admits this actual
    // kernel TUN FD. Only this guarded fixture creates a TUN; no Rust unsafe.
    let producer = ChildGuard(
        Command::new("/usr/bin/python3")
            .args([
                "-c",
                r#"
import array, fcntl, os, socket, struct, sys
fd=os.open('/dev/net/tun', os.O_RDWR|os.O_CLOEXEC)
fcntl.ioctl(fd, 0x400454ca, struct.pack('16sH', b'Meta', 0x1001))
s=socket.socket(socket.AF_UNIX, socket.SOCK_SEQPACKET)
s.connect(sys.argv[1])
s.sendmsg([b'OVDN\x01\x01\x01\x00'], [(socket.SOL_SOCKET,socket.SCM_RIGHTS,array.array('i',[fd]))])
sys.stdin.buffer.read(1)
"#,
                root.path().join("channel").to_str().unwrap(),
            ])
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut session = listener.accept().unwrap();
    let proof = session
        .receive_acquire()
        .unwrap()
        .try_clone_to_owned()
        .unwrap();
    let held = HeldTun::admit(proof).unwrap();
    let index = held.interface_index();
    let state = Arc::new(Mutex::new(State {
        index: i32::try_from(index).unwrap(),
        case: case.clone(),
        ..State::default()
    }));
    let address = format!("unix:path={}", root.path().join("bus").display());
    let mut bus = ChildGuard(
        Command::new("dbus-daemon")
            .args([
                "--session",
                "--nofork",
                "--nopidfile",
                "--address",
                &address,
            ])
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    while !root.path().join("bus").exists() {
        assert!(bus.0.try_wait().unwrap().is_none() && Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
    let manager = zbus::blocking::connection::Builder::address(address.as_str())
        .unwrap()
        .serve_at(UNIT, Manager(state.clone()))
        .unwrap()
        .name("org.freedesktop.systemd1")
        .unwrap()
        .build()
        .unwrap();
    let resolved = zbus::blocking::connection::Builder::address(address.as_str())
        .unwrap()
        .serve_at("/org/freedesktop/resolve1", Resolve(state.clone()))
        .unwrap()
        .serve_at(
            format!("/org/freedesktop/resolve1/link/_{index}"),
            Link(state.clone()),
        )
        .unwrap()
        .name("org.freedesktop.resolve1")
        .unwrap()
        .build()
        .unwrap();
    let client = zbus::blocking::connection::Builder::address(address.as_str())
        .unwrap()
        .build()
        .unwrap();
    let receive = UnixDatagram::bind(root.path().join("notify")).unwrap();
    let send = UnixDatagram::unbound().unwrap();
    send.connect(root.path().join("notify")).unwrap();
    let _notify = Notify::new(receive, state.clone());
    let context = RootContext::fixture_context(client.clone(), send.into()).unwrap();
    let retention = Retention::from_admitted_parts(
        client.clone(),
        manager.unique_name().unwrap().as_str(),
        context.duplicate_notify().unwrap(),
    )
    .unwrap();
    let adapter = ManagedResolved::from_admitted_parts(
        client,
        resolved.unique_name().unwrap().to_string(),
        held,
    )
    .unwrap();
    let mut journal = Journal::open_root().unwrap();
    let mut lease = Lease::new(
        &context,
        &mut journal,
        adapter,
        retention,
        session.proof().unwrap().try_clone_to_owned().unwrap(),
        index,
    );
    let outcome = lease.apply();
    match case.as_str() {
        "success" => {
            assert_eq!(outcome, Outcome::Ready);
            assert!(lease.check_active());
            assert_eq!(lease.release(), Outcome::Clean);
        }
        "denial" | "mismatch" => {
            assert_eq!(outcome, Outcome::Clean);
        }
        "policy" => {
            assert_eq!(outcome, Outcome::Refused);
        }
        "timeout" => {
            assert_eq!(outcome, Outcome::RecoveryRequired);
            thread::sleep(Duration::from_millis(500));
        }
        "drop" => {
            assert_eq!(outcome, Outcome::Ready);
        }
        "apply_drift" => {
            assert_eq!(outcome, Outcome::RecoveryRequired);
        }
        "release_drift" => {
            assert_eq!(outcome, Outcome::Ready);
            state.lock().unwrap().drift = true;
            assert_eq!(lease.release(), Outcome::RecoveryRequired);
        }
        "dns_drift" | "release_dns_drift" => {
            assert_eq!(outcome, Outcome::Ready);
            assert!(lease.check_active());
            state.lock().unwrap().servers = vec![(2, vec![192, 0, 2, 53])];
            if case == "dns_drift" {
                assert!(!lease.check_active());
            }
            assert_eq!(lease.release(), Outcome::RecoveryRequired);
        }
        _ => unreachable!(),
    }
    drop(lease);
    drop(journal);
    drop(session);
    drop(producer);
    let reopened = Journal::open_root().unwrap();
    let state = state.lock().unwrap();
    match case.as_str() {
        "success" | "denial" | "mismatch" | "policy" => {
            assert!(!reopened.requires_recovery() && reopened.phase().is_none());
            assert!(state.stored.is_none());
            assert!(!tun_exists());
            assert_eq!(
                state.calls,
                if case == "policy" {
                    vec![]
                } else if case != "denial" {
                    vec!["dns", "domains", "route", "revert"]
                } else {
                    vec!["dns", "domains", "revert"]
                }
            );
            if case == "policy" {
                assert!(state.notifications.is_empty());
            }
        }
        "timeout" | "drop" | "apply_drift" | "release_drift" | "dns_drift"
        | "release_dns_drift" => {
            assert!(reopened.requires_recovery());
            assert!(state.stored.is_some());
            assert!(tun_exists());
            assert_eq!(
                state.servers,
                [(
                    2,
                    if case.ends_with("dns_drift") {
                        vec![192, 0, 2, 53]
                    } else {
                        vec![198, 18, 0, 2]
                    }
                )]
            );
            assert!(!state.calls.contains(&"revert"));
            assert!(
                state
                    .notifications
                    .iter()
                    .all(|s| !s.starts_with("FDSTOREREMOVE"))
            );
            if case == "timeout" {
                assert_eq!(state.calls, ["dns"]);
            }
            assert_eq!(
                state.notifications,
                [
                    "FDSTORE=1\nFDNAME=omavless-tun-lease\nFDPOLL=0",
                    "BARRIER=1"
                ]
            );
        }
        _ => unreachable!(),
    }
}
