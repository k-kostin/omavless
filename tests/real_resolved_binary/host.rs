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
use zbus::zvariant::OwnedObjectPath;
include!("observer.rs");

const UNIT: &str = "/org/freedesktop/systemd1/unit/omavless_2ddns_2dbroker_2eservice";
const ADDRESS: &str = "unix:path=/run/dbus/system_bus_socket";
type Servers = Vec<(i32, Vec<u8>)>;
type Domains = Vec<(String, bool)>;
type Entry = (String, u32, u32, u32, u64, u32, u32, String, u32);

#[derive(Default)]
struct State {
    broker: u32,
    index: i32,
    stored: Option<OwnedFd>,
    baseline: Option<Observation>,
    reset: Option<Observation>,
    reset_while_held: bool,
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
fn unrelated_index() -> i32 {
    let socket = net::socket(net::AddressFamily::INET, net::SocketType::DGRAM, None).unwrap();
    i32::try_from(net::netdevice::name_to_index(&socket, "scope-other").unwrap()).unwrap()
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

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 4);
    assert!(matches!(
        args[1].as_str(),
        "success" | "denial" | "revert-denial" | "owner-loss"
    ));
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    assert_eq!(fs::metadata("/").unwrap().uid(), 0);
    let broker: u32 = args[2].parse().unwrap();
    assert!(broker > 1 && broker != std::process::id());
    let real = RealResolver::new(args[3].parse().unwrap());
    let unrelated_index = unrelated_index();
    let unrelated = real.seed_unrelated(unrelated_index);
    let mut monitor = Some(Monitor::start(real.clone(), broker));
    let monitored_effects = monitor.as_ref().unwrap().effects.clone();
    let observer = real.clone();
    let state = Arc::new(Mutex::new(State {
        broker,
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
                    let deadline = std::time::Instant::now() + Duration::from_secs(2);
                    let baseline = loop {
                        if let Some(value) = observer.observe(actual) {
                            break value;
                        }
                        assert!(std::time::Instant::now() < deadline);
                        thread::sleep(Duration::from_millis(5));
                    };
                    assert!(baseline.empty() && baseline.tls == "no" && baseline.dnssec == "no");
                    state.baseline = Some(baseline);
                    state.received_tun = true;
                    state.stored = Some(fd);
                }
                "BARRIER=1" => assert_eq!(fds.len(), 1),
                "FDSTOREREMOVE=1\nFDNAME=omavless-tun-lease" => {
                    assert!(fds.is_empty());
                    assert_eq!(phase().as_deref(), Some("cleanup_verified"));
                    let reset = observer.observe(state.index).unwrap();
                    assert_eq!(Some(&reset), state.baseline.as_ref());
                    assert!(reset.empty() && index() == Some(state.index));
                    let held =
                        HeldTun::admit(rustix::io::dup(state.stored.as_ref().unwrap()).unwrap())
                            .unwrap();
                    held.recheck().unwrap();
                    state.reset = Some(reset);
                    state.reset_while_held = true;
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
    println!("fixture-ready");
    std::io::stdout().flush().unwrap();
    for line in std::io::stdin().lock().lines() {
        let line = line.unwrap();
        assert!(line == "snapshot" || line == "finish");
        let finalized = line == "finish";
        if finalized {
            monitor.take().unwrap().stop();
        }
        let state = state.lock().unwrap();
        let observation = if state.stored.is_some() {
            real.observe(state.index)
        } else {
            state.reset.clone()
        };
        let same_owner = real.same_owner();
        let unrelated_preserved =
            same_owner && real.observe(unrelated_index).as_ref() == Some(&unrelated);
        let effects = monitored_effects.lock().unwrap();
        let snapshot = json!({"notifications": state.notifications,
            "stored": state.stored.is_some(), "received_tun": state.received_tun,
            "actual_fixed_policy": observation.as_ref().is_some_and(|value|
                value.active() && state.baseline.as_ref().is_some_and(|baseline|
                    value.same_non_dns(baseline))),
            "observation": observation, "reset_while_held": state.reset_while_held,
            "baseline": state.baseline, "unrelated_baseline": unrelated,
            "owner_pinned": same_owner, "unrelated_preserved": unrelated_preserved,
            "effects": *effects, "phase": phase(), "tun_exists": index().is_some(),
            "notify_alive": !worker.is_finished(),
            "observer_finalized": finalized,
            "monitor_alive": monitor.as_ref().is_some_and(|m| !m.worker.is_finished())});
        println!("{snapshot}");
        std::io::stdout().flush().unwrap();
    }
    stop.store(true, Ordering::Release);
    worker.join().unwrap();
    if let Some(monitor) = monitor {
        monitor.stop();
    }
    drop(manager);
}
