// SPDX-License-Identifier: MIT
// Test-only external host fixture. Compile as an example in a private export
// of the pinned c4 DNS workspace; never link this into the actual broker ELF.
use omavless_dns_tun::HeldTun;
use rustix::net::{self, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags};
use serde_json::json;
use std::{
    fs,
    io::{BufRead, IoSliceMut, Write},
    mem::MaybeUninit,
    os::{fd::OwnedFd, unix::net::UnixDatagram},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
use zbus::{fdo, zvariant::OwnedObjectPath};

const UNIT: &str = "/org/freedesktop/systemd1/unit/omavless_2ddns_2dbroker_2eservice";
const ADDRESS: &str = "unix:path=/run/dbus/system_bus_socket";
type Servers = Vec<(i32, Vec<u8>)>;
type Domains = Vec<(String, bool)>;
type Entry = (String, u32, u32, u32, u64, u32, u32, String, u32);

#[derive(Default)]
struct State {
    broker: u32,
    deny: bool,
    index: i32,
    stored: Option<OwnedFd>,
    servers: Servers,
    domains: Domains,
    route: bool,
    calls: Vec<&'static str>,
    notifications: Vec<String>,
    received_tun: bool,
}
fn phase() -> Option<String> {
    match fs::read("/run/omavless-dns/private/lease.json") {
        Ok(bytes) => {
            assert!(bytes.len() <= 512);
            let record: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            Some(record["phase"].as_str().unwrap().to_owned())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("private journal: {error:?}"),
    }
}
fn index() -> Option<i32> {
    let socket = net::socket(net::AddressFamily::INET, net::SocketType::DGRAM, None).unwrap();
    net::netdevice::name_to_index(&socket, "Meta")
        .ok()
        .map(|value| i32::try_from(value).unwrap())
}
struct Manager(Arc<Mutex<State>>);
#[zbus::interface(name = "org.freedesktop.systemd1.Service")]
impl Manager {
    #[zbus(property, name = "MainPID")]
    fn main_pid(&self) -> u32 {
        self.0.lock().unwrap().broker
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
        // Actual received object, not a copied expected identity/count.
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
    async fn get_link(
        &self,
        requested: i32,
        #[zbus(object_server)] server: &zbus::ObjectServer,
    ) -> fdo::Result<OwnedObjectPath> {
        assert_eq!(Some(requested), index());
        {
            let mut state = self.0.lock().unwrap();
            assert!(state.index == 0 || state.index == requested);
            state.index = requested;
        }
        let path = format!("/org/freedesktop/resolve1/link/_3{requested}");
        server.at(path.clone(), Link(self.0.clone())).await.unwrap();
        Ok(OwnedObjectPath::try_from(path).unwrap())
    }
    #[zbus(name = "SetLinkDNS")]
    fn set_link_dns(
        &self,
        requested: i32,
        servers: Servers,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> fdo::Result<()> {
        assert!(
            !header
                .primary()
                .flags()
                .contains(zbus::message::Flags::AllowInteractiveAuth)
        );
        assert_eq!(phase().as_deref(), Some("applying"));
        let mut state = self.0.lock().unwrap();
        assert_eq!(requested, state.index);
        assert_eq!(servers, [(2, vec![198, 18, 0, 2])]);
        assert!(state.stored.is_some() && state.received_tun);
        state.calls.push("dns");
        if state.deny {
            return Err(fdo::Error::AccessDenied("fixed fixture denial".into()));
        }
        state.servers = servers;
        Ok(())
    }
    fn set_link_domains(&self, requested: i32, domains: Domains) {
        let mut state = self.0.lock().unwrap();
        assert_eq!(requested, state.index);
        assert_eq!(domains, [(".".into(), true)]);
        state.calls.push("domains");
        state.domains = domains;
    }
    fn set_link_default_route(&self, requested: i32, enabled: bool) {
        let mut state = self.0.lock().unwrap();
        assert_eq!(requested, state.index);
        assert!(enabled);
        state.calls.push("route");
        state.route = enabled;
    }
    fn revert_link(&self, requested: i32) {
        assert_eq!(phase().as_deref(), Some("releasing"));
        let mut state = self.0.lock().unwrap();
        assert_eq!(requested, state.index);
        assert!(state.stored.is_some());
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
        "yes"
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
        "no"
    }
    #[zbus(property, name = "DNSSECNegativeTrustAnchors")]
    fn nta(&self) -> Vec<String> {
        vec![]
    }
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3);
    assert!(matches!(args[1].as_str(), "success" | "denial"));
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    assert_eq!(fs::metadata("/").unwrap().uid(), 0);
    let broker: u32 = args[2].parse().unwrap();
    assert!(broker > 1 && broker != std::process::id());
    let state = Arc::new(Mutex::new(State {
        broker,
        deny: args[1] == "denial",
        ..State::default()
    }));
    let receiver = UnixDatagram::bind("/run/systemd/notify").unwrap();
    use std::os::unix::fs::MetadataExt;
    net::sockopt::set_socket_passcred(&receiver, true).unwrap();
    receiver
        .set_read_timeout(Some(Duration::from_millis(50)))
        .unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    let notify_state = state.clone();
    let worker = thread::spawn(move || {
        while !stopped.load(Ordering::Acquire) {
            let mut bytes = [0; 256];
            let mut storage = [MaybeUninit::uninit();
                rustix::cmsg_aligned_space!(ScmRights(1), ScmCredentials(1))];
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
            let mut fds = vec![];
            let mut credentials = vec![];
            let mut invalid = false;
            // Exhaust ancillary drain even on rejected frames, closing all rights.
            for item in ancillary.drain() {
                match item {
                    RecvAncillaryMessage::ScmRights(rights) => fds.extend(rights),
                    RecvAncillaryMessage::ScmCredentials(value) => credentials.push(value),
                    _ => invalid = true,
                }
            }
            assert!(
                !invalid
                    && !received
                        .flags
                        .intersects(net::ReturnFlags::TRUNC | net::ReturnFlags::CTRUNC)
            );
            assert_eq!(credentials.len(), 1);
            assert_eq!(
                credentials[0].pid.as_raw_pid(),
                i32::try_from(broker).unwrap()
            );
            assert_eq!(credentials[0].uid.as_raw(), 0);
            assert_eq!(credentials[0].gid.as_raw(), 0);
            let message = std::str::from_utf8(&bytes[..received.bytes]).unwrap();
            let mut state = notify_state.lock().unwrap();
            state.notifications.push(message.to_owned());
            match message {
                "READY=1" => assert!(fds.is_empty()),
                "FDSTORE=1\nFDNAME=omavless-tun-lease\nFDPOLL=0" => {
                    assert_eq!(fds.len(), 1);
                    assert!(state.stored.is_none());
                    let fd = fds.pop().unwrap();
                    let held = HeldTun::admit(rustix::io::dup(&fd).unwrap()).unwrap();
                    held.recheck().unwrap();
                    let actual = i32::try_from(held.interface_index()).unwrap();
                    assert_eq!(Some(actual), index());
                    assert!(state.index == 0 || state.index == actual);
                    state.index = actual;
                    state.received_tun = true;
                    state.stored = Some(fd);
                }
                "BARRIER=1" => assert_eq!(fds.len(), 1),
                "FDSTOREREMOVE=1\nFDNAME=omavless-tun-lease" => {
                    assert!(fds.is_empty());
                    assert_eq!(phase().as_deref(), Some("cleanup_verified"));
                    assert!(state.servers.is_empty() && state.domains.is_empty() && !state.route);
                    assert!(state.stored.take().is_some());
                }
                _ => panic!("non-fixed notification"),
            }
        }
    });
    let manager = zbus::blocking::connection::Builder::address(ADDRESS)
        .unwrap()
        .serve_at(UNIT, Manager(state.clone()))
        .unwrap()
        .name("org.freedesktop.systemd1")
        .unwrap()
        .build()
        .unwrap();
    let resolved = zbus::blocking::connection::Builder::address(ADDRESS)
        .unwrap()
        .serve_at("/org/freedesktop/resolve1", Resolve(state.clone()))
        .unwrap()
        .name("org.freedesktop.resolve1")
        .unwrap()
        .build()
        .unwrap();
    println!("fixture-ready");
    std::io::stdout().flush().unwrap();
    for line in std::io::stdin().lock().lines() {
        assert_eq!(line.unwrap(), "snapshot");
        let state = state.lock().unwrap();
        let snapshot = json!({"calls": state.calls, "notifications": state.notifications,
            "stored": state.stored.is_some(), "received_tun": state.received_tun,
            "servers": state.servers, "domains": state.domains, "route": state.route,
            "phase": phase(), "tun_exists": index().is_some(), "notify_alive": !worker.is_finished()});
        println!("{snapshot}");
        std::io::stdout().flush().unwrap();
    }
    stop.store(true, Ordering::Release);
    worker.join().unwrap();
    drop(resolved);
    drop(manager);
}
