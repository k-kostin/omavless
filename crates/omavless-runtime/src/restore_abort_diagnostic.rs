// SPDX-License-Identifier: MIT
//! Test-only fixed admission breadcrumbs. Never a production trace or permit.
use std::cell::Cell;
use std::io::Write;

pub(crate) const ENTRY: &str =
    "production_owner::first_abort::cli_vm_fixture::diagnose_stopped_admission";
const HELPER: &[u8] = b"/home/ov-t4-abort-v6/.t4-first-abort/helper";

/// Private test-only latch: no extra observation, error details or output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ManagerCaptureStep {
    DirectoryOpen,
    DirectoryMetadata,
    StatRead,
    StatParse,
    StatusRead,
    StatusParse,
    CommandRead,
    CommandParse,
    CommRead,
    ExecutableOpen,
    ExecutableMetadata,
    ExecutableType,
    ExecutableLink,
    RecheckBudget,
    RecheckDirectory,
    RecheckExecutable,
    RecheckNamedDirectoryMetadata,
    RecheckHeldDirectoryMetadata,
    RecheckHeldExecutableMetadata,
    RecheckNamedExecutableMetadata,
    RecheckStatRead,
    RecheckStatParse,
    RecheckStatusRead,
    RecheckStatusParse,
    RecheckCommandRead,
    RecheckCommRead,
    RecheckExecutableLink,
}

impl ManagerCaptureStep {
    fn name(self) -> &'static str {
        match self {
            Self::DirectoryOpen => "directory_open",
            Self::DirectoryMetadata => "directory_metadata",
            Self::StatRead => "stat_read",
            Self::StatParse => "stat_parse",
            Self::StatusRead => "status_read",
            Self::StatusParse => "status_parse",
            Self::CommandRead => "command_read",
            Self::CommandParse => "command_parse",
            Self::CommRead => "comm_read",
            Self::ExecutableOpen => "executable_open",
            Self::ExecutableMetadata => "executable_metadata",
            Self::ExecutableType => "executable_type",
            Self::ExecutableLink => "executable_link",
            Self::RecheckBudget => "recheck_budget",
            Self::RecheckDirectory => "recheck_directory",
            Self::RecheckExecutable => "recheck_executable",
            Self::RecheckNamedDirectoryMetadata => "recheck_named_directory_metadata",
            Self::RecheckHeldDirectoryMetadata => "recheck_held_directory_metadata",
            Self::RecheckHeldExecutableMetadata => "recheck_held_executable_metadata",
            Self::RecheckNamedExecutableMetadata => "recheck_named_executable_metadata",
            Self::RecheckStatRead => "recheck_stat_read",
            Self::RecheckStatParse => "recheck_stat_parse",
            Self::RecheckStatusRead => "recheck_status_read",
            Self::RecheckStatusParse => "recheck_status_parse",
            Self::RecheckCommandRead => "recheck_command_read",
            Self::RecheckCommRead => "recheck_comm_read",
            Self::RecheckExecutableLink => "recheck_executable_link",
        }
    }
}

/// A category of the existing failed open result, never its cause or details.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ManagerExecutableOpenError {
    AccessDenied,
    OperationDenied,
    NotFound,
    Other,
}

