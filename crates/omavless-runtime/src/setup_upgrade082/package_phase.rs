// SPDX-License-Identifier: MIT
//! Exact public-0.8.2 first package UPDATE. The frontend independently qualifies
//! release archives/helper; closed stdin is DATA and native checks consistency.
//! DNS is a NEW admitted operation. Other-owner observation is not persistent
//! all-user exclusion; no foreign private files or stock core are changed.
use super::delivery_metadata::Metadata;
use super::*;
use std::io::Write;
use std::os::unix::fs::FileTypeExt;
use std::process::{Child, ExitStatus, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) const SOURCE: &str = "8d02f9d7dd1a9063a82e9dcd93cb19cf4c5d8524";
pub(super) const VERSION: &str = "0.9.8";

fn guest_identity(vendor: &[u8], product: &[u8]) -> bool {
    vendor == b"QEMU\n" && product == b"Standard PC (Q35 + ICH9, 2009)\n"
}

fn development_guest() -> Result<()> {
    // Fixed kernel DMI attributes of the selected QEMU/q35 development target.
    // No caller path/env bypass. DMI narrows the target; it neither proves
    // single-user custody nor replaces package exclusion/helper authenticity.
    let read = |name: &str| -> Result<Vec<u8>> {
        let path = Path::new("/sys/devices/virtual/dmi/id").join(name);
        let before =
            fs::symlink_metadata(&path).map_err(|_| Blocker::UnsupportedDevelopmentGuest)?;
        if !before.is_file()
            || before.uid() != 0
            || before.mode() & 0o022 != 0
            || before.nlink() != 1
        {
            return Err(Blocker::UnsupportedDevelopmentGuest);
        }
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
            .open(&path)
            .map_err(|_| Blocker::UnsupportedDevelopmentGuest)?;
        if !same(
            &before,
            &file
                .metadata()
                .map_err(|_| Blocker::UnsupportedDevelopmentGuest)?,
        ) {
            return Err(Blocker::UnsupportedDevelopmentGuest);
        }
        let mut bytes = Vec::new();
        (&mut file)
            .take(129)
            .read_to_end(&mut bytes)
            .map_err(|_| Blocker::UnsupportedDevelopmentGuest)?;
        if bytes.len() > 128
            || !same(
                &before,
                &file
                    .metadata()
                    .map_err(|_| Blocker::UnsupportedDevelopmentGuest)?,
            )
            || !same(
                &before,
                &fs::symlink_metadata(path).map_err(|_| Blocker::UnsupportedDevelopmentGuest)?,
            )
        {
            return Err(Blocker::UnsupportedDevelopmentGuest);
        }
        Ok(bytes)
    };
    if !guest_identity(&read("sys_vendor")?, &read("product_name")?) {
        return Err(Blocker::UnsupportedDevelopmentGuest);
    }
    Ok(())
}
pub(super) struct Archive {
    path: PathBuf,
    file: File,
    identity: fs::Metadata,
    digest: String,
}
impl Archive {
    pub(super) fn open(path: PathBuf, uid: u32, digest: &str, mode: u32) -> Result<Self> {
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
    pub(super) fn recheck(&mut self) -> Result<()> {
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
    metadata: Metadata,
}
pub(super) fn pins(arch: &str) -> Result<(&'static str, &'static str)> {
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
fn package_metadata(bytes: &[u8], name: &str, arch: &str, version: &str) -> Result<()> {
    let text = std::str::from_utf8(bytes).map_err(|_| Blocker::StagingUnavailable)?;
    for (key, expected) in [
        ("pkgname", name.to_owned()),
        ("pkgver", format!("{version}-1")),
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
pub(super) fn app_identity(
    bytes: &[u8],
    arch: &str,
    source: &str,
    version: &str,
) -> Result<String> {
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
        || fields.get("sourceCommit") != Some(&source)
        || fields.get("architecture") != Some(&arch)
        || fields.get("productVersion") != Some(&version)
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
// Archive hash and selected source identity are independently qualified above.
// Preserve immutable schema1 predecessors; the new family additionally passes
// its strict raw decoder, never a schema/ABI label alone.
fn dns_receipt_matches(raw: &[u8], arch: &str, source: &str) -> bool {
    let Ok(parsed) = serde_json::from_slice::<serde_json::Value>(raw) else {
        return false;
    };
    (parsed["schema"] == 1 || crate::managed_traffic_receipt::decode(raw, arch).is_ok())
        && parsed["architecture"] == arch
        && parsed["omavless_commit"] == source
        && parsed["package_flavor"] == "release"
        && parsed["broker_feature"] == "release-package"
}

impl Pair {
    fn current(metadata: Metadata) -> Result<Self> {
        let uid = Uid::current().as_raw();
        let root = PathBuf::from(std::env::var_os("HOME").ok_or(Blocker::Environment)?)
            .join(".cache/omavless/upgrade082")
            .join(format!("{}.{}", metadata.app_sha, metadata.dns_sha));
        Self::at(root, uid, metadata)
    }
    fn at(root: PathBuf, uid: u32, metadata: Metadata) -> Result<Self> {
        let arch = metadata.arch.as_str();
        let version = metadata.version.as_str();
        let staging = Directory::pin(&root, uid)?;
        let mut app = Archive::open(
            root.join(format!("omavless-{version}-1-{arch}.pkg.tar.zst")),
            uid,
            &metadata.app_sha,
            0o600,
        )?;
        let mut dns = Archive::open(
            root.join(format!("omavless-dns-{version}-1-{arch}.pkg.tar.zst")),
            uid,
            &metadata.dns_sha,
            0o600,
        )?;
        package_metadata(&app.member(".PKGINFO")?, "omavless", arch, version)?;
        package_metadata(&dns.member(".PKGINFO")?, "omavless-dns", arch, version)?;
        let identity = app.member("usr/share/doc/omavless/build-identity.txt")?;
        let app_digest = app_identity(&identity, arch, &metadata.source, version)?;
        let receipt = dns.member("usr/share/omavless-dns/source-receipt.json")?;
        if !dns_receipt_matches(&receipt, arch, &metadata.source) {
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
            metadata,
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
            != format!(
                "omavless {}-1\nomavless-dns {}-1\n",
                self.metadata.version, self.metadata.version
            )
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

struct Previous098 {
    members: Vec<Captured>,
    binaries: Vec<Archive>,
    dns_present: bool,
    require_prepared: bool,
}
impl Previous098 {
    fn capture(pair: &Pair, require_prepared: bool) -> Result<Self> {
        let receipt = Captured::read(
            Path::new("/usr/share/doc/omavless/build-identity.txt"),
            0,
            0o644,
            4096,
        )
        .map_err(|_| Blocker::Predecessor)?;
        let text = std::str::from_utf8(&receipt.bytes).map_err(|_| Blocker::Predecessor)?;
        let source = text
            .lines()
            .find_map(|line| line.strip_prefix("sourceCommit="))
            .filter(|source| PUBLIC_098_SOURCES.contains(source) || *source == pair.metadata.source)
            .ok_or(Blocker::Predecessor)?;
        let app_sha = app_identity(&receipt.bytes, std::env::consts::ARCH, source, "0.9.8")?;
        if source == pair.metadata.source && app_sha != pair.app_digest {
            return Err(Blocker::Predecessor);
        }
        let published = if source == pair.metadata.source {
            None
        } else {
            Some(published098(source, std::env::consts::ARCH)?)
        };
        if published
            .as_ref()
            .is_some_and(|pins| pins.app_sha != app_sha)
        {
            return Err(Blocker::Predecessor);
        }
        let app = Archive::open(PathBuf::from("/usr/bin/omavless"), 0, &app_sha, 0o755)?;
        let mut previous = Self {
            members: Vec::new(),
            binaries: vec![app],
            dns_present: false,
            require_prepared,
        };
        let source = source.to_owned();
        previous.members.push(receipt);
        let mut query = Command::new("/usr/bin/pacman");
        query.env("LC_ALL", "C").arg("-Qq");
        let packages =
            crate::production_observation::bounded_fixed_query(query, Duration::from_secs(3))
                .map_err(|_| Blocker::Predecessor)?;
        if packages
            .lines()
            .any(|name| name == "omavless-dns-experimental")
        {
            return Err(Blocker::Predecessor);
        }
        previous.dns_present = packages.lines().any(|name| name == "omavless-dns");
        if previous.dns_present {
            crate::managed_pair::ManagedPair::validate_package()
                .map_err(|_| Blocker::Predecessor)?;
            let dns = Captured::read(
                Path::new(crate::managed_pair::RELEASE_RECEIPT),
                0,
                0o644,
                8192,
            )
            .map_err(|_| Blocker::Predecessor)?;
            let value: serde_json::Value =
                serde_json::from_slice(&dns.bytes).map_err(|_| Blocker::Predecessor)?;
            if value["omavless_commit"] != source {
                return Err(Blocker::Predecessor);
            }
            if let Some(pins) = &published {
                if format!("{:x}", Sha256::digest(&dns.bytes)) != pins.dns_receipt_sha
                    || value["sha256"]["mihomo"] != pins.core_sha
                    || value["sha256"]["omavless-dns-broker"] != pins.broker_sha
                {
                    return Err(Blocker::Predecessor);
                }
            } else if dns.bytes != pair.dns_receipt {
                // Same-target repair must match the authenticated delivery,
                // not a self-declared installed companion hash.
                return Err(Blocker::Predecessor);
            }
            for (name, path) in [
                ("mihomo", crate::managed_pair::RELEASE_CORE),
                ("omavless-dns-broker", crate::managed_pair::RELEASE_BROKER),
            ] {
                let digest = value["sha256"][name].as_str().ok_or(Blocker::Predecessor)?;
                previous
                    .binaries
                    .push(Archive::open(PathBuf::from(path), 0, digest, 0o755)?);
            }
            previous.members.push(dns);
            for (path, expected) in [
                (
                    "/usr/lib/omavless-dns/package-guard",
                    include_bytes!("../../../../tests/dns_broker_host/package/package-guard")
                        .as_slice(),
                ),
                (
                    "/usr/share/libalpm/hooks/omavless-dns.hook",
                    include_bytes!("../../../../packaging/dns/omavless-dns.hook").as_slice(),
                ),
            ] {
                let member = Captured::read(
                    Path::new(path),
                    0,
                    if path.ends_with("package-guard") {
                        0o755
                    } else {
                        0o644
                    },
                    16384,
                )
                .map_err(|_| Blocker::Predecessor)?;
                if member.bytes != expected {
                    return Err(Blocker::Predecessor);
                }
                previous.members.push(member);
            }
        }
        previous.recheck()?;
        Ok(previous)
    }
    fn recheck(&mut self) -> Result<()> {
        package_owner098_before(self.dns_present, self.require_prepared)?;
        for member in &mut self.members {
            member.recheck()?;
        }
        for binary in &mut self.binaries {
            binary.recheck()?;
        }
        Ok(())
    }
}

fn package_owner098_before(dns_present: bool, require_prepared: bool) -> Result<()> {
    let mut query = Command::new("/usr/bin/pacman");
    query.env("LC_ALL", "C").arg("-Qq");
    let inventory =
        crate::production_observation::bounded_fixed_query(query, Duration::from_secs(3))
            .map_err(|_| Blocker::Predecessor)?;
    if inventory
        .lines()
        .any(|name| name == "omavless-dns-experimental")
        || inventory.lines().any(|name| name == "omavless-dns") != dns_present
    {
        return Err(Blocker::Predecessor);
    }
    let mut query = Command::new("/usr/bin/pacman");
    query.env("LC_ALL", "C").args(["-Q", "omavless"]);
    if crate::production_observation::bounded_fixed_query(query, Duration::from_secs(3))
        .map_err(|_| Blocker::Predecessor)?
        != "omavless 0.9.8-1\n"
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
    if !dns_present {
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
        return Ok(());
    }
    let mut query = Command::new("/usr/bin/pacman");
    query.env("LC_ALL", "C").args(["-Q", "omavless-dns"]);
    if crate::production_observation::bounded_fixed_query(query, Duration::from_secs(3))
        .map_err(|_| Blocker::Predecessor)?
        != "omavless-dns 0.9.8-1\n"
    {
        return Err(Blocker::Predecessor);
    }
    let mut ownership = Command::new("/usr/bin/pacman");
    ownership.env("LC_ALL", "C").args([
        "-Qoq",
        crate::managed_pair::RELEASE_RECEIPT,
        crate::managed_pair::RELEASE_CORE,
        crate::managed_pair::RELEASE_BROKER,
        "/usr/lib/omavless-dns/package-guard",
        "/usr/share/libalpm/hooks/omavless-dns.hook",
        "/usr/lib/systemd/system/omavless-dns-broker.service",
    ]);
    if crate::production_observation::bounded_fixed_query(ownership, Duration::from_secs(3))
        .map_err(|_| Blocker::Predecessor)?
        != "omavless-dns\n".repeat(6)
    {
        return Err(Blocker::Predecessor);
    }
    if !require_prepared {
        return Ok(());
    }
    require_broker_prepared()
}
pub(super) fn require_broker_prepared() -> Result<()> {
    let mut query = Command::new("/usr/bin/systemctl");
    query
        .env_clear()
        .env("PATH", "/usr/bin")
        .env("LC_ALL", "C")
        .args([
            "--system",
            "show",
            "omavless-dns-broker.service",
            "--no-pager",
            "--property=LoadState,ActiveState,SubState,MainPID,NFileDescriptorStore",
        ]);
    let state = crate::production_observation::bounded_fixed_query(query, Duration::from_secs(3))
        .map_err(|_| Blocker::BrokerPreparationRequired)?;
    if !broker_prepared_status(&state, absent(Path::new("/run/omavless-dns/control.sock"))) {
        return Err(Blocker::BrokerPreparationRequired);
    }
    Ok(())
}

fn broker_prepared_status(state: &str, socket_absent: bool) -> bool {
    let expected = [
        "LoadState=loaded",
        "ActiveState=inactive",
        "SubState=dead",
        "MainPID=0",
        "NFileDescriptorStore=0",
    ];
    let mut actual = state.lines().collect::<Vec<_>>();
    actual.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    actual == expected && socket_absent
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
    other_owners().map_err(|_| Blocker::PackageOutcomeUnverified)?;
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

fn guest_then<T>(guest: Result<()>, continue_operation: impl FnOnce() -> Result<T>) -> Result<T> {
    guest?;
    continue_operation()
}

#[cfg(feature = "developer-upgrade082")]
pub fn run_development_update() -> Result<serde_json::Value> {
    guest_then(development_guest(), || {
        run_qualified_update(Metadata::developer()?, false, false)
    })
}

#[cfg(feature = "setup-upgrade082")]
pub fn run_delivery_update() -> Result<serde_json::Value> {
    if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return Err(Blocker::DeliveryMetadataInvalid);
    }
    // Must consume the whole bounded frame and EOF before opening the tty or
    // considering consent. The metadata pipe is NEVER child stdin.
    let metadata = Metadata::read(&mut std::io::stdin().lock())?;
    run_qualified_update(metadata, true, false)
}

#[cfg(feature = "setup-upgrade082")]
pub fn run_pair_update() -> Result<serde_json::Value> {
    if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return Err(Blocker::DeliveryMetadataInvalid);
    }
    let metadata = Metadata::read(&mut std::io::stdin().lock())?;
    run_qualified_update(metadata, true, true)
}

fn open_tty() -> Result<File> {
    let tty = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open("/dev/tty")
        .map_err(|_| Blocker::TtyUnavailable)?;
    if !tty
        .metadata()
        .map_err(|_| Blocker::TtyUnavailable)?
        .file_type()
        .is_char_device()
        || !std::io::IsTerminal::is_terminal(&tty)
    {
        return Err(Blocker::TtyUnavailable);
    }
    Ok(tty)
}
fn consent_from_tty(tty: &mut File, expected: &[u8]) -> Result<()> {
    let mut bytes = Vec::new();
    while bytes.len() < 128 {
        let mut byte = [0];
        if tty.read(&mut byte).map_err(|_| Blocker::TtyUnavailable)? == 0 {
            break;
        }
        bytes.push(byte[0]);
        if byte[0] == b'\n' {
            break;
        }
    }
    if bytes != expected {
        return Err(Blocker::CancelledBeforeSubmission);
    }
    Ok(())
}
pub(super) fn running_digest() -> Result<String> {
    // /proc/self/exe is the fixed kernel reference, deliberately not a caller
    // path or an assertion that this helper is already the installed app.
    let mut file = File::open("/proc/self/exe").map_err(|_| Blocker::HelperIdentityUnverified)?;
    let before = file
        .metadata()
        .map_err(|_| Blocker::HelperIdentityUnverified)?;
    if !before.is_file()
        || before.len() == 0
        || before.len() > 512 * 1024 * 1024
        || before.mode() & 0o022 != 0
    {
        return Err(Blocker::HelperIdentityUnverified);
    }
    let mut sha = Sha256::new();
    let mut total = 0;
    let mut buffer = [0; 65536];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| Blocker::HelperIdentityUnverified)?;
        if count == 0 {
            break;
        }
        total += count;
        if total > 512 * 1024 * 1024 {
            return Err(Blocker::HelperIdentityUnverified);
        }
        sha.update(&buffer[..count]);
    }
    if !same(
        &before,
        &file
            .metadata()
            .map_err(|_| Blocker::HelperIdentityUnverified)?,
    ) {
        return Err(Blocker::HelperIdentityUnverified);
    }
    Ok(format!("{:x}", sha.finalize()))
}
fn qualify_helper(expected: &str) -> Result<()> {
    if running_digest()? != expected {
        return Err(Blocker::HelperIdentityUnverified);
    }
    Ok(())
}
fn other_owners() -> Result<()> {
    // Observation is not persistent exclusion and never reads another user's
    // private files or signals their process. The admin consents to replacement.
    let pids = omavless_mihomo::observation::processes_named_strict(Path::new("/proc"), "omavless")
        .map_err(|_| Blocker::HostAdmissionUnavailable)?;
    for pid in pids {
        if pid == std::process::id() {
            continue;
        }
        let uid = fs::metadata(format!("/proc/{pid}"))
            .map_err(|_| Blocker::HostAdmissionUnavailable)?
            .uid();
        if uid != Uid::current().as_raw() {
            return Err(Blocker::ForeignOwnerObserved);
        }
    }
    Ok(())
}

fn run_qualified_update(
    metadata: Metadata,
    delivered: bool,
    managed098: bool,
) -> Result<serde_json::Value> {
    let cancel = Arc::new(AtomicBool::new(false));
    let mut tty = open_tty()?;
    let locale = super::terminal_copy::Locale::current();
    let mut pair = Pair::current(metadata)?;
    if delivered {
        qualify_helper(&pair.app_digest)?;
    }
    if managed098 {
        // Read-only preflight BEFORE consent or volatile exclusion creation.
        // An active runtime must use normal Full Quit, not be stopped here.
        units()?;
        super::terminal_copy::prepare_prompt(&mut tty, locale)
            .map_err(|_| Blocker::TtyUnavailable)?;
    } else {
        super::terminal_copy::upgrade_prompt(&mut tty, locale, &pair.metadata.version, delivered)
            .map_err(|_| Blocker::TtyUnavailable)?;
    }
    tty.flush().map_err(|_| Blocker::TtyUnavailable)?;
    consent_from_tty(
        &mut tty,
        if managed098 {
            b"PREPARE\n"
        } else if delivered {
            b"UPGRADE\n"
        } else {
            b"UPGRADE SINGLE-USER VM\n"
        },
    )?;
    let _signals = Signals::capture(&cancel)?;
    let mut context = if managed098 {
        acquire_current_for(Some(&pair.metadata.source))?
    } else {
        acquire_current()?
    };
    let mut unit_files = predecessor_units()?;
    let mut previous = if managed098 {
        Some(Previous098::capture(&pair, false)?)
    } else {
        package_owner_before()?;
        None
    };
    other_owners()?;
    if !managed098 {
        for name in ["managed-dns-selection", ".managed-dns-selection.candidate"] {
            if !absent(&context.scope.config.join(name)) {
                return Err(Blocker::Predecessor);
            }
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
    other_owners()?;
    if let Some(previous) = &mut previous {
        previous.recheck()?;
    } else {
        package_owner_before()?;
    }
    if managed098 {
        if previous.as_ref().is_some_and(|p| p.dns_present) && require_broker_prepared().is_err() {
            #[cfg(feature = "setup-upgrade082")]
            super::root_stage::prepare(&mut context, &cancel, &tty, &pair.app_digest)?;
            #[cfg(not(feature = "setup-upgrade082"))]
            return Err(Blocker::BrokerPreparationRequired);
        }
        if let Some(previous) = &mut previous {
            previous.require_prepared = true;
            previous.recheck()?;
        }
        context.recheck()?;
        pair.recheck()?;
        super::terminal_copy::prepared_prompt(&mut tty, locale)
            .map_err(|_| Blocker::TtyUnavailable)?;
        tty.flush().map_err(|_| Blocker::TtyUnavailable)?;
        consent_from_tty(&mut tty, b"UPGRADE\n")?;
        context.recheck()?;
        pair.recheck()?;
        other_owners()?;
        for unit in &mut unit_files {
            unit.recheck()?;
        }
        if let Some(previous) = &mut previous {
            previous.recheck()?;
        }
    }
    let mut command = Command::new("/usr/bin/sudo");
    command.stdin(Stdio::from(
        tty.try_clone().map_err(|_| Blocker::TtyUnavailable)?,
    ));
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
    let selected = managed098
        && context
            .captured
            .iter()
            .any(|member| member.path == context.scope.config.join(crate::managed_pair::SELECTOR));
    drop(context);
    Ok(
        serde_json::json!({"schemaVersion":1,"scope":if managed098 {"stopped_098_pair_package_phase"} else if delivered {"first_082_package_phase"} else {"single_user_development_vm"},
        "outcome":if selected {"packages_updated_broker_start_required"} else {"packages_updated_dns_required"},
        "dnsSetupRequired":!selected,"startAllowed":false,
        "cancellationRequested":cancel.load(Ordering::Acquire)}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dns_package_receipt_accepts_exact_new_family_and_retains_old_delivery() {
        for arch in ["x86_64", "aarch64"] {
            let value = crate::managed_traffic_receipt::fixture(arch);
            let raw = serde_json::to_vec(&value).unwrap();
            let source = "a".repeat(40);
            assert!(dns_receipt_matches(&raw, arch, &source));
            assert!(!dns_receipt_matches(&raw, arch, &"b".repeat(40)));
            assert!(!dns_receipt_matches(&raw, "unknown", &source));
            for (field, bad) in [
                ("traffic_ready_abi", serde_json::json!(true)),
                ("traffic_ready_abi", serde_json::json!(2)),
                ("conditional_close_abi", serde_json::json!(1)),
                ("package_flavor", serde_json::json!("release-close")),
            ] {
                let mut changed = value.clone();
                changed[field] = bad;
                assert!(!dns_receipt_matches(
                    &serde_json::to_vec(&changed).unwrap(),
                    arch,
                    &source
                ));
            }
            let old = serde_json::json!({"schema":1,"architecture":arch,
                "omavless_commit":source,"package_flavor":"release","broker_feature":"release-package"});
            assert!(dns_receipt_matches(
                &serde_json::to_vec(&old).unwrap(),
                arch,
                &source
            ));
            let mut spoof = old;
            spoof["schema"] = serde_json::json!(crate::managed_traffic_receipt::SCHEMA);
            assert!(!dns_receipt_matches(
                &serde_json::to_vec(&spoof).unwrap(),
                arch,
                &source
            ));
        }
    }
    #[test]
    fn maintenance_admission_never_equates_idle_running_or_residual_socket_with_prepared() {
        let good = "LoadState=loaded\nActiveState=inactive\nSubState=dead\nMainPID=0\nNFileDescriptorStore=0\n";
        assert!(broker_prepared_status(good, true));
        assert!(!broker_prepared_status(good, false));
        for bad in [
            good.replace("inactive", "active")
                .replace("dead", "running"),
            good.replace("MainPID=0", "MainPID=123"),
            good.replace("NFileDescriptorStore=0", "NFileDescriptorStore=1"),
            good.replace("LoadState=loaded", "LoadState=not-found"),
            format!("{good}NFileDescriptorStore=0\n"),
            good.replace("SubState=dead\n", ""),
        ] {
            assert!(!broker_prepared_status(&bad, true));
        }
    }
    #[test]
    fn running_helper_digest_is_separate_from_installed_predecessor_and_mismatch_refuses() {
        let digest = running_digest().unwrap();
        assert_eq!(qualify_helper(&digest), Ok(()));
        let wrong = if digest == "a".repeat(64) {
            "b".repeat(64)
        } else {
            "a".repeat(64)
        };
        assert_eq!(
            qualify_helper(&wrong),
            Err(Blocker::HelperIdentityUnverified)
        );
    }
    #[test]
    fn app_receipt_requires_the_closed_source_arch_and_version_tuple() {
        let identity = format!(
            "schemaVersion=3\nsourceCommit={}\nbinarySha256={}\narchitecture={}\nprovenance=caller-supplied-prebuilt\nproductVersion={}\n",
            SOURCE,
            "a".repeat(64),
            std::env::consts::ARCH,
            VERSION
        );
        assert!(app_identity(identity.as_bytes(), std::env::consts::ARCH, SOURCE, VERSION).is_ok());
        for (arch, source, version) in [
            ("unknown", SOURCE, VERSION),
            (
                std::env::consts::ARCH,
                "cccccccccccccccccccccccccccccccccccccccc",
                VERSION,
            ),
            (std::env::consts::ARCH, SOURCE, "9.9.9"),
        ] {
            assert_eq!(
                app_identity(identity.as_bytes(), arch, source, version),
                Err(Blocker::StagingUnavailable)
            );
        }
    }
    #[test]
    fn fixed_guest_guard_refuses_physical_or_other_virtual_targets_before_continuation() {
        let expected = b"Standard PC (Q35 + ICH9, 2009)\n";
        assert!(guest_identity(b"QEMU\n", expected));
        for (vendor, product) in [
            (b"ASUSTeK COMPUTER INC.\n".as_slice(), expected.as_slice()),
            (
                b"QEMU\n".as_slice(),
                b"Standard PC (i440FX + PIIX, 1996)\n".as_slice(),
            ),
            (b"QEMU\n".as_slice(), b"synthetic mismatch\n".as_slice()),
            (b"QEMU\nextra\n".as_slice(), expected.as_slice()),
        ] {
            let mut continued = false;
            let guard = if guest_identity(vendor, product) {
                Ok(())
            } else {
                Err(Blocker::UnsupportedDevelopmentGuest)
            };
            let result = guest_then(guard, || {
                continued = true;
                Ok(())
            });
            assert_eq!(result, Err(Blocker::UnsupportedDevelopmentGuest));
            assert!(!continued);
        }
    }
    #[test]
    fn pinned_metadata_is_closed_and_duplicate_refuses() {
        let good = b"pkgname = omavless\npkgver = 0.9.8-1\narch = x86_64\n";
        assert_eq!(
            package_metadata(good, "omavless", "x86_64", VERSION),
            Ok(())
        );
        assert!(package_metadata(good, "omavless-dns", "x86_64", VERSION).is_err());
        assert!(
            package_metadata(
                &[good.as_slice(), b"pkgname = omavless\n"].concat(),
                "omavless",
                "x86_64",
                VERSION
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
                // Developer fixed-pair controls retain the immutable public
                // fix.3 record; mutable successor frontend pins are separate.
                .join("../../tests/fixtures/setup-upgrade082-public")
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
        let mut pair = Pair::at(
            root.clone(),
            Uid::current().as_raw(),
            Metadata::developer().unwrap(),
        )
        .unwrap();
        pair.recheck().unwrap();
        assert_eq!(
            app_identity(&pair.app_identity, arch, SOURCE, VERSION).unwrap(),
            pair.app_digest
        );
        drop(pair);
        fs::remove_dir_all(root).unwrap();
    }
}
