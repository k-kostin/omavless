// SPDX-License-Identifier: MIT
// All peers and responses below are synthetic. No system/session bus discovery.
use super::*;
use std::{
    fs,
    os::unix::fs::FileTypeExt,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::Instant,
};
use tempfile::TempDir;
use zbus::{fdo, zvariant::OwnedObjectPath};

const SERVICE: &str = "org.freedesktop.resolve1";
const LINK_PATH: &str = "/org/freedesktop/resolve1/link/_42";
const PRIVATE_ERROR: &str = "https://private.invalid/token password=synthetic-private-key";

#[derive(Default)]
struct State {
    servers: Servers,
    domains: Domains,
    default_route: bool,
    calls: Vec<&'static str>,
    deny_domains: bool,
    delay_dns: bool,
    fail_after_dns: bool,
    mismatch_default: bool,
    interactive_seen: bool,
    delay_read: bool,
}

#[derive(Clone)]
struct MockManager(Arc<Mutex<State>>);

#[zbus::interface(name = "org.freedesktop.resolve1.Manager")]
impl MockManager {
    fn get_link(&self, ifindex: i32) -> fdo::Result<OwnedObjectPath> {
        if ifindex != 42 {
            return Err(fdo::Error::InvalidArgs(PRIVATE_ERROR.into()));
        }
        Ok(OwnedObjectPath::try_from(LINK_PATH).unwrap())
    }

