// SPDX-License-Identifier: MIT
//! Developer-only integrated fixture. Parent module exists only under cfg(test).
//! No canonical namespace or installed authority is established here.
use super::*;
use crate::locked_state::LockedState;
use crate::protocol::{Health, Mode, Protection, Request, Response};
use crate::receipt::NamespaceObservation;
use crate::root_state::RootStateStore;
use nix::errno::Errno;
use nix::sched::{CloneFlags, setns};
use std::collections::BTreeMap;
use std::fs::{DirBuilder, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, OpenOptionsExt};
use std::path::Path;

const STAGE: &str = "/run/omavless-k1-supported-socket-admission";
const TEST: &str =
    "kernel_observer::creator_lifecycle::response_diagnostic::manager_private_lifecycle";
const CAP_NET_ADMIN: &str = "0000000000001000";
const ARM: Request = Request::Arm {
    generation: 7,
    mode: Mode::Full,
};
const DISARM: Request = Request::Disarm { generation: 7 };

fn bounded(path: &str) -> Result<String> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|_| REFUSE)?
        .take(16385)
        .read_to_end(&mut bytes)
        .map_err(|_| REFUSE)?;
    require(bytes.len() <= 16384)?;
    String::from_utf8(bytes).map_err(|_| REFUSE)
}

fn credentials(text: &str) -> Result<()> {
    let wanted = [
        "Uid",
        "Gid",
        "Groups",
        "CapInh",
        "CapPrm",
        "CapEff",
        "CapBnd",
        "CapAmb",
        "NoNewPrivs",
        "Seccomp",
    ];
    let mut fields = BTreeMap::new();
    for line in text.lines() {
        let (key, value) = line.split_once(':').ok_or(REFUSE)?;
        if wanted.contains(&key) {
            require(
                fields
                    .insert(key, value.split_whitespace().collect::<Vec<_>>())
                    .is_none(),
            )?;
        }
    }
    require(fields.len() == wanted.len())?;
    for key in ["Uid", "Gid"] {
        require(fields[key] == ["0", "0", "0", "0"])?;
    }
    require(fields["Groups"].is_empty())?;
    for key in ["CapPrm", "CapEff", "CapBnd"] {
        require(fields[key] == [CAP_NET_ADMIN])?;
    }
    for key in ["CapInh", "CapAmb"] {
        require(fields[key] == ["0000000000000000"])?;
    }
    require(fields["NoNewPrivs"] == ["1"] && fields["Seccomp"] == ["2"])
}

fn loopback_only(text: &str) -> Result<()> {
    let rows = text.lines().skip(2).collect::<Vec<_>>();
    require(
        rows.len() == 1
            && rows[0]
                .split_once(':')
                .is_some_and(|(name, _)| name.trim() == "lo"),
    )
}

fn distinct(host: (u64, u64), own: (u64, u64), manager: (u64, u64)) -> Result<()> {
    require(host.1 != 0 && own.1 != 0 && host == manager && own != host)
}

struct Isolation {
    host: File,
    own: File,
    host_id: (u64, u64),
    own_id: (u64, u64),
}
impl Isolation {
    fn capture() -> Result<Self> {
        // First open: missing FD3 cannot accidentally become our own ns opener.
        let host = File::open("/proc/self/fd/3").map_err(|_| REFUSE)?;
        let own = namespace_file()?;
        let value = Self {
            host_id: namespace_identity(&host)?,
            own_id: namespace_identity(&own)?,
            host,
            own,
        };
        value.recheck()?;
        Ok(value)
    }
    fn recheck(&self) -> Result<()> {
        credentials(&bounded("/proc/thread-self/status")?)?;
        require(
            namespace_identity(&self.host)? == self.host_id
                && namespace_identity(&self.own)? == self.own_id,
        )?;
        require(
            namespace_identity(&File::open("/proc/self/fd/3").map_err(|_| REFUSE)?)?
                == self.host_id,
        )?;
        require(namespace_identity(&namespace_file()?)? == self.own_id)?;
        let manager = File::open("/proc/1/ns/net").map_err(|_| REFUSE)?;
        distinct(self.host_id, self.own_id, namespace_identity(&manager)?)?;
        loopback_only(&bounded("/proc/thread-self/net/dev")?)?;
        let null = File::open("/dev/null").map_err(|_| REFUSE)?;
        let meta = null.metadata().map_err(|_| REFUSE)?;
        require(
            meta.file_type().is_char_device()
                && nix::sys::stat::major(meta.rdev()) == 1
                && nix::sys::stat::minor(meta.rdev()) == 3,
        )?;
        // Never call setns with a namespace FD. The inherited manager anchor is
        // only a negative witness. EINVAL means the filter was not demonstrated.
        require(setns(&null, CloneFlags::empty()) == Err(Errno::EPERM))
    }
}

