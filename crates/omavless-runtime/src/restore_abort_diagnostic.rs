// SPDX-License-Identifier: MIT
//! Test-only fixed admission breadcrumbs. Never a production trace or permit.
use std::cell::Cell;
use std::io::Write;

pub(crate) const ENTRY: &str =
    "production_owner::first_abort::cli_vm_fixture::diagnose_stopped_admission";
const HELPER: &[u8] = b"/home/ov-t4-abort-v4/.t4-first-abort/helper";

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
        self.count += 1;
        let raw = format!("T4_STOPPED_BEFORE_V1 {}\n", phase.name());
        if !matches!(write(raw.as_bytes()), Ok(size) if size == raw.len()) {
            self.sealed = true;
            self.reported = true;
            return Err(());
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
            format!("T4_STOPPED_FAILED_AT_V1 {}\n", phase.name())
        };
        if !matches!(write(raw.as_bytes()), Ok(size) if size == raw.len()) || !success {
            return Err(());
        }
        Ok(())
    }
}

thread_local! { static TRACE: Cell<Trace> = Cell::new(Trace::default()); }

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