    #[zbus(name = "SetLinkDNS")]
    fn set_link_dns(
        &self,
        ifindex: i32,
        servers: Servers,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> fdo::Result<()> {
        assert_eq!(ifindex, 42);
        assert_eq!(servers, [(2, FIXED_DNS.to_vec())]);
        let delay = self.0.lock().unwrap().delay_dns;
        if delay {
            thread::sleep(Duration::from_millis(250));
        }
        let mut state = self.0.lock().unwrap();
        state.interactive_seen |= header
            .primary()
            .flags()
            .contains(zbus::message::Flags::AllowInteractiveAuth);
        state.calls.push("SetLinkDNS");
        state.servers = servers;
        if state.fail_after_dns {
            return Err(fdo::Error::Failed(PRIVATE_ERROR.into()));
        }
        Ok(())
    }

    fn set_link_domains(&self, ifindex: i32, domains: Domains) -> fdo::Result<()> {
        assert_eq!(ifindex, 42);
        assert_eq!(domains, [(".".into(), true)]);
        let mut state = self.0.lock().unwrap();
        state.calls.push("SetLinkDomains");
        if state.deny_domains {
            return Err(fdo::Error::AccessDenied(PRIVATE_ERROR.into()));
        }
        state.domains = domains;
        Ok(())
    }

    fn set_link_default_route(&self, ifindex: i32, enabled: bool) {
        assert_eq!(ifindex, 42);
        assert!(enabled);
        let mut state = self.0.lock().unwrap();
        state.calls.push("SetLinkDefaultRoute");
        state.default_route = enabled && !state.mismatch_default;
    }

    fn revert_link(&self, ifindex: i32) {
        assert_eq!(ifindex, 42);
        let mut state = self.0.lock().unwrap();
        state.calls.push("RevertLink");
        state.servers.clear();
        state.domains.clear();
        state.default_route = false;
    }
}

#[derive(Clone)]
struct MockLink(Arc<Mutex<State>>);

#[zbus::interface(name = "org.freedesktop.resolve1.Link")]
impl MockLink {
    #[zbus(property, name = "DNS")]
    fn dns(&self) -> Servers {
        let delay = self.0.lock().unwrap().delay_read;
        if delay {
            thread::sleep(Duration::from_millis(250));
        }
        self.0.lock().unwrap().servers.clone()
    }
    #[zbus(property)]
    fn domains(&self) -> Domains {
        self.0.lock().unwrap().domains.clone()
    }
    #[zbus(property)]
    fn default_route(&self) -> bool {
        self.0.lock().unwrap().default_route
    }
}

struct Bus {
    directory: TempDir,
    child: Child,
}

impl Bus {
    fn start() -> Self {
        let directory = tempfile::Builder::new()
            .prefix("omavless-private-dbus-")
            .tempdir()
            .unwrap();
        let path = directory.path().join("bus");
        let address = format!("unix:path={}", path.display());
        let child = Command::new("dbus-daemon")
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
            .expect("private dbus-daemon fixture is required");
        let mut bus = Self { directory, child };
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_socket()) {
                return bus;
            }
            assert!(
                bus.child.try_wait().unwrap().is_none(),
                "private bus stopped"
            );
            assert!(Instant::now() < deadline, "private bus readiness timeout");
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn address(&self) -> String {
        format!("unix:path={}", self.directory.path().join("bus").display())
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Fixture {
    resolved: Resolved,
    state: Arc<Mutex<State>>,
    // Drop client and server before their own disposable bus.
    server: Connection,
    bus: Bus,
}

impl Fixture {
    fn new(state: State) -> Self {
        let bus = Bus::start();
        let state = Arc::new(Mutex::new(state));
        let server = zbus::blocking::connection::Builder::address(bus.address().as_str())
            .unwrap()
            .method_timeout(Duration::from_secs(2))
            .max_queued(8)
            .serve_at(MANAGER_PATH, MockManager(state.clone()))
            .unwrap()
            .serve_at(LINK_PATH, MockLink(state.clone()))
            .unwrap()
            .name(SERVICE)
            .unwrap()
            .build()
            .unwrap();
        let client = zbus::blocking::connection::Builder::address(bus.address().as_str())
            .unwrap()
            .method_timeout(Duration::from_secs(2))
            .max_queued(8)
            .build()
            .unwrap();
        let owner_reply = complete(
            Duration::from_secs(2),
            client.inner().call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "GetNameOwner",
                &(SERVICE,),
            ),
        )
        .unwrap();
        let owner: String = bounded_reply(&owner_reply).unwrap();
        assert_eq!(owner, server.unique_name().unwrap().as_str());
        let link_reply = complete(
            Duration::from_secs(2),
            client.inner().call_method(
                Some(owner.as_str()),
                MANAGER_PATH,
                Some(MANAGER_INTERFACE),
                "GetLink",
                &(42_i32,),
            ),
        )
        .unwrap();
        let link: OwnedObjectPath = bounded_reply(&link_reply).unwrap();
        assert_eq!(link.as_str(), LINK_PATH);
        let resolved = Resolved {
            connection: client,
            owner,
            link_path: link.to_string(),
            ifindex: 42,
            timeout: Duration::from_secs(2),
            uncertain_write: false,
        };
        Self {
            resolved,
            state,
            server,
            bus,
        }
    }

    fn apply(&mut self) -> Result<(), Error> {
        self.resolved.set_fixed_servers()?;
        self.resolved.set_root_routing_domain()?;
        self.resolved.set_default_route()?;
        self.resolved.verify_fixed_policy()
    }
}

#[test]
fn typed_wire_sequence_reads_and_reverts_only_fixed_synthetic_link() {
    let mut fixture = Fixture::new(State::default());
    assert!(!fixture.resolved.observe().unwrap().matches_fixed_policy());
    fixture.apply().unwrap();
    assert!(fixture.resolved.observe().unwrap().matches_fixed_policy());
    fixture.resolved.revert_pristine_link().unwrap();
    assert!(!fixture.resolved.observe().unwrap().matches_fixed_policy());
    let state = fixture.state.lock().unwrap();
    assert_eq!(
        state.calls,
        [
            "SetLinkDNS",
            "SetLinkDomains",
            "SetLinkDefaultRoute",
            "RevertLink"
        ]
    );
    assert!(!state.interactive_seen);
}

#[test]
fn acknowledgement_without_readback_match_is_not_success() {
    let mut fixture = Fixture::new(State {
        mismatch_default: true,
        ..State::default()
    });
    assert_eq!(fixture.apply(), Err(Error::ReadbackMismatch));
    // No speculative compensation; future transaction owner must choose it.
    assert_eq!(fixture.state.lock().unwrap().calls.len(), 3);
}

#[test]
fn refused_second_write_retains_partial_effect_and_safe_error() {
    let mut fixture = Fixture::new(State {
        deny_domains: true,
        ..State::default()
    });
    let error = fixture.apply().unwrap_err();
    assert_eq!(error, Error::AuthorizationRefused);
    assert!(!format!("{error:?} {error} {}", error.code()).contains("private"));
    assert_eq!(
        fixture.state.lock().unwrap().calls,
        ["SetLinkDNS", "SetLinkDomains"]
    );
    assert!(!fixture.state.lock().unwrap().servers.is_empty());
    // Only this disposable test's known pristine baseline authorizes Revert.
    fixture.resolved.revert_pristine_link().unwrap();
    assert!(fixture.state.lock().unwrap().servers.is_empty());
}

#[test]
fn post_effect_remote_error_cannot_be_treated_as_zero_write() {
    let mut fixture = Fixture::new(State {
        fail_after_dns: true,
        ..State::default()
    });
    assert_eq!(fixture.apply(), Err(Error::OutcomeUnknown));
    assert!(!fixture.state.lock().unwrap().servers.is_empty());
    assert_eq!(
        fixture.resolved.revert_pristine_link(),
        Err(Error::RecoveryRequired)
    );
    assert_eq!(fixture.state.lock().unwrap().calls, ["SetLinkDNS"]);
}

#[test]
fn timed_out_write_can_complete_late_and_blocks_compensation() {
    let mut fixture = Fixture::new(State {
        delay_dns: true,
        ..State::default()
    });
    fixture.resolved.timeout = Duration::from_millis(40);
    let started = Instant::now();
    assert_eq!(fixture.apply(), Err(Error::OutcomeUnknown));
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(
        fixture.resolved.revert_pristine_link(),
        Err(Error::RecoveryRequired)
    );
    thread::sleep(Duration::from_millis(300));
    assert!(!fixture.state.lock().unwrap().servers.is_empty());
    assert_eq!(
        fixture.resolved.set_fixed_servers(),
        Err(Error::RecoveryRequired)
    );
    assert_eq!(fixture.state.lock().unwrap().calls, ["SetLinkDNS"]);
}

#[test]
fn invalid_and_oversized_private_properties_fail_without_echo() {
    let fixture = Fixture::new(State::default());
    for invalid in [
        vec![(2, vec![1; 3])],
        vec![(2, vec![1; 4]); MAX_ENTRIES + 1],
    ] {
        fixture.state.lock().unwrap().servers = invalid;
        assert_eq!(fixture.resolved.observe().unwrap_err(), Error::InvalidReply);
    }
    fixture.state.lock().unwrap().servers.clear();
    for invalid in [
        vec![(PRIVATE_ERROR.repeat(400), true)],
        vec![("\n".into(), true)],
        vec![("a".into(), true); MAX_ENTRIES + 1],
    ] {
        fixture.state.lock().unwrap().domains = invalid;
        assert_eq!(fixture.resolved.observe().unwrap_err(), Error::InvalidReply);
    }
}

#[test]
fn observed_effective_values_have_no_debug_data_or_restore_api() {
    let observed = Observation {
        servers: vec![(2, vec![192, 0, 2, 1])],
        domains: vec![(PRIVATE_ERROR.into(), true)],
        default_route: false,
    };
    assert_eq!(
        format!("{observed:?}"),
        "Observation { private values omitted }"
    );
    assert!(!observed.matches_fixed_policy());
}

#[test]
fn service_name_replacement_is_not_adopted_mid_transaction() {
    let fixture = Fixture::new(State::default());
    fixture.server.release_name(SERVICE).unwrap();
    let other = zbus::blocking::connection::Builder::address(fixture.bus.address().as_str())
        .unwrap()
        .serve_at(
            LINK_PATH,
            MockLink(Arc::new(Mutex::new(State {
                servers: vec![(2, FIXED_DNS.to_vec())],
                domains: vec![(".".into(), true)],
                default_route: true,
                ..State::default()
            }))),
        )
        .unwrap()
        .name(SERVICE)
        .unwrap()
        .build()
        .unwrap();
    assert_ne!(other.unique_name(), fixture.server.unique_name());
    assert!(!fixture.resolved.observe().unwrap().matches_fixed_policy());
}

#[test]
fn peer_disconnect_produces_unknown_write_and_no_recovery_calls() {
    let mut fixture = Fixture::new(State::default());
    fixture.bus.child.kill().unwrap();
    fixture.bus.child.wait().unwrap();
    assert_eq!(
        fixture.resolved.set_fixed_servers(),
        Err(Error::OutcomeUnknown)
    );
    assert_eq!(
        fixture.resolved.revert_pristine_link(),
        Err(Error::RecoveryRequired)
    );
    assert!(fixture.state.lock().unwrap().calls.is_empty());
}

#[test]
fn read_deadline_is_bounded_without_poisoning_write_state() {
    let mut fixture = Fixture::new(State {
        delay_read: true,
        ..State::default()
    });
    fixture.resolved.timeout = Duration::from_millis(40);
    let started = Instant::now();
    assert_eq!(fixture.resolved.observe().unwrap_err(), Error::Unavailable);
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(!fixture.resolved.uncertain_write);
}

#[test]
fn readback_is_uncached_and_ipv6_shape_is_valid_but_not_fixed_policy() {
    let mut fixture = Fixture::new(State::default());
    fixture.apply().unwrap();
    fixture.state.lock().unwrap().servers = vec![(10, vec![0; 16])];
    let observed = fixture.resolved.observe().unwrap();
    assert!(!observed.matches_fixed_policy());
    assert_eq!(
        fixture.resolved.verify_fixed_policy(),
        Err(Error::ReadbackMismatch)
    );
}

#[test]
fn response_type_and_fd_rejection_precede_public_projection() {
    let call = Message::method_call(MANAGER_PATH, "SetLinkDNS")
        .unwrap()
        .build(&())
        .unwrap();
    let bad = Message::method_return(&call.header())
        .unwrap()
        .build(&true)
        .unwrap();
    assert_eq!(bounded_reply::<()>(&bad), Err(Error::InvalidReply));
    let file = tempfile::tempfile().unwrap();
    let descriptor = zbus::zvariant::Fd::from(&file);
    let with_fd = Message::method_return(&call.header())
        .unwrap()
        .build(&descriptor)
        .unwrap();
    assert_eq!(bounded_reply::<()>(&with_fd), Err(Error::InvalidReply));
}

#[test]
fn errors_have_fixed_bounded_english_fallbacks_only() {
    for error in [
        Error::Unavailable,
        Error::InvalidReply,
        Error::ReadbackMismatch,
        Error::AuthorizationRefused,
        Error::OutcomeUnknown,
        Error::RecoveryRequired,
    ] {
        let output = format!("{error} {error:?} {}", error.code());
        assert!(output.is_ascii() && output.len() < 200);
        assert!(!output.contains(PRIVATE_ERROR));
        assert!(!output.contains(SERVICE));
    }
}
