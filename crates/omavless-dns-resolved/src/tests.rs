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
const LINK_PATH: &str = "/org/freedesktop/resolve1/link/_342";
const PRIVATE_ERROR: &str = "https://private.invalid/token password=synthetic-private-key";

#[test]
fn expired_operation_does_not_poll_or_dispatch_and_budget_is_bounded() {
    let polled = std::cell::Cell::new(false);
    assert!(
        complete_until(Instant::now(), async {
            polled.set(true);
            Ok(())
        })
        .is_err()
    );
    assert!(!polled.get());
    let mut fixture = Fixture::new(State::default());
    assert_eq!(
        fixture
            .resolved
            .set_deadline(Instant::now() + Duration::from_secs(31)),
        Err(Error::Unavailable)
    );
    fixture.resolved.set_deadline(Instant::now()).unwrap();
    assert!(fixture.resolved.observe().is_err());
    assert_eq!(
        fixture.resolved.set_fixed_servers(),
        Err(Error::OutcomeUnknown)
    );
    fixture
        .resolved
        .set_deadline(Instant::now() + Duration::from_secs(5))
        .unwrap();
    assert_eq!(
        fixture.resolved.revert_pristine_link(),
        Err(Error::RecoveryRequired)
    );
    assert!(fixture.state.lock().unwrap().calls.is_empty());
}

#[test]
fn absolute_write_budget_does_not_reset_per_call_or_allow_compensation() {
    let mut fixture = Fixture::new(State {
        delay_dns: true,
        ..State::default()
    });
    fixture
        .resolved
        .set_deadline(Instant::now() + Duration::from_millis(40))
        .unwrap();
    let started = Instant::now();
    assert_eq!(
        fixture.resolved.set_fixed_servers(),
        Err(Error::OutcomeUnknown)
    );
    assert!(started.elapsed() < Duration::from_secs(1));
    fixture
        .resolved
        .set_deadline(Instant::now() + Duration::from_secs(5))
        .unwrap();
    assert_eq!(
        fixture.resolved.revert_pristine_link(),
        Err(Error::RecoveryRequired)
    );
    thread::sleep(Duration::from_millis(300));
    assert_eq!(fixture.state.lock().unwrap().calls, ["SetLinkDNS"]);
}

#[test]
fn sequential_observations_share_one_absolute_budget() {
    let mut fixture = Fixture::new(State {
        delay_read: true,
        ..State::default()
    });
    let started = Instant::now();
    fixture
        .resolved
        .set_deadline(started + Duration::from_millis(400))
        .unwrap();
    fixture.resolved.observe().unwrap();
    assert_eq!(fixture.resolved.observe().unwrap_err(), Error::Unavailable);
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(fixture.state.lock().unwrap().calls.is_empty());
}

#[derive(Default)]
pub(super) struct State {
    servers: Servers,
    extended_override: Option<ExtendedServers>,
    domains: Domains,
    default_route: bool,
    unchanged: Unchanged,
    reset_unchanged: Option<Unchanged>,
    pub(super) calls: Vec<&'static str>,
    pub(super) wrong_link_path: bool,
    deny_domains: bool,
    delay_dns: bool,
    fail_after_dns: bool,
    mismatch_default: bool,
    interactive_seen: bool,
    delay_read: bool,
}

impl Default for Unchanged {
    fn default() -> Self {
        Self {
            llmnr: "yes".into(),
            mdns: "no".into(),
            dns_over_tls: "no".into(),
            dnssec: "no".into(),
            negative_trust_anchors: vec![],
        }
    }
}

fn ownership() -> ExclusiveOwnership {
    // Test fixture alone owns its mock. There is no public production factory.
    ExclusiveOwnership { _sealed: () }
}

#[derive(Clone)]
struct MockManager(Arc<Mutex<State>>);

