// SPDX-License-Identifier: MIT
//! Ignored installed Product checks. No publication, native effect or fixture roots.
use super::*;
use std::io::Write;
use std::time::Duration;

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(3)
}
fn root_profile() {
    assert_eq!(rustix::process::getuid().as_raw(), 0);
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    assert_eq!(rustix::process::getgid().as_raw(), 0);
    assert_eq!(rustix::process::getegid().as_raw(), 0);
    let raw = std::fs::read_to_string("/proc/self/status").unwrap();
    for prefix in ["CapPrm:", "CapEff:"] {
        let values: Vec<_> = raw
            .lines()
            .filter_map(|line| line.strip_prefix(prefix))
            .collect();
        assert_eq!(values.len(), 1);
        assert!(
            values[0].trim() == "0000000000080000",
            "fixed helper capability profile required"
        );
    }
}
fn pidfd(pid: u32) -> OwnedFd {
    let pid = rustix::process::Pid::from_raw(pid.try_into().unwrap()).unwrap();
    rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty()).unwrap()
}
fn same_image(row: &Row, image: &Fixed, until: Instant) -> bool {
    row.image(until)
        .and_then(|file| file.metadata().map_err(|_| Error::Unavailable))
        .is_ok_and(|m| Identity::of(&m) == image.id)
}
fn namespaces_match(a: &Row, b: &Row, until: Instant) {
    for name in ["ns/pid", "ns/user"] {
        let a = a.namespace(name, until).unwrap().metadata().unwrap();
        let b = b.namespace(name, until).unwrap().metadata().unwrap();
        assert_eq!((a.dev(), a.ino()), (b.dev(), b.ino()));
    }
}

