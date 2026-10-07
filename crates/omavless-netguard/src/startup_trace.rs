//! Fixed opt-in developer startup frames, NEVER authority or raw diagnostics.
//! Exactly one STDERR descriptor write per frame, no retries. A failed/short
//! write parks in place without unwinding any currently held resource graph.
use std::sync::atomic::{AtomicBool, Ordering};

static ACTIVE: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
pub(crate) enum Phase {
    Enter,
    Anchor,
    InheritedAnchors,
    BusOwner,
    Rpc,
    OriginFiles,
    EffectiveUnit,
    OriginalNamespaces,
    CreatorOpen,
    CreatorAssembled,
    StateOpen,
    ControlPublished,
    RecoveryPublished,
    AuthorityAssembled,
    Ready,
    Panic,
}
impl Phase {
    fn label(self) -> &'static [u8] {
        match self {
            Self::Enter => b"ENTER",
            Self::Anchor => b"ANCHOR",
            Self::InheritedAnchors => b"INHERITED_ANCHORS",
            Self::BusOwner => b"BUS_OWNER",
            Self::Rpc => b"RPC",
            Self::OriginFiles => b"ORIGIN_FILES",
            Self::EffectiveUnit => b"EFFECTIVE_UNIT",
            Self::OriginalNamespaces => b"ORIGINAL_NAMESPACES",
            Self::CreatorOpen => b"CREATOR_OPEN",
            Self::CreatorAssembled => b"CREATOR_ASSEMBLED",
            Self::StateOpen => b"STATE_OPEN",
            Self::ControlPublished => b"CONTROL_PUBLISHED",
            Self::RecoveryPublished => b"RECOVERY_PUBLISHED",
            Self::AuthorityAssembled => b"AUTHORITY_ASSEMBLED",
            Self::Ready => b"READY",
            Self::Panic => b"PANIC",
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) enum Event {
    Begin,
    Pass,
    Refused,
    Missing,
    Type,
    Mismatch,
    Decode,
    Unavailable,
    Deadline,
}
impl Event {
    fn label(self) -> &'static [u8] {
        match self {
            Self::Begin => b"BEGIN",
            Self::Pass => b"PASS",
            Self::Refused => b"REFUSED",
            Self::Missing => b"MISSING",
            Self::Type => b"TYPE",
            Self::Mismatch => b"MISMATCH",
            Self::Decode => b"DECODE",
            Self::Unavailable => b"UNAVAILABLE",
            Self::Deadline => b"DEADLINE",
        }
    }
}
// Stable compile-time IDs, not manager-supplied names or values. Unknown
// internal key is ID99; source call sites use only these literal keys.
pub(crate) const PROPERTY_KEYS: &[&str] = &[
    "Id",
    "LoadState",
    "FragmentPath",
    "SourcePath",
    "Transient",
    "DropInPaths",
    "ActiveState",
    "InvocationID",
    "Type",
    "User",
    "Group",
    "SupplementaryGroups",
    "ExecStart",
    "ExecStartPre",
    "ExecStartPost",
    "ExecReload",
    "ExecStop",
    "ExecStopPost",
    "ControlGroup",
    "MainPID",
    "ExecMainPID",
    "StandardInput",
    "StandardOutput",
    "StandardError",
    "KillMode",
    "RuntimeDirectoryPreserve",
    "RuntimeDirectory",
    "RuntimeDirectoryMode",
    "NoNewPrivileges",
    "RestrictNamespaces",
    "CapabilityBoundingSet",
    "AmbientCapabilities",
    "PrivateNetwork",
    "PrivateMounts",
    "PrivateUsers",
    "Delegate",
    "FileDescriptorStoreMax",
    "WatchdogUSec",
    "RootDirectory",
    "RootImage",
    "NetworkNamespacePath",
    "PAMName",
    "JoinsNamespaceOf",
    "ReadWritePaths",
    "ReadOnlyPaths",
    "InaccessiblePaths",
    "ExtensionDirectories",
    "BindPaths",
    "BindReadOnlyPaths",
    "TemporaryFileSystem",
    "ProtectProc",
    "ProcSubset",
    "OpenFile",
    "ExtraFileDescriptorNames",
];

fn frame(phase: Phase, event: Event, property: Option<&str>) -> ([u8; 96], usize) {
    let mut bytes = [0; 96];
    let mut n = 0;
    for part in [
        b"K1_DEV_STARTUP_V1 ".as_slice(),
        phase.label(),
        b" ",
        event.label(),
    ] {
        bytes[n..n + part.len()].copy_from_slice(part);
        n += part.len();
    }
    if let Some(key) = property {
        let id = PROPERTY_KEYS
            .iter()
            .position(|candidate| *candidate == key)
            .unwrap_or(99);
        bytes[n..n + 3].copy_from_slice(b" P=");
        n += 3;
        bytes[n] = b'0' + (id / 10) as u8;
        bytes[n + 1] = b'0' + (id % 10) as u8;
        n += 2;
    }
    bytes[n] = b'\n';
    (bytes, n + 1)
}