fn root_directory(path: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_DIRECTORY | nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| REFUSE)?;
    let m = file.metadata().map_err(|_| REFUSE)?;
    let p = path.symlink_metadata().map_err(|_| REFUSE)?;
    require(
        m.is_dir()
            && m.uid() == 0
            && m.gid() == 0
            && m.mode() & 0o7777 == 0o700
            && (m.dev(), m.ino()) == (p.dev(), p.ino()),
    )?;
    Ok(file)
}

// Intentionally leaked until process exit. On any uncertainty the fixture
// parks forever, retaining actual original lock/socket/ns descriptors. No Drop
// path, reset, retry, signal, process spawn or table cleanup runs on failure.
struct Held {
    isolation: Isolation,
    stage: Option<File>,
    creator: Option<FixtureCreator>,
    state: Option<LockedState>,
    independent: Option<FixtureCreator>,
}

fn armed(response: Response) -> Result<()> {
    require(matches!(
        response,
        Response::Status {
            protection: Protection::Armed { generation: 7 },
            health: Health::Verified,
            ..
        }
    ))
}
fn disarmed(response: Response) -> Result<()> {
    require(matches!(
        response,
        Response::Status {
            protection: Protection::Disarmed {},
            health: Health::Verified,
            ..
        }
    ))
}

fn run(held: &mut Held) -> Result<()> {
    held.isolation.recheck()?;
    let run = std::fs::symlink_metadata("/run").map_err(|_| REFUSE)?;
    require(run.is_dir() && run.uid() == 0 && run.gid() == 0 && run.mode() & 0o022 == 0)?;
    let stage = Path::new(STAGE);
    held.stage = Some(root_directory(stage)?);
    let state_parent = stage.join("state");
    DirBuilder::new()
        .mode(0o700)
        .create(&state_parent)
        .map_err(|_| REFUSE)?;
    DirBuilder::new()
        .mode(0o700)
        .create(state_parent.join("omavless-netguard"))
        .map_err(|_| REFUSE)?;
    held.isolation.recheck()?; // Before the first netlink socket, not just writes.
    held.creator = Some(FixtureCreator::open(state_parent.clone())?);
    let creator = held.creator.as_mut().ok_or(REFUSE)?;
    require(creator.session.identity == held.isolation.own_id)?;
    // Existing TEST-ONLY modeled epoch; never an authenticated Canonical token.
    let namespace = NamespaceObservation::Canonical(creator.epoch);
    held.state = Some(LockedState::from_root(
        RootStateStore::open_test_parent(root_directory(&state_parent)?, (0, 0), 1001)
            .map_err(|_| REFUSE)?,
    ));
    let state = held.state.as_mut().ok_or(REFUSE)?;
    require(creator.session.inspect_policy_inventory()? == LocalPolicyInventory::TableAbsent)?;
    held.isolation.recheck()?;
    armed(state.request(ARM, namespace, creator).map_err(|_| REFUSE)?)?;
    require(creator.effects == 1 && creator.created.is_some())?;
    require(
        creator.session.inspect_policy_inventory()?
            == LocalPolicyInventory::ExactUntrusted(Policy::FullVpn),
    )?;
    held.isolation.recheck()?;
    // A second socket with exactly matching rules cannot acquire live creator
    // causality, even in the same namespace and with the same synthetic epoch.
    held.independent = Some(FixtureCreator::open(state_parent)?);
    let independent = held.independent.as_mut().ok_or(REFUSE)?;
    require(
        independent.session.inspect_policy_inventory()? == LocalPolicyInventory::OtherUntrusted,
    )?;
    require(
        independent.observe().is_err() && independent.effects == 0 && independent.created.is_none(),
    )?;
    held.isolation.recheck()?;
    disarmed(
        state
            .request(DISARM, namespace, creator)
            .map_err(|_| REFUSE)?,
    )?;
    require(creator.effects == 2 && creator.created.is_none())?;
    require(creator.session.inspect_policy_inventory()? == LocalPolicyInventory::TableAbsent)?;
    disarmed(
        state
            .request(Request::Status {}, namespace, creator)
            .map_err(|_| REFUSE)?,
    )?;
    held.isolation.recheck()?;
    // No reuse or cleanup. The root observer independently verifies Closed and
    // Retired files, exact own unit/process state and its full host baseline.
    let mut receipt = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(stage.join("native-result.json"))
        .map_err(|_| REFUSE)?;
    receipt.write_all(b"{\"schema\":1,\"synthetic_epoch\":true,\"effects\":2,\"full_inventory\":true,\"second_socket_untrusted\":true,\"absent\":true}\n").map_err(|_| REFUSE)?;
    receipt.sync_all().map_err(|_| REFUSE)?;
    held.stage
        .as_ref()
        .ok_or(REFUSE)?
        .sync_all()
        .map_err(|_| REFUSE)?;
    Ok(())
}

