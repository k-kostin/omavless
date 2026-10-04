// SPDX-License-Identifier: MIT
//! Source-only disposable-real-UID scaffold. No production hook or authority.
//! Ignored entries cannot create accounts or start managers/applications.
use sha2::{Digest, Sha256};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const UID: u32 = 48048;
const HOME: &str = "/home/ov-t4-abort-v5";
const RUNTIME: &str = "/run/user/48048";
const ARTIFACTS: &str = "/home/ov-t4-abort-v5/.t4-first-abort";
const HELPER: &str = "/home/ov-t4-abort-v5/.t4-first-abort/helper";
const CLI: &str = "/home/ov-t4-abort-v5/.t4-first-abort/omavless";
const LAUNCH: &str = "production_owner::first_abort::cli_vm_fixture::launch_normal_cli";
const SETUP: &str = "production_owner::first_abort::cli_vm_fixture::setup_mixed_intent";
const VERIFY_FIRST: &str = "production_owner::first_abort::cli_vm_fixture::verify_first_abort";
const VERIFY_REENTRY: &str = "production_owner::first_abort::cli_vm_fixture::verify_abort_reentry";
const PASS: &[u8] = b"synthetic disposable UID first Abort input";
const MAX_ELF: u64 = 512 * 1024 * 1024;
type Result<T> = std::result::Result<T, ()>;

fn require(value: bool) -> Result<()> {
    value.then_some(()).ok_or(())
}

fn bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = File::open(path).map_err(|_| ())?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    require(bytes.len() as u64 <= limit)?;
    Ok(bytes)
}

fn credentials(status: &str) -> bool {
    if status.len() > 65536 {
        return false;
    }
    [
        "Uid",
        "Gid",
        "Groups",
        "CapInh",
        "CapPrm",
        "CapEff",
        "CapAmb",
        "NoNewPrivs",
    ]
    .iter()
    .all(|key| {
        let mut values = status.lines().filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            (name == *key).then_some(value.trim())
        });
        let Some(value) = values.next() else {
            return false;
        };
        values.next().is_none()
            && match *key {
                "Uid" | "Gid" => value.split_whitespace().collect::<Vec<_>>() == ["48048"; 4],
                "Groups" => value.is_empty(),
                "NoNewPrivs" => value == "1",
                _ => value == "0000000000000000",
            }
    })
}

fn credential_gate() -> Result<()> {
    let bytes = bounded(Path::new("/proc/self/status"), 65536)?;
    require(credentials(std::str::from_utf8(&bytes).map_err(|_| ())?))?;
    require(std::env::var("HOME").as_deref() == Ok(HOME))?;
    require(std::env::var("XDG_RUNTIME_DIR").as_deref() == Ok(RUNTIME))?;
    require(std::env::var("PATH").as_deref() == Ok("/usr/bin"))?;
    for name in [
        "OMAVLESS_HOME",
        "OMAVLESS_MIHOMO",
        "XDG_STATE_HOME",
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
    ] {
        require(std::env::var_os(name).is_none())?;
    }
    Ok(())
}

fn file_identity(left: &Metadata, right: &Metadata) -> bool {
    [
        left.dev(),
        left.ino(),
        u64::from(left.uid()),
        u64::from(left.gid()),
        u64::from(left.mode()),
        left.nlink(),
        left.len(),
        left.mtime() as u64,
        left.mtime_nsec() as u64,
        left.ctime() as u64,
        left.ctime_nsec() as u64,
    ] == [
        right.dev(),
        right.ino(),
        u64::from(right.uid()),
        u64::from(right.gid()),
        u64::from(right.mode()),
        right.nlink(),
        right.len(),
        right.mtime() as u64,
        right.mtime_nsec() as u64,
        right.ctime() as u64,
        right.ctime_nsec() as u64,
    ]
}

fn directory_identity(left: &Metadata, right: &Metadata) -> bool {
    (left.dev(), left.ino(), left.uid(), left.gid(), left.mode())
        == (
            right.dev(),
            right.ino(),
            right.uid(),
            right.gid(),
            right.mode(),
        )
}

