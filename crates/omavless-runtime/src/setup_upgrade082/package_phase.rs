// SPDX-License-Identifier: MIT
//! Explicit UPDATE-only developer VM phase. DNS is a separate newly admitted
//! operation. No downloaded helper authentication or general multiuser claim.
use super::*;
use std::io::BufRead;
use std::process::{Child, ExitStatus};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

const SOURCE: &str = "8d02f9d7dd1a9063a82e9dcd93cb19cf4c5d8524";
const VERSION: &str = "0.9.8";
struct Archive {
    path: PathBuf,
    file: File,
    identity: fs::Metadata,
    digest: String,
}
impl Archive {
    fn open(path: PathBuf, uid: u32, digest: &str, mode: u32) -> Result<Self> {
        let identity = fs::symlink_metadata(&path).map_err(|_| Blocker::StagingUnavailable)?;
        if !identity.is_file()
            || identity.uid() != uid
            || identity.mode() & 0o7777 != mode
            || identity.nlink() != 1
            || identity.len() == 0
            || identity.len() > 256 * 1024 * 1024
        {
            return Err(Blocker::StagingUnavailable);
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
            .open(&path)
            .map_err(|_| Blocker::StagingUnavailable)?;
        let mut archive = Self {
            path,
            file,
            identity,
            digest: digest.to_owned(),
        };
        archive.recheck()?;
        Ok(archive)
    }
    fn recheck(&mut self) -> Result<()> {
        if !same(
            &self.identity,
            &fs::symlink_metadata(&self.path).map_err(|_| Blocker::StagingUnavailable)?,
        ) || !same(
            &self.identity,
            &self
                .file
                .metadata()
                .map_err(|_| Blocker::StagingUnavailable)?,
        ) {
            return Err(Blocker::StagingUnavailable);
        }
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|_| Blocker::StagingUnavailable)?;
        let mut sha = Sha256::new();
        let mut total = 0;
        let mut buffer = [0; 65536];
        loop {
            let count = self
                .file
                .read(&mut buffer)
                .map_err(|_| Blocker::StagingUnavailable)?;
            if count == 0 {
                break;
            }
            total += count;
            if total > 256 * 1024 * 1024 {
                return Err(Blocker::StagingUnavailable);
            }
            sha.update(&buffer[..count]);
        }
        if format!("{:x}", sha.finalize()) != self.digest
            || !same(
                &self.identity,
                &self
                    .file
                    .metadata()
                    .map_err(|_| Blocker::StagingUnavailable)?,
            )
        {
            return Err(Blocker::StagingUnavailable);
        }
        Ok(())
    }
    fn member(&self, name: &'static str) -> Result<Vec<u8>> {
        let mut command = Command::new("/usr/bin/bsdtar");
        command.args(["-xOf"]).arg(&self.path).arg(name);
        let bytes =
            crate::production_observation::bounded_fixed_query(command, Duration::from_secs(8))
                .map_err(|_| Blocker::StagingUnavailable)?
                .into_bytes();
        if bytes.len() > 16384 {
            return Err(Blocker::StagingUnavailable);
        }
        Ok(bytes)
    }
}
struct Pair {
    staging: Directory,
    app: Archive,
    dns: Archive,
    app_identity: Vec<u8>,
    dns_receipt: Vec<u8>,
    app_digest: String,
}
fn pins(arch: &str) -> Result<(&'static str, &'static str)> {
    match arch {
        "x86_64" => Ok((
            "bf27584874c7e75d40f856739783f66fdf6af5652e37ba638232a2bc5a3c7750",
            "3309fb54ab92c5f1f30f6ba3c925ddc153619841dc4ae5640f4f279383f6b464",
        )),
        "aarch64" => Ok((
            "dade522493c935e0e7e3988ab918c7a5d52614c62479637559906c3cafa76892",
            "1ef867583c826506ec73b8f95c20f753d9acdbaf2caf5fcc5560ac189f62fb40",
        )),
        _ => Err(Blocker::StagingUnavailable),
    }
}
fn package_metadata(bytes: &[u8], name: &str, arch: &str) -> Result<()> {
    let text = std::str::from_utf8(bytes).map_err(|_| Blocker::StagingUnavailable)?;
    for (key, expected) in [
        ("pkgname", name.to_owned()),
        ("pkgver", format!("{VERSION}-1")),
        ("arch", arch.to_owned()),
    ] {
        let found = text
            .lines()
            .filter_map(|line| line.split_once(" = "))
            .filter(|(field, _)| *field == key)
            .map(|(_, value)| value)
            .collect::<Vec<_>>();
        if found != [expected.as_str()] {
            return Err(Blocker::StagingUnavailable);
        }
    }
    Ok(())
}
fn app_identity(bytes: &[u8], arch: &str) -> Result<String> {
    let text = std::str::from_utf8(bytes).map_err(|_| Blocker::StagingUnavailable)?;
    let mut fields = std::collections::BTreeMap::new();
    for line in text.lines() {
        let (key, value) = line.split_once('=').ok_or(Blocker::StagingUnavailable)?;
        if fields.insert(key, value).is_some() {
            return Err(Blocker::StagingUnavailable);
        }
    }
    if fields.len() != 6
        || fields.get("schemaVersion") != Some(&"3")
        || fields.get("sourceCommit") != Some(&SOURCE)
        || fields.get("architecture") != Some(&arch)
        || fields.get("productVersion") != Some(&VERSION)
        || fields.get("provenance") != Some(&"caller-supplied-prebuilt")
    {
        return Err(Blocker::StagingUnavailable);
    }
    let digest = fields
        .get("binarySha256")
        .ok_or(Blocker::StagingUnavailable)?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(Blocker::StagingUnavailable);
    }
    Ok((*digest).to_owned())
}
impl Pair {
    fn current() -> Result<Self> {
        let uid = Uid::current().as_raw();
        let arch = std::env::consts::ARCH;
        let root = PathBuf::from(std::env::var_os("HOME").ok_or(Blocker::Environment)?)
            .join(".cache/omavless/upgrade082");
        Self::at(root, uid, arch)
    }
    fn at(root: PathBuf, uid: u32, arch: &str) -> Result<Self> {
        let staging = Directory::pin(&root, uid)?;
        let (app_pin, dns_pin) = pins(arch)?;
        let mut app = Archive::open(
            root.join(format!("omavless-{VERSION}-1-{arch}.pkg.tar.zst")),
            uid,
            app_pin,
            0o600,
        )?;
        let mut dns = Archive::open(
            root.join(format!("omavless-dns-{VERSION}-1-{arch}.pkg.tar.zst")),
            uid,
            dns_pin,
            0o600,
        )?;
        package_metadata(&app.member(".PKGINFO")?, "omavless", arch)?;
        package_metadata(&dns.member(".PKGINFO")?, "omavless-dns", arch)?;
        let identity = app.member("usr/share/doc/omavless/build-identity.txt")?;
        let app_digest = app_identity(&identity, arch)?;
        let receipt = dns.member("usr/share/omavless-dns/source-receipt.json")?;
        let parsed: serde_json::Value =
            serde_json::from_slice(&receipt).map_err(|_| Blocker::StagingUnavailable)?;
        if parsed["schema"] != 1
            || parsed["architecture"] != arch
            || parsed["omavless_commit"] != SOURCE
            || parsed["package_flavor"] != "release"
            || parsed["broker_feature"] != "release-package"
        {
            return Err(Blocker::StagingUnavailable);
        }
        app.recheck()?;
        dns.recheck()?;
        staging.recheck()?;
        Ok(Self {
            staging,
            app,
            dns,
            app_identity: identity,
            dns_receipt: receipt,
            app_digest,
        })
    }
    fn recheck(&mut self) -> Result<()> {
        self.staging.recheck()?;
        self.app.recheck()?;
        self.dns.recheck()
    }
    fn installed(&self) -> Result<()> {
        let mut identity = Captured::read(
            Path::new("/usr/share/doc/omavless/build-identity.txt"),
            0,
            0o644,
            4096,
        )
        .map_err(|_| Blocker::PackageOutcomeUnverified)?;
        let mut receipt = Captured::read(
            Path::new("/usr/share/omavless-dns/source-receipt.json"),
            0,
            0o644,
            16384,
        )
        .map_err(|_| Blocker::PackageOutcomeUnverified)?;
        if identity.bytes != self.app_identity || receipt.bytes != self.dns_receipt {
            return Err(Blocker::PackageOutcomeUnverified);
        }
        // Reuse streaming hash/type checks without executing the new binary.
        let mut binary = Archive::open(
            PathBuf::from("/usr/bin/omavless"),
            0,
            &self.app_digest,
            0o755,
        )
        .map_err(|_| Blocker::PackageOutcomeUnverified)?;
        crate::managed_pair::ManagedPair::validate_package()
            .map_err(|_| Blocker::PackageOutcomeUnverified)?;
        let mut query = Command::new("/usr/bin/pacman");
        query
            .env("LC_ALL", "C")
            .args(["-Q", "omavless", "omavless-dns"]);
        if crate::production_observation::bounded_fixed_query(query, Duration::from_secs(3))
            .map_err(|_| Blocker::PackageOutcomeUnverified)?
            != "omavless 0.9.8-1\nomavless-dns 0.9.8-1\n"
        {
            return Err(Blocker::PackageOutcomeUnverified);
        }
        identity
            .recheck()
            .map_err(|_| Blocker::PackageOutcomeUnverified)?;
        receipt
            .recheck()
            .map_err(|_| Blocker::PackageOutcomeUnverified)?;
        binary
            .recheck()
            .map_err(|_| Blocker::PackageOutcomeUnverified)
    }
}