#[test]
#[ignore = "actual enrolled Product, connected owned core and inert normal TUI; root with only CAP_SYS_PTRACE"]
fn installed_product_original_parent_and_uid_refusals() {
    root_profile();
    let roots = Roots::capture(Class::Product, deadline()).unwrap();
    let until = deadline();
    let mut cores = vec![];
    let mut tuis = vec![];
    let mut seen = 0;
    for entry in std::fs::read_dir("/proc").unwrap() {
        tick(until).unwrap();
        let entry = entry.unwrap();
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        seen += 1;
        assert!(seen <= 4096, "bounded discovery exceeded");
        let Ok(row) = Row::open(pid, until) else {
            continue;
        };
        let Ok(facts) = row.facts(until) else {
            continue;
        };
        if facts.uids != [roots.uid; 4] {
            continue;
        }
        if same_image(&row, &roots.core, until) {
            cores.push(pid);
        }
        if same_image(&row, &roots.client, until)
            && row.read("cmdline", until).is_ok_and(|raw| {
                raw == b"/usr/bin/omavless\0tui\0--developer-conditional-close\0--selected-set\0"
            })
        {
            tuis.push(pid);
        }
    }
    assert_eq!(cores.len(), 1, "unique original enrolled core required");
    assert_eq!(
        tuis.len(),
        1,
        "unique owned inert selected-set TUI required"
    );
    let core = Row::open(cores[0], deadline()).unwrap();
    let core_facts = core.facts(deadline()).unwrap();
    let daemon = Row::open(core_facts.parent, deadline()).unwrap();
    let tui = Row::open(tuis[0], deadline()).unwrap();
    let tui_fd = pidfd(tuis[0]);
    let tui_continuity = rustix::io::fcntl_dupfd_cloexec(&tui_fd, 0).unwrap();
    let tui_facts = tui.facts(deadline()).unwrap();
    assert_eq!(kernel_pid(&tui_fd, deadline()).unwrap(), tuis[0]);
    assert_eq!(tui.facts(deadline()).unwrap().uids, [roots.uid; 4]);
    assert!(same_image(&tui, &roots.client, deadline()));
    namespaces_match(&daemon, &tui, deadline());
    namespaces_match(&daemon, &core, deadline());
    assert_ne!(core_facts.parent, tuis[0]);
    let mut positive = Binding::bind(
        &roots,
        pidfd(core_facts.parent),
        pidfd(cores[0]),
        deadline(),
    )
    .unwrap();
    drop(positive.observe(deadline()).unwrap());
    // Exactly the independently qualified foreign parent changes. No PID/frame supplied by a caller.
    assert!(matches!(
        Binding::bind(&roots, tui_fd, pidfd(cores[0]), deadline()),
        Err(Error::Refused)
    ));
    // Refused must not be a dead foreign peer disguised as a parent mismatch.
    assert_eq!(kernel_pid(&tui_continuity, deadline()).unwrap(), tuis[0]);
    assert!(tui.facts(deadline()).unwrap() == tui_facts);
    assert!(same_image(&tui, &roots.client, deadline()));
    namespaces_match(&daemon, &tui, deadline());
    drop(positive.observe(deadline()).unwrap());
    let (a, _b) = rustix::net::socketpair(
        rustix::net::AddressFamily::UNIX,
        rustix::net::SocketType::SEQPACKET,
        rustix::net::SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    assert_eq!(
        rustix::net::sockopt::socket_peercred(&a)
            .unwrap()
            .uid
            .as_raw(),
        0
    );
    assert_ne!(roots.uid, 0);
    assert!(matches!(
        crate::channel::Endpoint::new(a, roots.uid, deadline()),
        Err(Error::Refused)
    ));
    roots.check(deadline()).unwrap();
}

// The holder cannot reconstruct Roots, Binding, a session, a permit or a lease.
struct RetainedObjects {
    core: Fixed,
    client: Fixed,
    enrollment: Fixed,
}
impl RetainedObjects {
    fn consume(roots: Roots) -> Self {
        let Roots {
            core,
            client,
            enrollment,
            uid: _,
            product_lease,
        } = roots;
        drop(product_lease);
        Self {
            core,
            client,
            enrollment,
        }
    }
}
#[test]
#[ignore = "one ordinary managed package/root upgrade, externally owned original stdin and completion"]
fn installed_product_retained_objects_refuse_managed_upgrade() {
    root_profile();
    let roots = Roots::capture(Class::Product, deadline()).unwrap();
    roots.check(deadline()).unwrap();
    let held = RetainedObjects::consume(roots);
    held.core.check(deadline()).unwrap();
    held.client.check(deadline()).unwrap();
    held.enrollment.check(deadline()).unwrap();
    println!("omavless.product-object.v1 stage=retained authority=none");
    std::io::stdout().flush().unwrap();
    // This finite diagnostic wait is not a product/session deadline or operation.
    let until = Instant::now() + Duration::from_secs(180);
    let stdin = std::io::stdin();
    let mut fds = [PollFd::new(&stdin, PollFlags::IN)];
    assert_eq!(
        poll(
            &mut fds,
            Some(&Timespec {
                tv_sec: 180,
                tv_nsec: 0
            })
        )
        .unwrap(),
        1
    );
    assert_eq!(fds[0].revents(), PollFlags::IN);
    let mut command = [0];
    assert_eq!(rustix::io::read(std::io::stdin(), &mut command).unwrap(), 1);
    assert_eq!(command, *b"I");
    tick(until).unwrap();
    assert!(matches!(held.client.check(deadline()), Err(Error::Refused)));
    assert!(matches!(
        held.enrollment.check(deadline()),
        Err(Error::Refused)
    ));
    let changed = match held.core.check(deadline()) {
        Ok(()) => false,
        Err(Error::Refused) => true,
        Err(_) => panic!("core observation unavailable, not a drift refusal"),
    };
    println!(
        "omavless.product-object.v1 stage=inspected runtime_refused=true enrollment_refused=true core_changed={changed}"
    );
}

#[test]
#[ignore = "actual Product original daemon/core; parent owns ONE normal Disconnect after READY; root CAP_SYS_PTRACE only"]
fn installed_product_same_binding_refuses_original_core_death() {
    use rustix::event::{PollFd, PollFlags, Timespec, poll};
    root_profile();
    let roots = Roots::capture(Class::Product, deadline()).unwrap();
    let until = deadline();
    let mut cores = vec![];
    let mut seen = 0;
    for entry in std::fs::read_dir("/proc").unwrap() {
        tick(until).unwrap();
        let entry = entry.unwrap();
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        seen += 1;
        assert!(seen <= 4096, "bounded discovery exceeded");
        let Ok(row) = Row::open(pid, until) else {
            continue;
        };
        let Ok(facts) = row.facts(until) else {
            continue;
        };
        if facts.uids == [roots.uid; 4] && same_image(&row, &roots.core, until) {
            cores.push(pid);
        }
    }
    assert_eq!(cores.len(), 1, "unique original enrolled core required");
    let core = Row::open(cores[0], deadline()).unwrap();
    let facts = core.facts(deadline()).unwrap();
    let daemon = Row::open(facts.parent, deadline()).unwrap();
    let daemon_facts = daemon.facts(deadline()).unwrap();
    let core_fd = pidfd(cores[0]);
    let daemon_fd = pidfd(facts.parent);
    let held = rustix::io::fcntl_dupfd_cloexec(&core_fd, 0).unwrap();
    let peer = rustix::io::fcntl_dupfd_cloexec(&daemon_fd, 0).unwrap();
    let mut original = Binding::bind(&roots, daemon_fd, core_fd, deadline()).unwrap();
    drop(original.observe(deadline()).unwrap());
    assert!(!original.poisoned);
    println!("omavless.product-death.v1 stage=ready");
    std::io::stdout().flush().unwrap();
    // Test rendezvous only, never a Session/Finish/effect deadline renewal.
    let bound = Instant::now() + Duration::from_secs(20);
    loop {
        let left = bound
            .checked_duration_since(Instant::now())
            .expect("original death timeout");
        let timeout = Timespec {
            tv_sec: left.as_secs().try_into().unwrap(),
            tv_nsec: left.subsec_nanos().into(),
        };
        let mut fds = [PollFd::new(&held, PollFlags::IN)];
        match poll(&mut fds, Some(&timeout)) {
            Err(rustix::io::Errno::INTR) => continue,
            Ok(1) if fds[0].revents().contains(PollFlags::IN) => break,
            _ => panic!("original core death not observed"),
        }
    }
    let until = deadline();
    roots.check(until).unwrap();
    assert_eq!(kernel_pid(&peer, until).unwrap(), facts.parent);
    assert!(daemon.facts(until).unwrap() == daemon_facts);
    assert!(same_image(&daemon, &roots.client, until));
    assert_eq!(alive(&held, until), Err(Error::Refused));
    assert!(matches!(original.observe(until), Err(Error::Refused)));
    assert!(original.poisoned);
    assert!(matches!(original.observe(until), Err(Error::Refused)));
    tick(until).unwrap();
    drop(original);
    drop((held, peer, daemon, core, roots));
    println!("omavless.product-death.v1 stage=complete same_binding_refused=true");
}
