// SPDX-License-Identifier: MIT
//! Inactive cfg(test) integration; NOT an authenticated root launcher or IPC.
//!
//! The only constructor below captures a locally accessible original through
//! the existing strict Process flow. It cannot mint privileged/canonical
//! authority. A future kernel-credential transport is a separate review gate.

use super::*;
use std::rc::Rc;

#[path = "restore_abort_retained_parent_kernel_smoke.rs"]
mod kernel_smoke;

pub(super) struct Bundle {
    pub(super) executable: Option<File>,
    pub(super) executable_name: Option<Zeroizing<Vec<u8>>>,
    pub(super) pid_namespace: Option<File>,
    pub(super) user_namespace: Option<File>,
}

pub(super) struct LocalParent {
    root: File,
    root_identity: Metadata,
    original: Process,
    namespaces: [(File, Metadata); 2],
    refused: Cell<bool>,
    consultations: Cell<usize>,
}

impl LocalParent {
    pub(super) fn refuse(&self) {
        self.refused.set(true);
    }
    // This is a synthetic/local source seam, not a root-origin constructor.
    // No ordinary binary contains this type or any constructor/caller flag.
    fn capture_local(root: File, pid: u32, budget: &mut Budget) -> Result<Rc<Self>> {
        budget.check()?;
        let root_identity = root.metadata().map_err(|_| ())?;
        if fstatfs(&root).map_err(|_| ())?.filesystem_type() != PROC_SUPER_MAGIC {
            return Err(());
        }
        let original = Process::capture(&root, pid, budget)?;
        let capture_ns = |name| -> Result<(File, Metadata)> {
            budget.check()?;
            let file = magic_file(&original.directory, name)?;
            budget.check()?;
            let metadata = file.metadata().map_err(|_| ())?;
            budget.check()?;
            Ok((file, metadata))
        };
        let namespaces = [capture_ns("ns/pid")?, capture_ns("ns/user")?];
        budget.check()?;
        Ok(Rc::new(Self {
            root,
            root_identity,
            original,
            namespaces,
            refused: Cell::new(false),
            consultations: Cell::new(0),
        }))
    }

    pub(super) fn exact_row(&self, pid: u32, start: u64, directory: &Metadata) -> bool {
        pid == self.original.pid
            && start == self.original.start
            && identity(directory, &self.original.directory_identity)
    }

    pub(super) fn consult(
        &self,
        root: &File,
        pid: u32,
        directory: &File,
        start: u64,
        budget: &mut Budget,
    ) -> Result<Bundle> {
        if self.refused.replace(true) {
            return Err(());
        }
        // Failure/unknown/late permanently retains this parent and never
        // begins a second consultation. Success permits the next required
        // fresh check; it is not a reusable serialized authority token.
        self.consultations.set(self.consultations.get() + 1);
        budget.check()?;
        let metadata = directory.metadata().map_err(|_| ())?;
        budget.check()?;
        if !self.exact_row(pid, start, &metadata)
            || !identity(&self.root_identity, &root.metadata().map_err(|_| ())?)
            || !identity(&self.root_identity, &self.root.metadata().map_err(|_| ())?)
        {
            return Err(());
        }
        budget.check()?;
        self.original.recheck(&self.root, budget)?;
        budget.check()?;
        // Reopen the current image, not only fstat the initial historical FD.
        let current = Process::capture(&self.root, self.original.pid, budget)?;
        budget.check()?;
        if !self.exact_row(current.pid, current.start, &current.directory_identity)
            || current.status != self.original.status
            || current.command != self.original.command
            || current.comm != self.original.comm
            || current.executable_name != self.original.executable_name
            || !executable_identity(
                &current.executable_identity,
                &self.original.executable_identity,
            )
        {
            return Err(());
        }
        let namespace = |index: usize, name| -> Result<File> {
            budget.check()?;
            let held = self.namespaces[index].0.metadata().map_err(|_| ())?;
            budget.check()?;
            let named = magic_file(&current.directory, name)?;
            budget.check()?;
            let metadata = named.metadata().map_err(|_| ())?;
            budget.check()?;
            if !identity(&self.namespaces[index].1, &held) || !identity(&held, &metadata) {
                return Err(());
            }
            Ok(named)
        };
        let pid_namespace = namespace(0, "ns/pid")?;
        let user_namespace = namespace(1, "ns/user")?;
        budget.check()?;
        let bundle = Bundle {
            executable: Some(current.executable),
            executable_name: Some(current.executable_name),
            pid_namespace: Some(pid_namespace),
            user_namespace: Some(user_namespace),
        };
        self.refused.set(false);
        Ok(bundle)
    }
}