struct Signals(Vec<signal_hook::SigId>);
impl Signals {
    fn capture(cancel: &Arc<AtomicBool>) -> Result<Self> {
        let mut signals = Self(Vec::new());
        for signal in [
            signal_hook::consts::SIGINT,
            signal_hook::consts::SIGTERM,
            signal_hook::consts::SIGHUP,
        ] {
            signals.0.push(
                signal_hook::flag::register(signal, Arc::clone(cancel))
                    .map_err(|_| Blocker::Environment)?,
            );
        }
        Ok(signals)
    }
}
impl Drop for Signals {
    fn drop(&mut self) {
        for signal in &self.0 {
            signal_hook::low_level::unregister(*signal);
        }
    }
}

fn supervise(child: &mut Child) -> ExitStatus {
    // Never kill a submitted pacman/sudo child, release exclusions on timeout,
    // or detach it. Even cancellation waits for its original terminal outcome.
    // A wait error is outcome-unknown retained state, with no resend or bounded
    // completion promise. Fatal parent loss remains unknown, not rollback.
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status,
            Ok(None) | Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}
pub(super) fn consume(
    context: &mut UpgradeContext,
    cancel: &AtomicBool,
    submit: impl FnOnce() -> Result<Child>,
    verify: impl FnOnce(&mut UpgradeContext) -> Result<()>,
) -> Result<()> {
    // Private in-process seam: production supplies only its literal command;
    // tests supply owned inert children. No command/path enters CLI or IPC.
    context.recheck_private()?;
    if cancel.load(Ordering::Acquire) {
        return Err(Blocker::CancelledBeforeSubmission);
    }
    let mut child = submit()?;
    let status = supervise(&mut child);
    // Inspect actual installed/private state on both success and nonzero exit.
    let verified = verify(context);
    if !status.success() || verified.is_err() {
        return Err(Blocker::PackageOutcomeUnverified);
    }
    Ok(())
}