fn admit_write(frame: &[u8], write: impl FnOnce(&[u8]) -> Option<usize>) -> bool {
    write(frame) == Some(frame.len())
}
pub(crate) fn emit(phase: Phase, event: Event, property: Option<&str>) {
    if !ACTIVE.load(Ordering::Relaxed) {
        #[cfg(feature = "netguard-service-diagnostics")]
        if !matches!(event, Event::Begin | Event::Pass) {
            let (bytes, length) = frame(phase, event, property);
            crate::service_diagnostic::remember_origin(bytes, length);
        }
        return;
    }
    let (bytes, n) = frame(phase, event, property);
    if !admit_write(&bytes[..n], |bytes| {
        nix::unistd::write(std::io::stderr(), bytes).ok()
    }) {
        // No return/unwind/retry/query/exit: every local/assembled owner at
        // this exact point remains held. Single write has no hard time bound.
        loop {
            std::thread::park();
        }
    }
}
pub(crate) fn start() {
    ACTIVE.store(true, Ordering::Relaxed);
    // Do not let default panic formatting send private payloads to journal.
    // No Debug/Display, panic payload, location or backtrace is exported.
    std::panic::set_hook(Box::new(|_| emit(Phase::Panic, Event::Refused, None)));
    emit(Phase::Enter, Event::Begin, None);
}
pub(crate) fn finish() {
    emit(Phase::Ready, Event::Pass, None);
    ACTIVE.store(false, Ordering::Relaxed);
}
pub(crate) fn step<T, E>(phase: Phase, operation: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
    step_with(phase, |phase, event| emit(phase, event, None), operation)
}
fn step_with<T, E>(
    phase: Phase,
    mut sink: impl FnMut(Phase, Event),
    operation: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    sink(phase, Event::Begin);
    let result = operation();
    sink(
        phase,
        if result.is_ok() {
            Event::Pass
        } else {
            Event::Refused
        },
    );
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_frames_export_no_unknown_key_or_payload() {
        for phase in [
            Phase::Enter,
            Phase::Anchor,
            Phase::InheritedAnchors,
            Phase::BusOwner,
            Phase::Rpc,
            Phase::OriginFiles,
            Phase::EffectiveUnit,
            Phase::OriginalNamespaces,
            Phase::CreatorOpen,
            Phase::CreatorAssembled,
            Phase::StateOpen,
            Phase::ControlPublished,
            Phase::RecoveryPublished,
            Phase::AuthorityAssembled,
            Phase::Ready,
            Phase::Panic,
        ] {
            for event in [
                Event::Begin,
                Event::Pass,
                Event::Refused,
                Event::Missing,
                Event::Type,
                Event::Mismatch,
                Event::Decode,
                Event::Unavailable,
                Event::Deadline,
            ] {
                let (bytes, n) = frame(phase, event, Some("PRIVATE_DO_NOT_PRINT"));
                let text = std::str::from_utf8(&bytes[..n]).unwrap();
                assert!(text.starts_with("K1_DEV_STARTUP_V1 ") && text.ends_with(" P=99\n"));
                assert!(!text.contains("PRIVATE"));
                assert!(n <= 96 && text.matches('\n').count() == 1);
            }
        }
        assert!(PROPERTY_KEYS.len() < 99);
        for (index, key) in PROPERTY_KEYS.iter().enumerate() {
            assert!(!PROPERTY_KEYS[..index].contains(key));
        }
    }
    #[test]
    fn one_attempt_only_and_no_completion_on_short_error_or_extra_count() {
        for returned in [None, Some(0), Some(2), Some(4)] {
            let mut calls = 0;
            assert!(!admit_write(b"abc", |_| {
                calls += 1;
                returned
            }));
            assert_eq!(calls, 1);
        }
        assert!(admit_write(b"abc", |_| Some(3)));
    }
    #[test]
    fn failed_trace_cut_never_enters_following_effect() {
        for returned in [None, Some(0), Some(2)] {
            let mut reached = false;
            if admit_write(b"abc", |_| returned) {
                reached = true;
            }
            assert!(!reached);
        }
        // Pure stage operation controls do not enable trace or invoke IO.
        assert!(step(Phase::StateOpen, || Err::<(), _>(())).is_err());
    }
    #[test]
    fn every_pure_stage_refusal_stops_following_pipeline() {
        let phases = [
            Phase::Anchor,
            Phase::BusOwner,
            Phase::OriginFiles,
            Phase::EffectiveUnit,
            Phase::CreatorOpen,
            Phase::CreatorAssembled,
            Phase::StateOpen,
            Phase::ControlPublished,
            Phase::RecoveryPublished,
            Phase::AuthorityAssembled,
        ];
        for cut in 0..phases.len() {
            let mut attempts = 0;
            let mut frames = Vec::new();
            let result: Result<(), ()> = (|| {
                for (index, phase) in phases.iter().enumerate() {
                    step_with(
                        *phase,
                        |phase, event| frames.push((phase.label(), event.label())),
                        || {
                            attempts += 1;
                            if index == cut { Err(()) } else { Ok(()) }
                        },
                    )?;
                }
                Ok(())
            })();
            assert!(result.is_err());
            assert_eq!(attempts, cut + 1);
            assert_eq!(frames.last().unwrap().1, b"REFUSED");
            assert_eq!(frames.len(), 2 * (cut + 1));
        }
    }
}