impl ManagerExecutableOpenError {
    fn from_errno(error: nix::errno::Errno) -> Self {
        match error {
            nix::errno::Errno::EACCES => Self::AccessDenied,
            nix::errno::Errno::EPERM => Self::OperationDenied,
            nix::errno::Errno::ENOENT => Self::NotFound,
            _ => Self::Other,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::AccessDenied => "eacces",
            Self::OperationDenied => "eperm",
            Self::NotFound => "enoent",
            Self::Other => "other",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    CurrentPaths,
    ExistingLock,
    ProcRoot,
    SelfProcess,
    SelfArguments,
    ProcVisibility,
    NamespaceHandles,
    ManagerQuery,
    ManagerRecord,
    ManagerProcess,
    ManagerExecutable,
    ManagerIdentity,
    NamespaceBoundary,
    ManagerRecheck,
    ManagerIdentityRecheck,
    LegacyUnitQuery,
    LegacyUnitRecord,
    RuntimeUnitQuery,
    RuntimeUnitRecord,
    Inventory,
    UnixTable,
    UnixParser,
    FinalManager,
    FinalSelf,
    FinalNamespaces,
    FinalProcVisibility,
    FinalBudget,
    FinalLock,
}

impl Phase {
    fn name(self) -> &'static str {
        match self {
            Self::CurrentPaths => "current_paths",
            Self::ExistingLock => "existing_lock",
            Self::ProcRoot => "proc_root",
            Self::SelfProcess => "self_process",
            Self::SelfArguments => "self_arguments",
            Self::ProcVisibility => "proc_visibility",
            Self::NamespaceHandles => "namespace_handles",
            Self::ManagerQuery => "manager_query",
            Self::ManagerRecord => "manager_record",
            Self::ManagerProcess => "manager_process",
            Self::ManagerExecutable => "manager_executable",
            Self::ManagerIdentity => "manager_identity",
            Self::NamespaceBoundary => "namespace_boundary",
            Self::ManagerRecheck => "manager_recheck",
            Self::ManagerIdentityRecheck => "manager_identity_recheck",
            Self::LegacyUnitQuery => "legacy_unit_query",
            Self::LegacyUnitRecord => "legacy_unit_record",
            Self::RuntimeUnitQuery => "runtime_unit_query",
            Self::RuntimeUnitRecord => "runtime_unit_record",
            Self::Inventory => "inventory",
            Self::UnixTable => "unix_table",
            Self::UnixParser => "unix_parser",
            Self::FinalManager => "final_manager",
            Self::FinalSelf => "final_self",
            Self::FinalNamespaces => "final_namespaces",
            Self::FinalProcVisibility => "final_proc_visibility",
            Self::FinalBudget => "final_budget",
            Self::FinalLock => "final_lock",
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Trace {
    active: bool,
    sealed: bool,
    reported: bool,
    count: usize,
    last: Option<Phase>,
    manager_capture_step: Option<ManagerCaptureStep>,
    manager_executable_open_error: Option<ManagerExecutableOpenError>,
}

impl Trace {
    fn before(
        &mut self,
        phase: Phase,
        write: impl FnOnce(&[u8]) -> std::io::Result<usize>,
    ) -> Result<(), ()> {
        if !self.active {
            return Ok(());
        }
        if self.sealed || self.count >= 128 {
            self.sealed = true;
            return Err(());
        }
        self.last = Some(phase);
        self.manager_capture_step = None;
        self.manager_executable_open_error = None;
        self.count += 1;
        let raw = format!("T4_STOPPED_BEFORE_V1 {}\n", phase.name());
        if !matches!(write(raw.as_bytes()), Ok(size) if size == raw.len()) {
            self.sealed = true;
            self.reported = true;
            return Err(());
        }
        Ok(())
    }

    fn manager_executable_open_error(&mut self, error: nix::errno::Errno) {
        if !self.active
            || self.sealed
            || self.reported
            || self.last != Some(Phase::ManagerProcess)
            || self.manager_capture_step != Some(ManagerCaptureStep::ExecutableOpen)
        {
            return;
        }
        self.manager_executable_open_error = Some(ManagerExecutableOpenError::from_errno(error));
        // This operation has already failed. It cannot allow another operation.
        self.sealed = true;
    }

    fn manager_capture_before(&mut self, step: ManagerCaptureStep) -> Result<(), ()> {
        if !self.active {
            return Ok(());
        }
        if self.sealed || self.reported {
            return Err(());
        }
        if self.last == Some(Phase::ManagerProcess) {
            self.manager_capture_step = Some(step);
        }
        Ok(())
    }

    fn finish(
        &mut self,
        success: bool,
        write: impl FnOnce(&[u8]) -> std::io::Result<usize>,
    ) -> Result<(), ()> {
        if !self.active || self.reported {
            return Err(());
        }
        let success = success && !self.sealed;
        self.sealed = true;
        self.reported = true;
        let phase = self.last.ok_or(())?;
        let raw = if success {
            "T4_STOPPED_READONLY_OBSERVATION_NOT_ADMISSION\n".to_owned()
        } else {
            let mut raw = format!("T4_STOPPED_FAILED_AT_V1 {}\n", phase.name());
            if phase == Phase::ManagerProcess
                && let Some(step) = self.manager_capture_step
            {
                raw.push_str("T4_STOPPED_MANAGER_CAPTURE_BEFORE_V1 ");
                raw.push_str(step.name());
                raw.push('\n');
                if step == ManagerCaptureStep::ExecutableOpen
                    && let Some(error) = self.manager_executable_open_error
                {
                    raw.push_str("T4_STOPPED_MANAGER_EXECUTABLE_OPEN_ERROR_V1 ");
                    raw.push_str(error.name());
                    raw.push('\n');
                }
            }
            raw
        };
        if raw.len() > 256
            || !matches!(write(raw.as_bytes()), Ok(size) if size == raw.len())
            || !success
        {
            return Err(());
        }
        Ok(())
    }
}

thread_local! { static TRACE: Cell<Trace> = Cell::new(Trace::default()); }

pub(super) fn manager_executable_open_error(error: nix::errno::Errno) {
    TRACE.with(|cell| {
        let mut trace = cell.get();
        trace.manager_executable_open_error(error);
        cell.set(trace);
    });
}

pub(super) fn manager_capture_before(step: ManagerCaptureStep) -> Result<(), ()> {
    TRACE.with(|cell| {
        let mut trace = cell.get();
        let result = trace.manager_capture_before(step);
        cell.set(trace);
        result
    })
}

pub(super) fn before(phase: Phase) -> Result<(), ()> {
    TRACE.with(|cell| {
        let mut trace = cell.get();
        let result = trace.before(phase, |raw| std::io::stderr().lock().write(raw));
        cell.set(trace);
        result
    })
}

pub(super) fn run(observe: impl FnOnce() -> Result<(), ()>) -> Result<(), ()> {
    TRACE.with(|cell| {
        if cell.get().active {
            return Err(());
        }
        cell.set(Trace {
            active: true,
            ..Trace::default()
        });
        let result = observe();
        let mut trace = cell.get();
        let finished = trace.finish(result.is_ok(), |raw| std::io::stderr().lock().write(raw));
        cell.set(trace);
        finished
    })
}

pub(super) fn exact_self(args: &[&[u8]]) -> Result<(), ()> {
    (args
        == [
            HELPER,
            b"--exact",
            ENTRY.as_bytes(),
            b"--ignored",
            b"--nocapture",
            b"--test-threads=1",
            b"--quiet",
        ])
    .then_some(())
    .ok_or(())
}

#[test]
fn sink_failure_and_operation_failure_are_terminal() {
    for short in [false, true] {
        let mut trace = Trace {
            active: true,
            ..Trace::default()
        };
        assert!(
            trace
                .before(Phase::ExistingLock, |raw| if short {
                    Ok(raw.len() - 1)
                } else {
                    Err(std::io::Error::other("synthetic"))
                })
                .is_err()
        );
        assert!(
            trace
                .before(Phase::Inventory, |_| panic!("late sink"))
                .is_err()
        );
        assert!(trace.finish(false, |_| panic!("retry sink")).is_err());
    }
    let mut trace = Trace {
        active: true,
        ..Trace::default()
    };
    assert!(
        trace
            .before(Phase::ManagerQuery, |raw| Ok(raw.len()))
            .is_ok()
    );
    let mut output: Vec<u8> = Vec::new();
    assert!(
        trace
            .finish(false, |raw| {
                output.extend(raw);
                Ok(raw.len())
            })
            .is_err()
    );
    assert_eq!(output, b"T4_STOPPED_FAILED_AT_V1 manager_query\n");
    assert!(
        trace
            .before(Phase::Inventory, |_| panic!("late operation"))
            .is_err()
    );
    assert!(trace.finish(true, |_| panic!("late success")).is_err());
}

#[test]
fn fixed_self_and_trace_budget_never_admit_generic_overrides() {
    let args = [
        HELPER,
        b"--exact",
        ENTRY.as_bytes(),
        b"--ignored",
        b"--nocapture",
        b"--test-threads=1",
        b"--quiet",
    ];
    assert!(exact_self(&args).is_ok());
    for index in 0..args.len() {
        let mut bad = args;
        bad[index] = b"other";
        assert!(exact_self(&bad).is_err());
    }
    assert!(exact_self(&args[..6]).is_err());
    let mut trace = Trace {
        active: true,
        ..Trace::default()
    };
    for _ in 0..128 {
        assert!(trace.before(Phase::Inventory, |raw| Ok(raw.len())).is_ok());
    }
    assert!(
        trace
            .before(Phase::Inventory, |_| panic!("over budget"))
            .is_err()
    );
    assert!(trace.finish(true, |raw| Ok(raw.len())).is_err());
    assert!(
        Trace::default()
            .before(Phase::Inventory, |_| panic!("ordinary test trace"))
            .is_ok()
    );
}

#[test]
fn every_phase_short_write_prevents_its_operation_and_all_later_sinks() {
    for phase in [
        Phase::CurrentPaths,
        Phase::ExistingLock,
        Phase::ProcRoot,
        Phase::SelfProcess,
        Phase::SelfArguments,
        Phase::ProcVisibility,
        Phase::NamespaceHandles,
        Phase::ManagerQuery,
        Phase::ManagerRecord,
        Phase::ManagerProcess,
        Phase::ManagerExecutable,
        Phase::ManagerIdentity,
        Phase::NamespaceBoundary,
        Phase::ManagerRecheck,
        Phase::ManagerIdentityRecheck,
        Phase::LegacyUnitQuery,
        Phase::LegacyUnitRecord,
        Phase::RuntimeUnitQuery,
        Phase::RuntimeUnitRecord,
        Phase::Inventory,
        Phase::UnixTable,
        Phase::UnixParser,
        Phase::FinalManager,
        Phase::FinalSelf,
        Phase::FinalNamespaces,
        Phase::FinalProcVisibility,
        Phase::FinalBudget,
        Phase::FinalLock,
    ] {
        let mut trace = Trace {
            active: true,
            ..Trace::default()
        };
        let mut reached = false;
        let operation = (|| {
            trace.before(phase, |raw| Ok(raw.len() - 1))?;
            reached = true;
            Ok::<(), ()>(())
        })();
        assert!(operation.is_err());
        assert!(!reached);
        assert!(trace.before(phase, |_| panic!("late phase sink")).is_err());
        assert!(
            trace
                .finish(false, |_| panic!("late failure sink"))
                .is_err()
        );
    }
}

#[test]
fn manager_capture_latch_is_private_finite_reset_and_terminal() {
    for step in [
        ManagerCaptureStep::DirectoryOpen,
        ManagerCaptureStep::DirectoryMetadata,
        ManagerCaptureStep::StatRead,
        ManagerCaptureStep::StatParse,
        ManagerCaptureStep::StatusRead,
        ManagerCaptureStep::StatusParse,
        ManagerCaptureStep::CommandRead,
        ManagerCaptureStep::CommandParse,
        ManagerCaptureStep::CommRead,
        ManagerCaptureStep::ExecutableOpen,
        ManagerCaptureStep::ExecutableMetadata,
        ManagerCaptureStep::ExecutableType,
        ManagerCaptureStep::ExecutableLink,
        ManagerCaptureStep::RecheckBudget,
        ManagerCaptureStep::RecheckDirectory,
        ManagerCaptureStep::RecheckExecutable,
        ManagerCaptureStep::RecheckNamedDirectoryMetadata,
        ManagerCaptureStep::RecheckHeldDirectoryMetadata,
        ManagerCaptureStep::RecheckHeldExecutableMetadata,
        ManagerCaptureStep::RecheckNamedExecutableMetadata,
        ManagerCaptureStep::RecheckStatRead,
        ManagerCaptureStep::RecheckStatParse,
        ManagerCaptureStep::RecheckStatusRead,
        ManagerCaptureStep::RecheckStatusParse,
        ManagerCaptureStep::RecheckCommandRead,
        ManagerCaptureStep::RecheckCommRead,
        ManagerCaptureStep::RecheckExecutableLink,
    ] {
        let mut inactive = Trace::default();
        inactive.manager_capture_before(step).unwrap();
        assert_eq!(inactive.manager_capture_step, None);
        assert_eq!(inactive.count, 0);
        let mut trace = Trace {
            active: true,
            ..Trace::default()
        };
        trace
            .before(Phase::SelfProcess, |raw| Ok(raw.len()))
            .unwrap();
        trace.manager_capture_before(step).unwrap();
        assert_eq!(trace.manager_capture_step, None);
        trace
            .before(Phase::ManagerProcess, |raw| Ok(raw.len()))
            .unwrap();
        trace.manager_capture_before(step).unwrap();
        assert_eq!(trace.count, 2, "latch neither emits nor charges a BEFORE");
        let mut writes = 0;
        assert!(trace.finish(false, |raw| {
            writes += 1;
            let expected = format!("T4_STOPPED_FAILED_AT_V1 manager_process\nT4_STOPPED_MANAGER_CAPTURE_BEFORE_V1 {}\n", step.name());
            assert_eq!(raw, expected.as_bytes());
            assert!(raw.len() <= 256);
            Ok(raw.len())
        }).is_err());
        assert_eq!(writes, 1);
        assert!(trace.manager_capture_before(step).is_err());
        assert!(trace.finish(false, |_| panic!("second terminal")).is_err());

        let mut trace = Trace {
            active: true,
            ..Trace::default()
        };
        trace
            .before(Phase::ManagerProcess, |raw| Ok(raw.len()))
            .unwrap();
        trace.manager_capture_before(step).unwrap();
        trace
            .before(Phase::ManagerExecutable, |raw| Ok(raw.len()))
            .unwrap();
        assert_eq!(trace.manager_capture_step, None);
        trace
            .finish(true, |raw| {
                assert_eq!(raw, b"T4_STOPPED_READONLY_OBSERVATION_NOT_ADMISSION\n");
                Ok(raw.len())
            })
            .unwrap();
    }
}

#[test]
fn manager_executable_errno_is_finite_scoped_and_permanently_failed() {
    use nix::errno::Errno;
    for (error, literal) in [
        (Errno::EACCES, "eacces"),
        (Errno::EPERM, "eperm"),
        (Errno::ENOENT, "enoent"),
        (Errno::EIO, "other"),
        (Errno::EINTR, "other"),
        (Errno::UnknownErrno, "other"),
    ] {
        let mut trace = Trace {
            active: true,
            ..Trace::default()
        };
        trace
            .before(Phase::ManagerProcess, |raw| Ok(raw.len()))
            .unwrap();
        trace
            .manager_capture_before(ManagerCaptureStep::ExecutableOpen)
            .unwrap();
        trace.manager_executable_open_error(error);
        assert_eq!(trace.count, 1, "the error latch emits/observes nothing");
        assert!(trace.sealed);
        assert!(
            trace
                .manager_capture_before(ManagerCaptureStep::ExecutableMetadata)
                .is_err()
        );
        assert!(
            trace
                .before(Phase::ManagerExecutable, |_| panic!("later operation"))
                .is_err()
        );
        trace.manager_executable_open_error(Errno::EINVAL);
        let mut writes = 0;
        // Even a synthetic successful return cannot turn an observed error into success.
        assert!(trace.finish(true, |raw| {
            writes += 1;
            let expected = format!("T4_STOPPED_FAILED_AT_V1 manager_process\nT4_STOPPED_MANAGER_CAPTURE_BEFORE_V1 executable_open\nT4_STOPPED_MANAGER_EXECUTABLE_OPEN_ERROR_V1 {literal}\n");
            assert_eq!(raw, expected.as_bytes());
            assert!(raw.len() <= 256);
            Ok(raw.len())
        }).is_err());
        assert_eq!(writes, 1);
        assert!(
            trace
                .finish(false, |_| panic!("second terminal attempt"))
                .is_err()
        );
    }
}

#[test]
fn manager_executable_errno_wrong_phase_step_or_inactive_has_no_effect() {
    use nix::errno::Errno;
    let mut inactive = Trace::default();
    inactive.manager_executable_open_error(Errno::EACCES);
    assert!(!inactive.sealed);
    assert_eq!(inactive.manager_executable_open_error, None);
    for (phase, step) in [
        (Phase::SelfProcess, ManagerCaptureStep::ExecutableOpen),
        (Phase::ManagerProcess, ManagerCaptureStep::CommRead),
        (Phase::ManagerProcess, ManagerCaptureStep::RecheckExecutable),
        (Phase::ManagerIdentity, ManagerCaptureStep::ExecutableOpen),
    ] {
        let mut trace = Trace {
            active: true,
            ..Trace::default()
        };
        trace.before(phase, |raw| Ok(raw.len())).unwrap();
        trace.manager_capture_before(step).unwrap();
        trace.manager_executable_open_error(Errno::EACCES);
        assert!(!trace.sealed);
        assert_eq!(trace.manager_executable_open_error, None);
        trace.before(Phase::FinalLock, |raw| Ok(raw.len())).unwrap();
        assert_eq!(trace.manager_capture_step, None);
        trace
            .finish(true, |raw| {
                assert_eq!(raw, b"T4_STOPPED_READONLY_OBSERVATION_NOT_ADMISSION\n");
                Ok(raw.len())
            })
            .unwrap();
    }
}

#[test]
fn manager_executable_errno_terminal_short_throw_and_extra_bytes_do_not_retry() {
    for variant in 0..3 {
        let mut trace = Trace {
            active: true,
            ..Trace::default()
        };
        trace
            .before(Phase::ManagerProcess, |raw| Ok(raw.len()))
            .unwrap();
        trace
            .manager_capture_before(ManagerCaptureStep::ExecutableOpen)
            .unwrap();
        trace.manager_executable_open_error(nix::errno::Errno::EACCES);
        let mut writes = 0;
        assert!(
            trace
                .finish(false, |raw| {
                    writes += 1;
                    match variant {
                        0 => Ok(raw.len() - 1),
                        1 => Err(std::io::Error::other("private synthetic value")),
                        _ => Ok(raw.len() + 1),
                    }
                })
                .is_err()
        );
        assert_eq!(writes, 1);
        assert!(trace.reported && trace.sealed);
        assert!(
            trace
                .finish(false, |_| panic!("fallback or retry output"))
                .is_err()
        );
    }
}