fn package_owner_before() -> Result<()> {
    let mut query = Command::new("/usr/bin/pacman");
    query.env("LC_ALL", "C").args(["-Q", "omavless"]);
    if crate::production_observation::bounded_fixed_query(query, Duration::from_secs(3))
        .map_err(|_| Blocker::Predecessor)?
        != "omavless 0.8.2-1\n"
    {
        return Err(Blocker::Predecessor);
    }
    let mut all = Command::new("/usr/bin/pacman");
    all.env("LC_ALL", "C").arg("-Qq");
    let inventory = crate::production_observation::bounded_fixed_query(all, Duration::from_secs(3))
        .map_err(|_| Blocker::Predecessor)?;
    if inventory
        .lines()
        .any(|name| name == "omavless-dns" || name == "omavless-dns-experimental")
    {
        return Err(Blocker::Predecessor);
    }
    let mut ownership = Command::new("/usr/bin/pacman");
    ownership.env("LC_ALL", "C").args([
        "-Qoq",
        "/usr/bin/omavless",
        "/usr/share/doc/omavless/build-identity.txt",
    ]);
    if crate::production_observation::bounded_fixed_query(ownership, Duration::from_secs(3))
        .map_err(|_| Blocker::Predecessor)?
        != "omavless\nomavless\n"
    {
        return Err(Blocker::Predecessor);
    }
    for path in [
        "/usr/lib/omavless-dns",
        "/usr/share/omavless-dns",
        "/usr/lib/systemd/system/omavless-dns-broker.service",
        "/run/omavless-dns",
    ] {
        if !absent(Path::new(path)) {
            return Err(Blocker::Predecessor);
        }
    }
    Ok(())
}
fn predecessor_units() -> Result<Vec<Captured>> {
    let mut captured = Vec::new();
    for (name, expected) in [
        (
            "omavless-runtime.service",
            include_bytes!("../../../../packaging/systemd/omavless-runtime.service").as_slice(),
        ),
        (
            "omavless-login-prepare.service",
            include_bytes!("../../../../packaging/systemd/omavless-login-prepare.service")
                .as_slice(),
        ),
    ] {
        let bytes = Captured::read(
            &PathBuf::from("/usr/lib/systemd/user").join(name),
            0,
            0o644,
            16384,
        )
        .map_err(|_| Blocker::Predecessor)?;
        if bytes.bytes != expected {
            return Err(Blocker::Predecessor);
        }
        captured.push(bytes);
    }
    Ok(captured)
}
fn after_packages(context: &mut UpgradeContext, pair: &Pair) -> Result<()> {
    // Check actual installation even when pacman reports failure. No automatic
    // replay, rollback, daemon reload, DNS enrollment or next phase follows.
    pair.installed()?;
    context
        .recheck_private()
        .map_err(|_| Blocker::PackageOutcomeUnverified)?;
    crate::cutover_activation::environment::with_current(|| {
        crate::production_observation::ProductionOwnershipObserver::current()
            .and_then(|observer| observer.verify_native_empty())
    })
    .map_err(|_| Blocker::PackageOutcomeUnverified)?
    .map_err(|_| Blocker::PackageOutcomeUnverified)?;
    // NeedDaemonReload may legitimately become yes after package replacement.
    // Both services must stay settled, with the original disabled/static state.
    for (index, name) in ["omavless-runtime.service", "omavless-login-prepare.service"]
        .into_iter()
        .enumerate()
    {
        let mut query = Command::new("/usr/bin/systemctl");
        query.env_clear().env("PATH", "/usr/bin").env("LC_ALL", "C")
            .env("XDG_RUNTIME_DIR", format!("/run/user/{}", Uid::current().as_raw())).args(["--user", "show", name, "--no-pager",
            "--property=LoadState,ActiveState,SubState,MainPID,Job,FragmentPath,DropInPaths,UnitFileState,NeedDaemonReload,ExecStart"]);
        let raw = crate::production_observation::bounded_fixed_query(query, Duration::from_secs(2))
            .map_err(|_| Blocker::PackageOutcomeUnverified)?;
        let normalized = raw
            .lines()
            .map(|line| {
                if line == "NeedDaemonReload=yes" {
                    "NeedDaemonReload=no"
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        validate_unit(&normalized, name).map_err(|_| Blocker::PackageOutcomeUnverified)?;
        if normalized != context.units[index] {
            return Err(Blocker::PackageOutcomeUnverified);
        }
    }
    context
        .recheck_private()
        .map_err(|_| Blocker::PackageOutcomeUnverified)
}

pub fn run_development_update() -> Result<serde_json::Value> {
    let cancel = Arc::new(AtomicBool::new(false));
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return Err(Blocker::Environment);
    }
    eprintln!(
        "Development single-user VM only; other users are not excluded. Installs the pinned 0.9.8 app/DNS pair. DNS setup is a separate operation. Type UPGRADE SINGLE-USER VM:"
    );
    let mut consent = String::new();
    std::io::stdin()
        .lock()
        .take(128)
        .read_line(&mut consent)
        .map_err(|_| Blocker::Environment)?;
    if consent != "UPGRADE SINGLE-USER VM\n" || cancel.load(Ordering::Acquire) {
        return Err(Blocker::CancelledBeforeSubmission);
    }
    let _signals = Signals::capture(&cancel)?;
    let mut pair = Pair::current()?;
    let mut context = acquire_current()?;
    let mut unit_files = predecessor_units()?;
    package_owner_before()?;
    for name in ["managed-dns-selection", ".managed-dns-selection.candidate"] {
        if !absent(&context.scope.config.join(name)) {
            return Err(Blocker::Predecessor);
        }
    }
    context.recheck()?;
    pair.recheck()?;
    for unit in &mut unit_files {
        unit.recheck()?;
    }
    if cancel.load(Ordering::Acquire) {
        return Err(Blocker::CancelledBeforeSubmission);
    }
    let mut command = Command::new("/usr/bin/sudo");
    command
        .args(["/usr/bin/pacman", "-U", "--"])
        .arg(&pair.app.path)
        .arg(&pair.dns.path);
    consume(
        &mut context,
        &cancel,
        || {
            command
                .spawn()
                .map_err(|_| Blocker::PackageSubmissionUnavailable)
        },
        |context| after_packages(context, &pair),
    )?;
    // Original exclusions end only here, after known exit and independent
    // verification. The result is DATA; NEW DNS setup must re-earn admission.
    drop(context);
    Ok(
        serde_json::json!({"schemaVersion":1,"scope":"single_user_development_vm",
        "outcome":"packages_updated_dns_required","dnsSetupRequired":true,"startAllowed":false,
        "cancellationRequested":cancel.load(Ordering::Acquire)}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pinned_metadata_is_closed_and_duplicate_refuses() {
        let good = b"pkgname = omavless\npkgver = 0.9.8-1\narch = x86_64\n";
        assert_eq!(package_metadata(good, "omavless", "x86_64"), Ok(()));
        assert!(package_metadata(good, "omavless-dns", "x86_64").is_err());
        assert!(
            package_metadata(
                &[good.as_slice(), b"pkgname = omavless\n"].concat(),
                "omavless",
                "x86_64"
            )
            .is_err()
        );
        assert!(pins("unknown").is_err());
    }
    #[test]
    fn supervised_original_child_has_known_success_and_nonzero_outcomes() {
        let mut yes = Command::new("/usr/bin/true").spawn().unwrap();
        assert!(supervise(&mut yes).success());
        let mut no = Command::new("/usr/bin/false").spawn().unwrap();
        assert!(!supervise(&mut no).success());
    }
    #[test]
    fn staged_archive_rechecks_actual_owned_inode_bytes_and_published_pins() {
        use std::os::unix::fs::PermissionsExt;
        let root = crate::test_temp::directory("up082archive").unwrap();
        let path = root.join("synthetic-archive");
        fs::write(&path, b"synthetic-not-a-public-package").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let digest = format!("{:x}", Sha256::digest(fs::read(&path).unwrap()));
        let mut archive =
            Archive::open(path.clone(), Uid::current().as_raw(), &digest, 0o600).unwrap();
        fs::rename(&path, root.join("original-held-archive")).unwrap();
        fs::write(&path, b"synthetic-not-a-public-package").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(archive.recheck(), Err(Blocker::StagingUnavailable));
        assert!(
            Archive::open(
                path.clone(),
                Uid::current().as_raw(),
                &"a".repeat(64),
                0o600
            )
            .is_err()
        );
        for (index, filename) in ["runtime-release.json", "dns-release.json"]
            .into_iter()
            .enumerate()
        {
            let source = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../plugin")
                .join(filename);
            let data: serde_json::Value =
                serde_json::from_slice(&fs::read(source).unwrap()).unwrap();
            for arch in ["x86_64", "aarch64"] {
                let pair = pins(arch).unwrap();
                let pinned = if index == 0 { pair.0 } else { pair.1 };
                assert_eq!(data["packages"][arch]["sha256"], pinned);
                assert_eq!(data["packages"][arch]["sourceCommit"], SOURCE);
            }
        }
        drop(archive);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn optional_real_pinned_archives_are_inspected_without_execution_or_effects() {
        use std::os::unix::fs::PermissionsExt;
        // Private test-only inputs. Production accepts no path/UID/archive
        // injection; CI may omit retained public artifacts entirely.
        let (Some(app), Some(dns)) = (
            std::env::var_os("OMAVLESS_TEST_UPGRADE_APP_ARCHIVE"),
            std::env::var_os("OMAVLESS_TEST_UPGRADE_DNS_ARCHIVE"),
        ) else {
            return;
        };
        let root = crate::test_temp::directory("up082realpair").unwrap();
        let arch = std::env::consts::ARCH;
        for (source, name) in [(app, "omavless"), (dns, "omavless-dns")] {
            let destination = root.join(format!("{name}-{VERSION}-1-{arch}.pkg.tar.zst"));
            fs::copy(source, &destination).unwrap();
            fs::set_permissions(destination, fs::Permissions::from_mode(0o600)).unwrap();
        }
        let mut pair = Pair::at(root.clone(), Uid::current().as_raw(), arch).unwrap();
        pair.recheck().unwrap();
        assert_eq!(
            app_identity(&pair.app_identity, arch).unwrap(),
            pair.app_digest
        );
        drop(pair);
        fs::remove_dir_all(root).unwrap();
    }
}
