// SPDX-License-Identifier: MIT
//! Explicit ignored native-owner launcher, not an issuer or product endpoint.
//! A separately reviewed ROOT supervisor owns execution, fixtures and evidence.
use super::*;
use std::ffi::OsStr;

const UID: u32 = 1000;
const HOME: &str = "/home/kdk_vm";
const RUNTIME: &str = "/run/user/1000";
const OPT_IN: &str = "OMAVLESS_NATIVE_K1_VM";
const PROFILE: &str = "k1-native-fixed";

#[derive(Clone, Copy)]
enum Phase {
    Selection,
    Paths,
    OffIntent,
    ExistingSocket,
    Bind,
    Owner,
    CurrentOff,
    Roundtrip,
    FinalClosed,
    Unwind,
}
impl Phase {
    const fn token(self) -> &'static str {
        match self {
            Self::Selection => "selection",
            Self::Paths => "paths",
            Self::OffIntent => "off_intent",
            Self::ExistingSocket => "existing_socket",
            Self::Bind => "bind",
            Self::Owner => "owner",
            Self::CurrentOff => "current_off",
            Self::Roundtrip => "roundtrip",
            Self::FinalClosed => "final_closed",
            Self::Unwind => "unwind",
        }
    }
}

fn fixed_identity(
    uid: u32,
    euid: u32,
    gid: u32,
    egid: u32,
    home: Option<&OsStr>,
    runtime: Option<&OsStr>,
    opt_in: Option<&OsStr>,
) -> bool {
    [uid, euid, gid, egid] == [UID; 4]
        && home == Some(OsStr::new(HOME))
        && runtime == Some(OsStr::new(RUNTIME))
        && opt_in == Some(OsStr::new("1"))
}

fn selected() -> bool {
    fixed_identity(
        nix::unistd::getuid().as_raw(),
        nix::unistd::geteuid().as_raw(),
        nix::unistd::getgid().as_raw(),
        nix::unistd::getegid().as_raw(),
        std::env::var_os("HOME").as_deref(),
        std::env::var_os("XDG_RUNTIME_DIR").as_deref(),
        std::env::var_os(OPT_IN).as_deref(),
    ) && [
        "OMAVLESS_HOME",
        "OMAVLESS_MIHOMO",
        "XDG_CONFIG_HOME",
        "XDG_STATE_HOME",
        "XDG_CACHE_HOME",
    ]
    .into_iter()
    .all(|name| std::env::var_os(name).is_none())
}

fn run() -> Result<LifecycleOutcome, Phase> {
    if !selected() {
        return Err(Phase::Selection);
    }
    // Install whole reported-prefix custody before the first normal constructor.
    // This does not claim custody of backend descriptors it never reported.
    let mut custody = Custody {
        original: Some((
            None::<ProductionNativeOwner<NativeLifecycleHost>>,
            None::<RuntimeServer>,
        )),
    };
    let paths = RuntimePaths::current().map_err(|_| Phase::Paths)?;
    if paths != RuntimePaths::below(Path::new(RUNTIME)) {
        return Err(Phase::Paths);
    }
    let desired_paths = DesiredPaths::current().map_err(|_| Phase::Paths)?;
    let before =
        crate::desired::read_desired_snapshot(&desired_paths, UID).map_err(|_| Phase::OffIntent)?;
    if before.connected || !before.profile_id.is_empty() {
        return Err(Phase::OffIntent);
    }
    // The ordinary bind owns its singleton. Do not ask it to remove a stale or
    // unexplained preexisting socket on behalf of this development scenario.
    if !matches!(std::fs::symlink_metadata(&paths.socket),
                 Err(error) if error.kind() == std::io::ErrorKind::NotFound)
    {
        return Err(Phase::ExistingSocket);
    }
    let (owner, server) = custody.original.as_mut().ok_or(Phase::Paths)?;
    *server = Some(RuntimeServer::bind(paths.clone()).map_err(|_| Phase::Bind)?);
    // Actual current HOME, committed marker, current-manager consumed receipt,
    // migration lease, installed pair and startup reconciliation: no test host,
    // initialize_off_research, injected receipt, bare executor or fake TUN.
    *owner = Some(ProductionNativeOwner::current(&paths).map_err(|_| Phase::Owner)?);
    if owner
        .as_ref()
        .is_none_or(|value| value.actual() != ActualState::Disconnected)
        || crate::desired::read_desired_snapshot(&desired_paths, UID)
            .ok()
            .as_ref()
            != Some(&before)
    {
        return Err(Phase::CurrentOff);
    }
    // Move the pair together. Even an internal partial-construction invariant
    // failure puts the entire reported prefix back under custody before Err.
    let (owner, server) = match custody.original.take().ok_or(Phase::Paths)? {
        (Some(owner), Some(server)) => (owner, server),
        prefix => {
            custody.original = Some(prefix);
            return Err(Phase::Owner);
        }
    };
    // These exact originals now move into the already reviewed consuming guard.
    // Its closed issuer still refuses pre-Arm; this entry cannot change that.
    let outcome = owner
        .protected_developer_roundtrip(server, PROFILE)
        .map_err(|_| Phase::Roundtrip)?;
    if outcome.actual != ActualState::Disconnected {
        return Err(Phase::FinalClosed);
    }
    // The guarded roundtrip's original stop/empty/Disarm/final Status completed.
    // Empty outer custody may retire normally; no restoration of old bytes.
    drop(custody.original.take());
    Ok(outcome)
}

#[test]
#[ignore = "ROOT-only disposable VM actual native owner; issuer closed until separately reviewed qualification"]
fn installed_current_owner_protected_roundtrip() {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run));
    match result {
        Ok(Ok(_)) => println!("K1_NATIVE_CURRENT_OWNER_ROUNDTRIP_CLOSED"),
        other => {
            let phase = match other {
                Ok(Err(phase)) => phase,
                _ => Phase::Unwind,
            };
            eprintln!(
                "K1_NATIVE_CURRENT_OWNER_REFUSED_OR_UNKNOWN {}",
                phase.token()
            );
            // Never turn a failed constructor/roundtrip into a retry, cleanup,
            // original-child completion or guessed ownership. ROOT may choose
            // separately reviewed observation/recovery or VM administration.
            loop {
                std::thread::park();
            }
        }
    }
}

#[test]
fn native_launcher_selection_is_fixed_and_closed() {
    let home = Some(OsStr::new(HOME));
    let runtime = Some(OsStr::new(RUNTIME));
    let yes = Some(OsStr::new("1"));
    assert!(fixed_identity(UID, UID, UID, UID, home, runtime, yes));
    for ids in [
        [0, UID, UID, UID],
        [UID, 0, UID, UID],
        [UID, UID, 0, UID],
        [UID, UID, UID, 0],
    ] {
        assert!(!fixed_identity(
            ids[0], ids[1], ids[2], ids[3], home, runtime, yes
        ));
    }
    assert!(!fixed_identity(
        UID,
        UID,
        UID,
        UID,
        Some(OsStr::new("/tmp/home")),
        runtime,
        yes
    ));
    assert!(!fixed_identity(
        UID,
        UID,
        UID,
        UID,
        home,
        Some(OsStr::new("/run/user/1001")),
        yes
    ));
    for token in [None, Some(OsStr::new("0")), Some(OsStr::new("true"))] {
        assert!(!fixed_identity(UID, UID, UID, UID, home, runtime, token));
    }
}