pub(super) fn capture_inventory_row(
    root: &File,
    pid: u32,
    start: u64,
    metadata: &Metadata,
    budget: &mut Budget,
    parent: Option<&Rc<LocalParent>>,
) -> Result<Process> {
    let selected = parent
        .filter(|parent| parent.exact_row(pid, start, metadata))
        .cloned();
    Process::capture_inner(root, pid, budget, selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parent() -> Rc<LocalParent> {
        // Current test process only. No child/manager query, root action,
        // process mutation or claim about a canonical system manager.
        LocalParent::capture_local(
            File::open("/proc").unwrap(),
            std::process::id(),
            &mut Budget::new(),
        )
        .unwrap()
    }

    #[test]
    fn actual_originals_integrate_capture_recheck_and_namespaces() {
        let parent = parent();
        let root = File::open("/proc").unwrap();
        let mut budget = Budget::new();
        let process = Process::capture_inner(
            &root,
            parent.original.pid,
            &mut budget,
            Some(parent.clone()),
        )
        .unwrap();
        assert!(process.retained_parent.is_some());
        assert!(executable_identity(
            &process.executable_identity,
            &parent.original.executable_identity
        ));
        process.recheck(&root, &mut budget).unwrap();
        for (index, name) in ["ns/pid", "ns/user"].into_iter().enumerate() {
            let namespace = process.current_namespace(&root, name, &mut budget).unwrap();
            same_namespace(&namespace, &parent.namespaces[index].0).unwrap();
        }
        assert!(parent.consultations.get() >= 5);
        assert!(!parent.refused.get());
    }

    #[test]
    fn wrong_pid_or_start_seals_before_any_next_consultation() {
        for wrong_pid in [true, false] {
            let parent = parent();
            let result = parent.consult(
                &parent.root,
                parent.original.pid + u32::from(wrong_pid),
                &parent.original.directory,
                parent.original.start + u64::from(!wrong_pid),
                &mut Budget::new(),
            );
            assert!(result.is_err());
            assert!(
                parent
                    .consult(
                        &parent.root,
                        parent.original.pid,
                        &parent.original.directory,
                        parent.original.start,
                        &mut Budget::new()
                    )
                    .is_err()
            );
            assert_eq!(parent.consultations.get(), 1);
        }
    }

    #[test]
    fn expired_consultation_is_permanent_with_originals_retained() {
        let parent = parent();
        let mut budget = Budget::new();
        budget.until = Instant::now();
        assert!(
            parent
                .consult(
                    &parent.root,
                    parent.original.pid,
                    &parent.original.directory,
                    parent.original.start,
                    &mut budget
                )
                .is_err()
        );
        assert!(
            parent
                .consult(
                    &parent.root,
                    parent.original.pid,
                    &parent.original.directory,
                    parent.original.start,
                    &mut Budget::new()
                )
                .is_err()
        );
        assert_eq!(parent.consultations.get(), 1);
        assert!(parent.original.executable.metadata().unwrap().is_file());
    }

    #[test]
    fn namespace_names_are_closed_and_ordinary_process_stays_direct() {
        let root = File::open("/proc").unwrap();
        let process = Process::capture(&root, std::process::id(), &mut Budget::new()).unwrap();
        assert!(process.retained_parent.is_none());
        let parent = parent();
        let process = Process::capture_inner(
            &root,
            parent.original.pid,
            &mut Budget::new(),
            Some(parent.clone()),
        )
        .unwrap();
        let before = parent.consultations.get();
        assert!(
            process
                .current_namespace(&root, "ns/net", &mut Budget::new())
                .is_err()
        );
        assert_eq!(parent.consultations.get(), before);
        assert!(parent.refused.get());
        assert!(
            process
                .current_namespace(&root, "ns/pid", &mut Budget::new())
                .is_err()
        );
        assert_eq!(parent.consultations.get(), before);
    }

    #[test]
    fn inventory_routes_only_exact_original_row_and_never_skips_capture() {
        let parent = parent();
        let root = File::open("/proc").unwrap();
        let direct = capture_inventory_row(
            &root,
            parent.original.pid,
            parent.original.start + 1,
            &parent.original.directory_identity,
            &mut Budget::new(),
            Some(&parent),
        )
        .unwrap();
        assert!(direct.retained_parent.is_none());
        assert_eq!(parent.consultations.get(), 0);
        let forwarded = capture_inventory_row(
            &root,
            parent.original.pid,
            parent.original.start,
            &parent.original.directory_identity,
            &mut Budget::new(),
            Some(&parent),
        )
        .unwrap();
        assert!(forwarded.retained_parent.is_some());
        assert!(parent.consultations.get() >= 2);
    }

    #[test]
    fn wrong_original_directory_image_or_namespace_permanently_refuses() {
        for mismatch in 0..3 {
            let mut parent = parent();
            if mismatch == 1 {
                Rc::get_mut(&mut parent)
                    .unwrap()
                    .original
                    .executable_identity = File::open("/dev/null").unwrap().metadata().unwrap();
            } else if mismatch == 2 {
                Rc::get_mut(&mut parent).unwrap().namespaces[0].1 =
                    File::open("/dev/null").unwrap().metadata().unwrap();
            }
            let supplied = if mismatch == 0 {
                &parent.root
            } else {
                &parent.original.directory
            };
            assert!(
                parent
                    .consult(
                        &parent.root,
                        parent.original.pid,
                        supplied,
                        parent.original.start,
                        &mut Budget::new()
                    )
                    .is_err()
            );
            assert!(parent.refused.get());
            assert_eq!(parent.consultations.get(), 1);
            assert!(
                parent
                    .consult(
                        &parent.root,
                        parent.original.pid,
                        &parent.original.directory,
                        parent.original.start,
                        &mut Budget::new()
                    )
                    .is_err()
            );
            assert_eq!(parent.consultations.get(), 1);
            assert!(parent.original.executable.metadata().unwrap().is_file());
        }
    }
}