#[zbus::interface(name = "org.freedesktop.resolve1.Manager")]
impl MockManager {
    fn get_link(&self, ifindex: i32) -> fdo::Result<OwnedObjectPath> {
        if ifindex != 42 {
            return Err(fdo::Error::InvalidArgs(PRIVATE_ERROR.into()));
        }
        Ok(
            OwnedObjectPath::try_from(if self.0.lock().unwrap().wrong_link_path {
                "/org/freedesktop/resolve1/link/_343"
            } else {
                LINK_PATH
            })
            .unwrap(),
        )
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
        if let Some(reset) = state.reset_unchanged.clone() {
            state.unchanged = reset;
        }
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
    #[zbus(property, name = "DNSEx")]
    fn dns_ex(&self) -> ExtendedServers {
        let state = self.0.lock().unwrap();
        state.extended_override.clone().unwrap_or_else(|| {
            state
                .servers
                .iter()
                .map(|(family, bytes)| (*family, bytes.clone(), 0, String::new()))
                .collect()
        })
    }
    #[zbus(property)]
    fn domains(&self) -> Domains {
        self.0.lock().unwrap().domains.clone()
    }
    #[zbus(property)]
    fn default_route(&self) -> bool {
        self.0.lock().unwrap().default_route
    }
    #[zbus(property, name = "LLMNR")]
    fn llmnr(&self) -> String {
        self.0.lock().unwrap().unchanged.llmnr.clone()
    }
    #[zbus(property, name = "MulticastDNS")]
    fn mdns(&self) -> String {
        self.0.lock().unwrap().unchanged.mdns.clone()
    }
    #[zbus(property, name = "DNSOverTLS")]
    fn dns_over_tls(&self) -> String {
        self.0.lock().unwrap().unchanged.dns_over_tls.clone()
    }
    #[zbus(property, name = "DNSSEC")]
    fn dnssec(&self) -> String {
        self.0.lock().unwrap().unchanged.dnssec.clone()
    }
    #[zbus(property, name = "DNSSECNegativeTrustAnchors")]
    fn negative_trust_anchors(&self) -> Vec<String> {
        self.0
            .lock()
            .unwrap()
            .unchanged
            .negative_trust_anchors
            .clone()
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

pub(super) struct Fixture {
    pub(super) resolved: Resolved,
    pub(super) state: Arc<Mutex<State>>,
    // Drop client and server before their own disposable bus.
    pub(super) server: Connection,
    bus: Bus,
}

impl Fixture {
    pub(super) fn new(state: State) -> Self {
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
            deadline: None,
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
        extended_servers: vec![(2, vec![192, 0, 2, 1], 853, "private.invalid".into())],
        domains: vec![(PRIVATE_ERROR.into(), true)],
        default_route: false,
        unchanged: Unchanged::default(),
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
        Error::NonPristine,
        Error::UnsupportedPolicy,
        Error::LeaseLost,
        Error::OwnershipChanged,
    ] {
        let output = format!("{error} {error:?} {}", error.code());
        assert!(output.is_ascii() && output.len() < 200);
        assert!(!output.contains(PRIVATE_ERROR));
        assert!(!output.contains(SERVICE));
    }
}

#[test]
fn sealed_baseline_checks_apply_and_whole_link_reset_without_restoring_snapshot() {
    let mut fixture = Fixture::new(State::default());
    let baseline = fixture.resolved.capture_baseline(&ownership()).unwrap();
    assert_eq!(
        format!("{baseline:?}"),
        "Baseline { private values omitted }"
    );
    fixture.apply().unwrap();
    fixture
        .resolved
        .verify_policy_preserves_baseline(&baseline)
        .unwrap();
    assert_eq!(
        fixture.resolved.verify_reset(&baseline),
        Err(Error::ReadbackMismatch)
    );
    fixture.resolved.revert_pristine_link().unwrap();
    fixture.resolved.verify_reset(&baseline).unwrap();
}

#[test]
fn nonempty_baselines_refuse_before_any_mutation() {
    for state in [
        State {
            servers: vec![(2, vec![192, 0, 2, 1])],
            ..State::default()
        },
        State {
            domains: vec![("example.invalid".into(), false)],
            ..State::default()
        },
        State {
            default_route: true,
            ..State::default()
        },
        State {
            unchanged: Unchanged {
                negative_trust_anchors: vec!["example.invalid".into()],
                ..Unchanged::default()
            },
            ..State::default()
        },
    ] {
        let fixture = Fixture::new(state);
        assert_eq!(
            fixture.resolved.capture_baseline(&ownership()).unwrap_err(),
            Error::NonPristine
        );
        assert!(fixture.state.lock().unwrap().calls.is_empty());
    }
}

#[test]
fn tls_and_dnssec_modes_require_policy_review_not_extra_writes() {
    for (tls, dnssec) in [
        ("yes", "no"),
        ("opportunistic", "no"),
        ("no", "yes"),
        ("no", "allow-downgrade"),
    ] {
        let fixture = Fixture::new(State {
            unchanged: Unchanged {
                dns_over_tls: tls.into(),
                dnssec: dnssec.into(),
                ..Unchanged::default()
            },
            ..State::default()
        });
        assert_eq!(
            fixture.resolved.capture_baseline(&ownership()).unwrap_err(),
            Error::UnsupportedPolicy
        );
        assert!(fixture.state.lock().unwrap().calls.is_empty());
    }
}

#[test]
fn extended_dns_fields_are_not_lost_in_legacy_projection() {
    let mut fixture = Fixture::new(State::default());
    fixture.apply().unwrap();
    for (port, name) in [(853, ""), (53, ""), (0, "private.invalid")] {
        fixture.state.lock().unwrap().extended_override =
            Some(vec![(2, FIXED_DNS.to_vec(), port, name.into())]);
        assert_eq!(
            fixture.resolved.verify_fixed_policy(),
            Err(Error::ReadbackMismatch)
        );
    }
    fixture.state.lock().unwrap().extended_override =
        Some(vec![(2, vec![192, 0, 2, 1], 0, String::new())]);
    assert_eq!(fixture.resolved.observe().unwrap_err(), Error::InvalidReply);
}

#[test]
fn expanded_fields_reject_oversize_invalid_enums_domains_and_shapes_privately() {
    let fixture = Fixture::new(State::default());
    for field in ["llmnr", "mdns", "tls", "dnssec"] {
        for value in [PRIVATE_ERROR.to_owned(), "no\n".into(), "yes".repeat(6000)] {
            let mut unchanged = Unchanged::default();
            match field {
                "llmnr" => unchanged.llmnr = value,
                "mdns" => unchanged.mdns = value,
                "tls" => unchanged.dns_over_tls = value,
                _ => unchanged.dnssec = value,
            }
            fixture.state.lock().unwrap().unchanged = unchanged;
            assert_eq!(fixture.resolved.observe().unwrap_err(), Error::InvalidReply);
        }
    }
    for anchors in [
        vec!["x".into(); MAX_ENTRIES + 1],
        vec![PRIVATE_ERROR.into()],
        vec!["x".repeat(64)],
        vec!["x..invalid".into()],
        vec!["юникод.invalid".into()],
    ] {
        fixture.state.lock().unwrap().unchanged = Unchanged {
            negative_trust_anchors: anchors,
            ..Unchanged::default()
        };
        assert_eq!(fixture.resolved.observe().unwrap_err(), Error::InvalidReply);
    }
    fixture.state.lock().unwrap().unchanged = Unchanged::default();
    fixture.state.lock().unwrap().servers = vec![(2, vec![192, 0, 2, 1])];
    for extended in [
        vec![(2, vec![192, 0, 2, 1], 0, "x".repeat(254))],
        vec![(2, vec![192, 0, 2, 1], 0, PRIVATE_ERROR.into())],
        vec![(10, vec![0; 4], 0, String::new())],
        vec![(2, vec![192, 0, 2, 1], 0, String::new()); MAX_ENTRIES + 1],
    ] {
        fixture.state.lock().unwrap().extended_override = Some(extended);
        assert_eq!(fixture.resolved.observe().unwrap_err(), Error::InvalidReply);
    }
}

#[test]
fn reset_acknowledgement_cannot_hide_changes_to_unmodified_settings() {
    for field in ["llmnr", "mdns", "tls", "dnssec", "nta"] {
        let mut fixture = Fixture::new(State::default());
        let baseline = fixture.resolved.capture_baseline(&ownership()).unwrap();
        fixture.apply().unwrap();
        let mut reset = Unchanged::default();
        match field {
            "llmnr" => reset.llmnr = "no".into(),
            "mdns" => reset.mdns = "yes".into(),
            "tls" => reset.dns_over_tls = "yes".into(),
            "dnssec" => reset.dnssec = "yes".into(),
            _ => reset.negative_trust_anchors = vec!["example.invalid".into()],
        }
        fixture.state.lock().unwrap().reset_unchanged = Some(reset);
        fixture.resolved.revert_pristine_link().unwrap();
        assert_eq!(
            fixture.resolved.verify_reset(&baseline),
            Err(Error::ReadbackMismatch)
        );
    }
}

#[test]
fn baseline_binding_and_unknown_write_cannot_be_bypassed_by_readback() {
    let mut fixture = Fixture::new(State::default());
    let mut baseline = fixture.resolved.capture_baseline(&ownership()).unwrap();
    baseline.ifindex += 1;
    assert_eq!(
        fixture.resolved.verify_reset(&baseline),
        Err(Error::OwnershipChanged)
    );
    baseline.ifindex -= 1;
    baseline.owner.push_str("-different");
    assert_eq!(
        fixture.resolved.verify_reset(&baseline),
        Err(Error::OwnershipChanged)
    );
    let baseline = fixture.resolved.capture_baseline(&ownership()).unwrap();
    fixture.state.lock().unwrap().fail_after_dns = true;
    assert_eq!(
        fixture.resolved.set_fixed_servers(),
        Err(Error::OutcomeUnknown)
    );
    fixture.state.lock().unwrap().servers.clear();
    assert_eq!(
        fixture.resolved.verify_reset(&baseline),
        Err(Error::RecoveryRequired)
    );
    assert_eq!(
        fixture.resolved.capture_baseline(&ownership()).unwrap_err(),
        Error::RecoveryRequired
    );
}

#[test]
fn foreign_untouched_setting_drift_blocks_apply_readback_and_whole_link_reset() {
    for field in ["llmnr", "mdns", "tls", "dnssec", "nta"] {
        let mut fixture = Fixture::new(State::default());
        let baseline = fixture.resolved.capture_baseline(&ownership()).unwrap();
        fixture.apply().unwrap();
        {
            let mut state = fixture.state.lock().unwrap();
            match field {
                "llmnr" => state.unchanged.llmnr = "no".into(),
                "mdns" => state.unchanged.mdns = "yes".into(),
                "tls" => state.unchanged.dns_over_tls = "yes".into(),
                "dnssec" => state.unchanged.dnssec = "yes".into(),
                _ => state.unchanged.negative_trust_anchors = vec!["example.invalid".into()],
            }
        }
        assert_eq!(
            fixture.resolved.verify_policy_preserves_baseline(&baseline),
            Err(Error::OwnershipChanged)
        );
        assert_eq!(
            fixture.resolved.revert_preserving_untouched(&baseline),
            Err(Error::OwnershipChanged)
        );
        assert!(!fixture.state.lock().unwrap().calls.contains(&"RevertLink"));
    }
}

#[test]
fn unknown_write_cannot_be_fenced_by_unchanged_settings_read_before_reset() {
    let mut fixture = Fixture::new(State::default());
    let baseline = fixture.resolved.capture_baseline(&ownership()).unwrap();
    fixture.state.lock().unwrap().fail_after_dns = true;
    assert_eq!(
        fixture.resolved.set_fixed_servers(),
        Err(Error::OutcomeUnknown)
    );
    assert_eq!(
        fixture.resolved.revert_preserving_untouched(&baseline),
        Err(Error::RecoveryRequired)
    );
    assert_eq!(fixture.state.lock().unwrap().calls, ["SetLinkDNS"]);
}

#[test]
fn maximum_bounded_domains_and_recognized_multicast_modes_are_observable() {
    let fixture = Fixture::new(State {
        domains: vec![
            (
                format!(
                    "{}.{}.{}.{}",
                    "a".repeat(63),
                    "b".repeat(63),
                    "c".repeat(63),
                    "d".repeat(61)
                ),
                true
            );
            MAX_ENTRIES
        ],
        unchanged: Unchanged {
            llmnr: "resolve".into(),
            mdns: "resolve".into(),
            negative_trust_anchors: vec!["example.invalid.".into(); MAX_ENTRIES],
            ..Unchanged::default()
        },
        ..State::default()
    });
    assert!(fixture.resolved.observe().is_ok());
}