#[test]
#[ignore = "fixed manager-owned PrivateNetwork fixture; dedicated VM lease and reviewed outer guard required"]
fn manager_private_lifecycle() {
    assert_eq!(
        std::env::var("OMAVLESS_K1_SUPPORTED_SOCKET_WRITER").as_deref(),
        Ok("1")
    );
    assert_eq!(
        std::env::args().skip(1).collect::<Vec<_>>(),
        [
            "--exact",
            TEST,
            "--ignored",
            "--nocapture",
            "--test-threads=1"
        ]
    );
    let isolation = Isolation::capture().expect("K1_MANAGER_PRIVATE_ADMISSION_REFUSED");
    let held = Box::leak(Box::new(Held {
        isolation,
        stage: None,
        creator: None,
        state: None,
        independent: None,
    }));
    if !matches!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(held))),
        Ok(Ok(()))
    ) {
        eprintln!("K1_MANAGER_PRIVATE_UNCERTAIN_RETAINED");
        loop {
            std::thread::park();
        }
    }
    println!("K1_MANAGER_PRIVATE_NATIVE_PASS_NOT_CANONICAL");
}

#[test]
fn actual_credentials_require_only_net_admin_and_effective_filter() {
    let good = format!(
        "Uid:\t0 0 0 0\nGid:\t0 0 0 0\nGroups:\nCapInh:\t0000000000000000\nCapPrm:\t{CAP_NET_ADMIN}\nCapEff:\t{CAP_NET_ADMIN}\nCapBnd:\t{CAP_NET_ADMIN}\nCapAmb:\t0000000000000000\nNoNewPrivs:\t1\nSeccomp:\t2\n"
    );
    assert!(credentials(&good).is_ok());
    for line in good.lines() {
        assert!(credentials(&good.replace(&format!("{line}\n"), "")).is_err());
        assert!(credentials(&format!("{good}{line}\n")).is_err());
    }
    for (from, to) in [
        ("Uid:\t0 0 0 0", "Uid:\t0 0 1000 0"),
        ("Groups:\n", "Groups:\t0\n"),
        ("NoNewPrivs:\t1", "NoNewPrivs:\t0"),
        ("Seccomp:\t2", "Seccomp:\t0"),
        (CAP_NET_ADMIN, "0000000000201000"),
        ("CapAmb:\t0000000000000000", "CapAmb:\t0000000000001000"),
    ] {
        assert!(credentials(&good.replace(from, to)).is_err());
    }
}

#[test]
fn skipped_private_network_wrong_anchor_and_extra_interfaces_refuse() {
    assert!(distinct((5, 1), (5, 2), (5, 1)).is_ok());
    for (host, own, manager) in [
        ((5, 1), (5, 1), (5, 1)),
        ((5, 3), (5, 2), (5, 1)),
        ((5, 0), (5, 2), (5, 0)),
    ] {
        assert!(distinct(host, own, manager).is_err());
    }
    assert!(loopback_only("header\nheader\n lo: 0 0\n").is_ok());
    for value in [
        "",
        "header\nheader\n",
        "header\nheader\n eth0: 0\n",
        "header\nheader\n lo: 0\n eth0: 0\n",
    ] {
        assert!(loopback_only(value).is_err());
    }
}