struct Ancestors(Vec<(PathBuf, File, Metadata)>);
impl Ancestors {
    fn capture() -> Result<Self> {
        let mut held = Vec::new();
        for path in Path::new(ARTIFACTS).ancestors() {
            let original = fs::symlink_metadata(path).map_err(|_| ())?;
            require(
                original.is_dir()
                    && [0, UID].contains(&original.uid())
                    && original.mode() & 0o6022 == 0,
            )?;
            require(matches!(fs::symlink_metadata(path.join(".git")),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound))?;
            let fd = OpenOptions::new()
                .read(true)
                .custom_flags(nix::libc::O_DIRECTORY | nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
                .open(path)
                .map_err(|_| ())?;
            require(directory_identity(
                &original,
                &fd.metadata().map_err(|_| ())?,
            ))?;
            held.push((path.to_owned(), fd, original));
        }
        require(
            held[0].2.uid() == UID && held[0].2.gid() == UID && held[0].2.mode() & 0o7777 == 0o700,
        )?;
        Ok(Self(held))
    }
    fn recheck(&self) -> Result<()> {
        for (path, fd, original) in &self.0 {
            require(
                directory_identity(original, &fd.metadata().map_err(|_| ())?)
                    && directory_identity(original, &fs::symlink_metadata(path).map_err(|_| ())?),
            )?;
        }
        Ok(())
    }
}

struct Namespace(Vec<(&'static str, File, Metadata)>);
impl Namespace {
    fn capture() -> Result<Self> {
        let mut held = Vec::new();
        for (name, expected) in [
            ("pid", "OV_T4_NS_PID"),
            ("user", "OV_T4_NS_USER"),
            ("mnt", "OV_T4_NS_MNT"),
            ("net", "OV_T4_NS_NET"),
        ] {
            let file = File::open(format!("/proc/self/ns/{name}")).map_err(|_| ())?;
            let metadata = file.metadata().map_err(|_| ())?;
            // Root supplies its held/checked namespace identities. An ordinary
            // UID cannot assume ptrace access to /proc/1/ns. The normal CLI
            // independently anchors its real user manager through systemd.
            let expected = std::env::var(expected).map_err(|_| ())?;
            require(expected == format!("{}:{}", metadata.dev(), metadata.ino()))?;
            held.push((name, file, metadata));
        }
        Ok(Self(held))
    }
    fn recheck(&self) -> Result<()> {
        for (name, file, original) in &self.0 {
            let named = fs::metadata(format!("/proc/self/ns/{name}")).map_err(|_| ())?;
            let held = file.metadata().map_err(|_| ())?;
            require(
                (original.dev(), original.ino()) == (held.dev(), held.ino())
                    && (held.dev(), held.ino()) == (named.dev(), named.ino()),
            )?;
        }
        Ok(())
    }
}

fn expected_hash(name: &str) -> Result<String> {
    let value = std::env::var(name).map_err(|_| ())?;
    require(
        value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
    )?;
    Ok(value)
}

fn readonly_fd(fd: u32) -> Result<()> {
    let bytes = bounded(Path::new(&format!("/proc/self/fdinfo/{fd}")), 4096)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| ())?;
    let mut flags = text
        .lines()
        .filter_map(|line| line.strip_prefix("flags:\t"));
    let value = u32::from_str_radix(flags.next().ok_or(())?, 8).map_err(|_| ())?;
    require(flags.next().is_none() && value & nix::libc::O_ACCMODE as u32 == 0)
}

struct Elf {
    file: File,
    original: Metadata,
    path: &'static str,
    fd: u32,
    sha: String,
}
impl Elf {
    fn capture(path: &'static str, fd: u32, sha: String) -> Result<Self> {
        readonly_fd(fd)?;
        require(fs::read_link(format!("/proc/self/fd/{fd}")).map_err(|_| ())? == Path::new(path))?;
        let file = File::open(format!("/proc/self/fd/{fd}")).map_err(|_| ())?;
        let original = file.metadata().map_err(|_| ())?;
        require(
            original.is_file()
                && original.uid() == UID
                && original.gid() == UID
                && original.mode() & 0o7777 == 0o500
                && original.nlink() == 1
                && (4..=MAX_ELF).contains(&original.len()),
        )?;
        let result = Self {
            file,
            original,
            path,
            fd,
            sha,
        };
        result.recheck()?;
        Ok(result)
    }
    fn recheck(&self) -> Result<()> {
        use std::os::unix::fs::FileExt;
        readonly_fd(self.fd)?;
        require(
            fs::read_link(format!("/proc/self/fd/{}", self.fd)).map_err(|_| ())?
                == Path::new(self.path),
        )?;
        require(
            file_identity(&self.original, &self.file.metadata().map_err(|_| ())?)
                && file_identity(
                    &self.original,
                    &fs::metadata(format!("/proc/self/fd/{}", self.fd)).map_err(|_| ())?,
                )
                && file_identity(
                    &self.original,
                    &fs::symlink_metadata(self.path).map_err(|_| ())?,
                ),
        )?;
        let mut attrs = [0_u8; 1];
        require(rustix::fs::flistxattr(&self.file, &mut attrs[..]).map_err(|_| ())? == 0)?;
        let mut magic = [0_u8; 4];
        require(
            self.file.read_at(&mut magic, 0).map_err(|_| ())? == magic.len()
                && magic == *b"\x7fELF",
        )?;
        let mut hash = Sha256::new();
        let mut position = 0;
        let mut bytes = [0_u8; 32768];
        while position < self.original.len() {
            let count = self.file.read_at(&mut bytes, position).map_err(|_| ())?;
            require(count > 0)?;
            position += count as u64;
            require(position <= self.original.len())?;
            hash.update(&bytes[..count]);
        }
        require(
            self.file
                .read_at(&mut bytes[..1], position)
                .map_err(|_| ())?
                == 0,
        )?;
        require(format!("{:x}", hash.finalize()) == self.sha)?;
        require(
            file_identity(&self.original, &self.file.metadata().map_err(|_| ())?)
                && file_identity(
                    &self.original,
                    &fs::metadata(format!("/proc/self/fd/{}", self.fd)).map_err(|_| ())?,
                )
                && file_identity(
                    &self.original,
                    &fs::symlink_metadata(self.path).map_err(|_| ())?,
                ),
        )
    }
}

fn admit(entry: &'static str) -> Result<(Ancestors, Namespace, Elf, Elf)> {
    require(
        std::env::args().skip(1).collect::<Vec<_>>()
            == [
                "--exact",
                entry,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
                "--quiet",
            ],
    )?;
    credential_gate()?;
    let ancestors = Ancestors::capture()?;
    let namespaces = Namespace::capture()?;
    // FD198/199 are reserved by the fixed reviewed root guard, never selected
    // by a caller or accepted by the normal backend. Both are read-only.
    let helper = Elf::capture(HELPER, 198, expected_hash("OV_T4_HELPER_SHA256")?)?;
    require(file_identity(
        &helper.original,
        &fs::metadata("/proc/self/exe").map_err(|_| ())?,
    ))?;
    let cli = Elf::capture(CLI, 199, expected_hash("OV_T4_CLI_SHA256")?)?;
    ancestors.recheck()?;
    namespaces.recheck()?;
    helper.recheck()?;
    cli.recheck()?;
    credential_gate()?;
    Ok((ancestors, namespaces, helper, cli))
}

fn launch() -> Result<()> {
    let (_ancestors, _namespaces, helper, _cli) = admit(LAUNCH)?;
    // No helper/evidence/privileged FD is inherited by the normal CLI. FD199
    // alone remains, deliberately read-only, to execute the original CLI inode.
    nix::unistd::close(198).map_err(|_| ())?;
    drop(helper);
    let error = Command::new("/proc/self/fd/199")
        .arg0(CLI)
        .args(["restore", "abort", "--confirm-rollback"])
        .env_clear()
        .env("HOME", HOME)
        .env("PATH", "/usr/bin")
        .env("LC_ALL", "C")
        .env("XDG_RUNTIME_DIR", RUNTIME)
        .exec();
    let _ = error;
    Err(())
}

fn create_private(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| ())?;
    file.write_all(bytes).map_err(|_| ())?;
    file.sync_all().map_err(|_| ())?;
    let metadata = file.metadata().map_err(|_| ())?;
    require(
        metadata.uid() == UID
            && metadata.gid() == UID
            && metadata.mode() & 0o7777 == 0o600
            && metadata.nlink() == 1
            && metadata.len() == bytes.len() as u64
            && file_identity(&metadata, &fs::symlink_metadata(path).map_err(|_| ())?),
    )?;
    File::open(path.parent().ok_or(())?)
        .map_err(|_| ())?
        .sync_all()
        .map_err(|_| ())
}

fn old_pair() -> Result<[Vec<u8>; 2]> {
    let backup = crate::restore_successor_publication_candidate::tests::backup();
    let mut store: serde_json::Value = serde_json::from_slice(backup.store()).map_err(|_| ())?;
    store["profiles"][0]["name"] = serde_json::json!("Synthetic OLD disposable UID");
    store["startup"]["enabled"] = serde_json::json!(false);
    store["startup"]["mode"] = serde_json::json!("global");
    // The archive admits exact bundled variants, not arbitrary YAML/comments.
    let template = omavless_domain::routing::template_with_mode(
        std::str::from_utf8(backup.template()).map_err(|_| ())?,
        "global",
    )
    .map_err(|_| ())?
    .into_bytes();
    Ok([serde_json::to_vec(&store).map_err(|_| ())?, template])
}

fn setup() -> Result<()> {
    let (ancestors, namespaces, helper, cli) = admit(SETUP)?;
    // All paths are the actual current paths for this separate real account.
    // No HOME/XDG substitution or fabricated Mihomo executable is used.
    let runtime = crate::RuntimePaths::current().map_err(|_| ())?;
    let desired = crate::desired::DesiredPaths::current().map_err(|_| ())?;
    let paths = crate::cutover::CutoverPaths::current(UID).map_err(|_| ())?;
    let config = Path::new(HOME).join(".config/omavless");
    for path in [
        PathBuf::from(format!("{HOME}/.config")),
        config.clone(),
        PathBuf::from(format!("{HOME}/.local")),
        PathBuf::from(format!("{HOME}/.local/state")),
        desired.directory.clone(),
        runtime.directory.clone(),
    ] {
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .map_err(|_| ())?;
        let metadata = fs::symlink_metadata(&path).map_err(|_| ())?;
        require(
            metadata.is_dir()
                && metadata.uid() == UID
                && metadata.gid() == UID
                && metadata.mode() & 0o7777 == 0o700,
        )?;
        File::open(path.parent().ok_or(())?)
            .map_err(|_| ())?
            .sync_all()
            .map_err(|_| ())?;
    }
    let host = crate::native_host::NativeHostPaths::current(&runtime.directory).map_err(|_| ())?;
    require(
        host.core
            == Path::new("/usr/bin/mihomo")
                .canonicalize()
                .map_err(|_| ())?,
    )?;
    let archive_path = Path::new(ARTIFACTS).join("archive.ovb");
    let backup = crate::restore_successor_publication_candidate::tests::backup();
    let archive = omavless_domain::private_backup::seal(backup.store(), backup.template(), PASS)
        .map_err(|_| ())?;
    create_private(&archive_path, &archive)?;
    let opened = crate::backup_destination_candidate::open_existing(&archive_path, UID, PASS)
        .map_err(|_| ())?;
    let old = old_pair()?;
    // Validate the synthetic OLD pair with the real archive schema as well.
    omavless_domain::private_backup::seal(&old[0], &old[1], PASS).map_err(|_| ())?;
    create_private(&runtime.owner_lock, b"")?;
    create_private(
        &paths.ownership_marker,
        br#"{"schemaVersion":1,"generation":2,"phase":"rust"}"#,
    )?;
    crate::desired::write_desired(&desired, UID, &crate::desired::DesiredState::default())
        .map_err(|_| ())?;
    for (name, bytes) in ["profiles.json", "route-template.yaml"]
        .into_iter()
        .zip(&old)
    {
        create_private(&config.join(name), bytes)?;
    }
    let incoming = opened.restore_store_off().map_err(|_| ())?;
    crate::restore_staging_candidate::stage_private_pair(
        &paths.state_directory,
        UID,
        &old[0],
        &old[1],
        &incoming,
        opened.template(),
    )
    .map_err(|_| ())?;
    let lock = crate::cutover::MigrationLock::acquire(&paths, UID).map_err(|_| ())?;
    let mut reached = false;
    let result = crate::restore_executor_candidate::execute_with_hook(
        &config,
        &paths,
        UID,
        2,
        &lock,
        [117; 16],
        || true,
        |step| {
            if step == crate::restore_executor_candidate::EffectStep::Renamed(0) {
                reached = true;
                false
            } else {
                true
            }
        },
    );
    require(reached && result.is_err())?;
    require(
        fs::read(config.join("profiles.json")).map_err(|_| ())? == incoming.as_slice()
            && fs::read(config.join("route-template.yaml")).map_err(|_| ())? == old[1],
    )?;
    require(
        crate::restore_journal_candidate::inspect_decision_journal(&paths, UID, &lock)
            .map_err(|_| ())?
            .active()
            .phase()
            == crate::restore_decision_candidate::DecisionPhase::Intent,
    )?;
    drop(lock);
    let input = serde_json::to_vec(&serde_json::json!({"schema":1,"archive":archive_path,
        "passphrase":std::str::from_utf8(PASS).map_err(|_| ())?}))
    .map_err(|_| ())?;
    create_private(&Path::new(ARTIFACTS).join("request.json"), &input)?;
    ancestors.recheck()?;
    namespaces.recheck()?;
    helper.recheck()?;
    cli.recheck()?;
    create_private(
        &Path::new(ARTIFACTS).join("setup.json"),
        br#"{"schema":1,"state":"MIXED_FIRST_INTENT","synthetic":true,"product_pass":false}"#,
    )
}

fn verify(entry: &'static str, receipt: &'static str) -> Result<()> {
    let (ancestors, namespaces, helper, cli) = admit(entry)?;
    let paths = crate::cutover::CutoverPaths::current(UID).map_err(|_| ())?;
    let lock = crate::cutover::MigrationLock::acquire_existing(&paths, UID).map_err(|_| ())?;
    let chain = crate::restore_journal_candidate::inspect_decision_journal(&paths, UID, &lock)
        .map_err(|_| ())?;
    require(chain.active().phase() == crate::restore_decision_candidate::DecisionPhase::Aborted)?;
    let staged = crate::restore_staging_candidate::read_staged_pair(&paths.state_directory, UID)
        .map_err(|_| ())?;
    let old = old_pair()?;
    require(staged.old_store() == old[0] && staged.old_template() == old[1])?;
    for (name, bytes) in ["profiles.json", "route-template.yaml"]
        .into_iter()
        .zip(&old)
    {
        require(
            fs::read(Path::new(HOME).join(".config/omavless").join(name)).map_err(|_| ())?
                == *bytes,
        )?;
    }
    let desired = crate::desired::read_desired_snapshot(
        &crate::desired::DesiredPaths::current().map_err(|_| ())?,
        UID,
    )
    .map_err(|_| ())?;
    require(!desired.connected)?;
    ancestors.recheck()?;
    namespaces.recheck()?;
    helper.recheck()?;
    cli.recheck()?;
    create_private(&Path::new(ARTIFACTS).join(receipt),
        br#"{"schema":1,"state":"ABORTED_OLD_PAIR_STILL_FENCED","synthetic":true,"product_pass":false}"#)
}

fn fixed_result(result: Result<()>) {
    if result.is_err() {
        eprintln!("T4_DISPOSABLE_CLI_FIXTURE_NONPASS");
        std::process::exit(2);
    }
}

#[test]
#[ignore = "fixed disposable-UID synthetic setup; root review and VM lease required"]
fn setup_mixed_intent() {
    fixed_result(setup());
}

#[test]
#[ignore = "fixed fresh disposable-UID read-only admission observation; root review and VM lease required"]
fn diagnose_stopped_admission() {
    let result = (|| {
        let _pins = admit(crate::restore_abort_cli::diagnostic::ENTRY)?;
        crate::restore_abort_cli::diagnose_current_stopped()
    })();
    if result.is_err() {
        // In particular, a failed diagnostic sink must never trigger another
        // stderr write through the generic fixture result or test harness.
        std::process::exit(2);
    }
}

#[test]
#[ignore = "fixed disposable-UID read-only pair verification and exclusive receipt"]
fn verify_first_abort() {
    fixed_result(verify(VERIFY_FIRST, "after-first.json"));
}

#[test]
#[ignore = "fixed disposable-UID read-only reentry verification and exclusive receipt"]
fn verify_abort_reentry() {
    fixed_result(verify(VERIFY_REENTRY, "after-reentry.json"));
}

#[test]
#[ignore = "fixed disposable-UID CLI launcher; root review and VM lease required"]
fn launch_normal_cli() {
    if launch().is_err() {
        eprintln!("T4_DISPOSABLE_CLI_PREEXEC_NONPASS");
        std::process::exit(2);
    }
}

#[test]
fn credential_parser_requires_all_saved_ids_empty_groups_zero_caps_and_nnp() {
    let good = "Uid:\t48048\t48048\t48048\t48048\nGid:\t48048\t48048\t48048\t48048\nGroups:\t\nCapInh:\t0000000000000000\nCapPrm:\t0000000000000000\nCapEff:\t0000000000000000\nCapAmb:\t0000000000000000\nNoNewPrivs:\t1\n";
    assert!(credentials(good));
    for bad in [
        good.replace("NoNewPrivs:\t1", "NoNewPrivs:\t0"),
        good.replacen("48048\t48048\t48048\t48048", "48048\t48048\t0\t48048", 1),
        good.replace("Gid:\t48048", "Gid:\t0"),
        good.replace("Groups:\t", "Groups:\t48048"),
        format!("{good}Uid:\t48048\t48048\t48048\t48048\n"),
        good.replace("CapAmb:", "Missing:"),
    ] {
        assert!(!credentials(&bad));
    }
    for name in ["CapInh", "CapPrm", "CapEff", "CapAmb"] {
        assert!(!credentials(&good.replace(
            &format!("{name}:\t0000000000000000"),
            &format!("{name}:\t0000000000000001")
        )));
    }
    // Bounding capabilities are intentionally not claimed zero.
    assert!(credentials(&format!("{good}CapBnd:\t000001ffffffffff\n")));
}

#[test]
fn synthetic_old_pair_is_valid_distinct_and_never_requests_startup() {
    let old = old_pair().unwrap();
    let archive = omavless_domain::private_backup::seal(&old[0], &old[1], PASS).unwrap();
    let authenticated = omavless_domain::private_backup::open(&archive, PASS).unwrap();
    assert_eq!(authenticated.store(), old[0]);
    assert_eq!(authenticated.template(), old[1]);
    let source = crate::restore_successor_publication_candidate::tests::backup();
    assert_ne!(old[0], source.restore_store_off().unwrap().as_slice());
    assert_ne!(old[1], source.template());
    let store: serde_json::Value = serde_json::from_slice(&old[0]).unwrap();
    assert_eq!(store["startup"]["enabled"], false);
}
