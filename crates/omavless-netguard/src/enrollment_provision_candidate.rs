//! Inactive first-enrollment producer. It neither grants group access nor
//! starts a service, modifies kernel policy, or reports K1 as ready.

use crate::enrollment::EnrollmentBinding;
use nix::errno::Errno;
use nix::fcntl::{OFlag, open, openat};
use nix::sys::stat::{Mode, mkdirat};
use std::fs::{File, Metadata};
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;

const DIRECTORY: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_DIRECTORY)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_CLOEXEC);
const READ: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_NONBLOCK)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_CLOEXEC);
const CREATE: OFlag = OFlag::O_WRONLY
    .union(OFlag::O_CREAT)
    .union(OFlag::O_EXCL)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_CLOEXEC);
const MAX_PASSWD: u64 = 1_048_576;
const DIR: &str = "omavless-netguard";
const FILE: &str = "enrollment-v1.json";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProvisionError {
    AccountUnavailable,
    UnsafeOrExisting,
    Ambiguous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CreatedEnrollment;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Checkpoint {
    DirectoryCreated,
    FileCreated,
    FileSynced,
    DirectorySynced,
}

fn safe_directory(file: &File, owner: (u32, u32)) -> Result<(), ProvisionError> {
    let m = file
        .metadata()
        .map_err(|_| ProvisionError::UnsafeOrExisting)?;
    if !m.is_dir() || (m.uid(), m.gid()) != owner || m.mode() & 0o022 != 0 {
        return Err(ProvisionError::UnsafeOrExisting);
    }
    Ok(())
}

fn mkdir_error(error: Errno) -> ProvisionError {
    if error == Errno::EEXIST {
        ProvisionError::UnsafeOrExisting
    } else {
        // Once attempted, an I/O or quota failure does not prove that the
        // directory was not created. Leave that state for explicit recovery.
        ProvisionError::Ambiguous
    }
}

fn same(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.len() == b.len()
        && a.mode() == b.mode()
        && a.uid() == b.uid()
        && a.gid() == b.gid()
        && a.nlink() == b.nlink()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}

fn same_directory(a: &Metadata, b: &Metadata) -> bool {
    a.is_dir()
        && b.is_dir()
        && a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.mode() == b.mode()
        && a.uid() == b.uid()
        && a.gid() == b.gid()
}

fn local_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    (1..=32).contains(&bytes.len())
        && (bytes[0].is_ascii_lowercase() || bytes[0] == b'_')
        && bytes[1..]
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-'))
}

fn canonical_number(value: &str) -> Option<u32> {
    if value.is_empty()
        || value.len() > 10
        || (value.len() > 1 && value.starts_with('0'))
        || !value.as_bytes().iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    value.parse().ok()
}

/// Only an existing, uniquely named local login account is eligible. The
/// result is a numeric UID; no GECOS, home or shell is persisted.
fn account_uid(bytes: &[u8], requested: &str) -> Result<u32, ProvisionError> {
    if !local_name(requested)
        || bytes.is_empty()
        || bytes.len() > MAX_PASSWD as usize
        || bytes.last() != Some(&b'\n')
    {
        return Err(ProvisionError::AccountUnavailable);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| ProvisionError::AccountUnavailable)?;
    let mut accounts = Vec::new();
    for line in text.lines() {
        let fields = line.split(':').collect::<Vec<_>>();
        if fields.len() != 7 || !local_name(fields[0]) {
            return Err(ProvisionError::AccountUnavailable);
        }
        let uid = canonical_number(fields[2]).ok_or(ProvisionError::AccountUnavailable)?;
        let gid = canonical_number(fields[3]).ok_or(ProvisionError::AccountUnavailable)?;
        if !fields[5].starts_with('/') || !fields[6].starts_with('/') {
            return Err(ProvisionError::AccountUnavailable);
        }
        accounts.push((fields[0], uid, gid, fields[6]));
    }
    let matches = accounts
        .iter()
        .filter(|(name, _, _, _)| *name == requested)
        .collect::<Vec<_>>();
    let [(name, uid, gid, shell)] = matches.as_slice() else {
        return Err(ProvisionError::AccountUnavailable);
    };
    if *uid == 0
        || *gid == 0
        || matches!(
            *shell,
            "/bin/false" | "/usr/bin/false" | "/usr/sbin/nologin" | "/usr/bin/nologin"
        )
        || accounts
            .iter()
            .any(|(other, number, _, _)| other != name && number == uid)
    {
        return Err(ProvisionError::AccountUnavailable);
    }
    Ok(*uid)
}

struct LocalAccountSource {
    etc: File,
    original: File,
    owner: (u32, u32),
    name: String,
    uid: u32,
    bytes: Vec<u8>,
    metadata: Metadata,
}

fn read_passwd(etc: &File, owner: (u32, u32)) -> Result<(File, Metadata, Vec<u8>), ProvisionError> {
    let mut file = File::from(
        openat(etc, "passwd", READ, Mode::empty())
            .map_err(|_| ProvisionError::AccountUnavailable)?,
    );
    let before = file
        .metadata()
        .map_err(|_| ProvisionError::AccountUnavailable)?;
    if !before.is_file()
        || (before.uid(), before.gid()) != owner
        || before.nlink() != 1
        || before.mode() & 0o022 != 0
        || before.len() == 0
        || before.len() > MAX_PASSWD
    {
        return Err(ProvisionError::AccountUnavailable);
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_PASSWD + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ProvisionError::AccountUnavailable)?;
    let reopened = File::from(
        openat(etc, "passwd", READ, Mode::empty())
            .map_err(|_| ProvisionError::AccountUnavailable)?,
    );
    if !same(
        &before,
        &file
            .metadata()
            .map_err(|_| ProvisionError::AccountUnavailable)?,
    ) || !same(
        &before,
        &reopened
            .metadata()
            .map_err(|_| ProvisionError::AccountUnavailable)?,
    ) || bytes.len() as u64 != before.len()
    {
        return Err(ProvisionError::AccountUnavailable);
    }
    Ok((file, before, bytes))
}

impl LocalAccountSource {
    fn open(etc: File, owner: (u32, u32), name: &str) -> Result<Self, ProvisionError> {
        safe_directory(&etc, owner)?;
        let (original, metadata, bytes) = read_passwd(&etc, owner)?;
        let uid = account_uid(&bytes, name)?;
        Ok(Self {
            etc,
            original,
            owner,
            name: name.to_owned(),
            uid,
            bytes,
            metadata,
        })
    }

    fn validate(&self) -> Result<(), ProvisionError> {
        safe_directory(&self.etc, self.owner).map_err(|_| ProvisionError::Ambiguous)?;
        let (file, metadata, bytes) =
            read_passwd(&self.etc, self.owner).map_err(|_| ProvisionError::Ambiguous)?;
        if !same(&self.metadata, &metadata)
            || !same(
                &self.metadata,
                &self
                    .original
                    .metadata()
                    .map_err(|_| ProvisionError::Ambiguous)?,
            )
            || !same(
                &self.metadata,
                &file.metadata().map_err(|_| ProvisionError::Ambiguous)?,
            )
            || bytes != self.bytes
            || account_uid(&bytes, &self.name) != Ok(self.uid)
        {
            return Err(ProvisionError::Ambiguous);
        }
        Ok(())
    }
}

fn no_root_state(varlib: &File, owner: (u32, u32)) -> Result<(), ProvisionError> {
    safe_directory(varlib, owner)?;
    match openat(varlib, DIR, DIRECTORY, Mode::empty()) {
        Err(Errno::ENOENT) => Ok(()),
        _ => Err(ProvisionError::UnsafeOrExisting),
    }
}

/// No caller currently invokes this root-path operation. It is not a helper
/// installer, group provisioner or status check.
#[allow(dead_code)]
pub(crate) fn provision_fixed(name: &str) -> Result<CreatedEnrollment, ProvisionError> {
    let root = File::from(
        open("/", DIRECTORY, Mode::empty()).map_err(|_| ProvisionError::UnsafeOrExisting)?,
    );
    safe_directory(&root, (0, 0))?;
    let etc = File::from(
        openat(&root, "etc", DIRECTORY, Mode::empty())
            .map_err(|_| ProvisionError::UnsafeOrExisting)?,
    );
    let var = File::from(
        openat(&root, "var", DIRECTORY, Mode::empty())
            .map_err(|_| ProvisionError::UnsafeOrExisting)?,
    );
    safe_directory(&var, (0, 0))?;
    let varlib = File::from(
        openat(&var, "lib", DIRECTORY, Mode::empty())
            .map_err(|_| ProvisionError::UnsafeOrExisting)?,
    );
    provision_under(etc, varlib, (0, 0), name, |_| true)
}

fn provision_under(
    etc: File,
    varlib: File,
    owner: (u32, u32),
    name: &str,
    mut hook: impl FnMut(Checkpoint) -> bool,
) -> Result<CreatedEnrollment, ProvisionError> {
    let account = LocalAccountSource::open(etc, owner, name)?;
    no_root_state(&varlib, owner)?;
    account.validate()?;
    mkdirat(&account.etc, DIR, Mode::S_IRWXU).map_err(mkdir_error)?;
    if !hook(Checkpoint::DirectoryCreated) {
        return Err(ProvisionError::Ambiguous);
    }
    let dir = File::from(
        openat(&account.etc, DIR, DIRECTORY, Mode::empty())
            .map_err(|_| ProvisionError::Ambiguous)?,
    );
    let identity = dir.metadata().map_err(|_| ProvisionError::Ambiguous)?;
    if !identity.is_dir()
        || (identity.uid(), identity.gid()) != owner
        || identity.mode() & 0o7777 != 0o700
        || account.validate().is_err()
        || no_root_state(&varlib, owner).is_err()
    {
        return Err(ProvisionError::Ambiguous);
    }
    let mut file = File::from(
        openat(&dir, FILE, CREATE, Mode::S_IRUSR | Mode::S_IWUSR)
            .map_err(|_| ProvisionError::Ambiguous)?,
    );
    if !hook(Checkpoint::FileCreated) {
        return Err(ProvisionError::Ambiguous);
    }
    let bytes = format!("{{\"version\":1,\"enrolled_uid\":{}}}", account.uid);
    file.write_all(bytes.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|_| ProvisionError::Ambiguous)?;
    let written = file.metadata().map_err(|_| ProvisionError::Ambiguous)?;
    if !written.is_file()
        || (written.uid(), written.gid()) != owner
        || written.mode() & 0o7777 != 0o600
        || written.nlink() != 1
        || written.len() != bytes.len() as u64
    {
        return Err(ProvisionError::Ambiguous);
    }
    if !hook(Checkpoint::FileSynced) {
        return Err(ProvisionError::Ambiguous);
    }
    dir.sync_all().map_err(|_| ProvisionError::Ambiguous)?;
    account
        .etc
        .sync_all()
        .map_err(|_| ProvisionError::Ambiguous)?;
    if !hook(Checkpoint::DirectorySynced) {
        return Err(ProvisionError::Ambiguous);
    }
    account.validate()?;
    no_root_state(&varlib, owner).map_err(|_| ProvisionError::Ambiguous)?;
    let current_dir = File::from(
        openat(&account.etc, DIR, DIRECTORY, Mode::empty())
            .map_err(|_| ProvisionError::Ambiguous)?,
    );
    if !same_directory(
        &identity,
        &current_dir
            .metadata()
            .map_err(|_| ProvisionError::Ambiguous)?,
    ) {
        return Err(ProvisionError::Ambiguous);
    }
    let current_file =
        File::from(openat(&dir, FILE, READ, Mode::empty()).map_err(|_| ProvisionError::Ambiguous)?);
    if !same(
        &written,
        &file.metadata().map_err(|_| ProvisionError::Ambiguous)?,
    ) || !same(
        &written,
        &current_file
            .metadata()
            .map_err(|_| ProvisionError::Ambiguous)?,
    ) {
        return Err(ProvisionError::Ambiguous);
    }
    let binding = EnrollmentBinding::open_under(
        account
            .etc
            .try_clone()
            .map_err(|_| ProvisionError::Ambiguous)?,
        owner,
    )
    .map_err(|_| ProvisionError::Ambiguous)?;
    if binding.uid() != account.uid || binding.validate().is_err() {
        return Err(ProvisionError::Ambiguous);
    }
    Ok(CreatedEnrollment)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    const PASSWD: &[u8] =
        b"root:x:0:0:root:/root:/bin/bash\nkdk_vm:x:1001:1001:Test User:/home/kdk_vm:/bin/zsh\n";

    struct Fixture {
        root: PathBuf,
        etc: File,
        varlib: File,
        owner: (u32, u32),
    }

    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let home = std::env::var_os("HOME").expect("test needs home");
            let root = Path::new(&home).join(format!(
                "omavless-netguard-provision-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
            fs::DirBuilder::new()
                .mode(0o700)
                .create(root.join("etc"))
                .unwrap();
            fs::create_dir(root.join("var")).unwrap();
            fs::DirBuilder::new()
                .mode(0o700)
                .create(root.join("var/lib"))
                .unwrap();
            fs::write(root.join("etc/passwd"), PASSWD).unwrap();
            fs::set_permissions(root.join("etc/passwd"), fs::Permissions::from_mode(0o644))
                .unwrap();
            let etc = File::open(root.join("etc")).unwrap();
            let varlib = File::open(root.join("var/lib")).unwrap();
            let meta = etc.metadata().unwrap();
            Self {
                root,
                etc,
                varlib,
                owner: (meta.uid(), meta.gid()),
            }
        }

        fn run(
            &self,
            hook: impl FnMut(Checkpoint) -> bool,
        ) -> Result<CreatedEnrollment, ProvisionError> {
            provision_under(
                self.etc.try_clone().unwrap(),
                self.varlib.try_clone().unwrap(),
                self.owner,
                "kdk_vm",
                hook,
            )
        }

        fn enrollment(&self) -> PathBuf {
            self.root.join("etc/omavless-netguard/enrollment-v1.json")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[test]
    fn only_unique_non_root_local_login_account_is_accepted() {
        assert_eq!(account_uid(PASSWD, "kdk_vm"), Ok(1001));
        for (bytes, name) in [
            (PASSWD, "root"),
            (PASSWD, "absent"),
            (PASSWD, "../bad"),
            (b"kdk_vm:x:0:1001:Test:/home/kdk_vm:/bin/zsh\n", "kdk_vm"),
            (b"kdk_vm:x:1001:0:Test:/home/kdk_vm:/bin/zsh\n", "kdk_vm"),
            (b"kdk_vm:x:1001:1001:Test:/home/kdk_vm:/usr/bin/nologin\n", "kdk_vm"),
            (b"kdk_vm:x:1001:1001:Test:/home/kdk_vm:/bin/zsh", "kdk_vm"),
            (b"kdk_vm:x:1001:1001:Test:/home/kdk_vm:/bin/zsh\nother:x:1001:1002:Test:/home/other:/bin/zsh\n", "kdk_vm"),
            (b"kdk_vm:x:1001:1001:Test:/home/kdk_vm:/bin/zsh\nkdk_vm:x:1002:1002:Test:/home/kdk_vm:/bin/zsh\n", "kdk_vm"),
            (b"kdk_vm:x:01001:1001:Test:/home/kdk_vm:/bin/zsh\n", "kdk_vm"),
        ] {
            assert_eq!(account_uid(bytes, name), Err(ProvisionError::AccountUnavailable));
        }
    }

    #[test]
    fn uncertain_directory_creation_failure_requires_recovery() {
        assert_eq!(mkdir_error(Errno::EEXIST), ProvisionError::UnsafeOrExisting);
        assert_eq!(mkdir_error(Errno::EIO), ProvisionError::Ambiguous);
        assert_eq!(mkdir_error(Errno::ENOSPC), ProvisionError::Ambiguous);
    }

    #[test]
    fn first_enrollment_is_canonical_exclusive_and_not_ready_claim() {
        let f = Fixture::new();
        assert_eq!(f.run(|_| true), Ok(CreatedEnrollment));
        assert_eq!(
            fs::read(f.enrollment()).unwrap(),
            b"{\"version\":1,\"enrolled_uid\":1001}"
        );
        let binding = EnrollmentBinding::open_under(f.etc.try_clone().unwrap(), f.owner).unwrap();
        assert_eq!(binding.uid(), 1001);
        binding.validate().unwrap();
        assert_eq!(f.run(|_| true), Err(ProvisionError::UnsafeOrExisting));
        assert_eq!(
            fs::read(f.enrollment()).unwrap(),
            b"{\"version\":1,\"enrolled_uid\":1001}"
        );
    }

    #[test]
    fn existing_state_unsafe_source_and_existing_enrollment_refuse_without_write() {
        for variant in 0..5 {
            let f = Fixture::new();
            match variant {
                0 => fs::create_dir(f.root.join("var/lib/omavless-netguard")).unwrap(),
                1 => symlink("elsewhere", f.root.join("var/lib/omavless-netguard")).unwrap(),
                2 => {
                    fs::set_permissions(
                        f.root.join("etc/passwd"),
                        fs::Permissions::from_mode(0o666),
                    )
                    .unwrap();
                }
                3 => {
                    fs::remove_file(f.root.join("etc/passwd")).unwrap();
                    symlink("/etc/passwd", f.root.join("etc/passwd")).unwrap();
                }
                _ => {
                    fs::create_dir(f.root.join("etc/omavless-netguard")).unwrap();
                    fs::write(f.enrollment(), b"existing").unwrap();
                }
            }
            assert!(f.run(|_| true).is_err());
            if variant < 4 {
                assert!(!f.root.join("etc/omavless-netguard").exists());
            } else {
                assert_eq!(fs::read(f.enrollment()).unwrap(), b"existing");
            }
        }
    }

    #[test]
    fn every_interrupted_publication_leaves_evidence_and_never_reports_created() {
        for stop in [
            Checkpoint::DirectoryCreated,
            Checkpoint::FileCreated,
            Checkpoint::FileSynced,
            Checkpoint::DirectorySynced,
        ] {
            let f = Fixture::new();
            assert_eq!(f.run(|point| point != stop), Err(ProvisionError::Ambiguous));
            assert!(f.root.join("etc/omavless-netguard").exists());
            assert_eq!(f.run(|_| true), Err(ProvisionError::UnsafeOrExisting));
        }
    }

    #[test]
    fn account_or_directory_rebinding_cannot_report_success() {
        let f = Fixture::new();
        let original = f.root.join("etc/passwd");
        assert_eq!(
            f.run(|point| {
                if point == Checkpoint::DirectoryCreated {
                    fs::rename(&original, f.root.join("etc/old-passwd")).unwrap();
                    fs::write(
                        &original,
                        b"kdk_vm:x:2002:2002:Test:/home/kdk_vm:/bin/zsh\n",
                    )
                    .unwrap();
                }
                true
            }),
            Err(ProvisionError::Ambiguous)
        );
        assert!(!f.enrollment().exists());

        let f = Fixture::new();
        assert_eq!(
            f.run(|point| {
                if point == Checkpoint::FileSynced {
                    let directory = f.root.join("etc/omavless-netguard");
                    fs::rename(&directory, f.root.join("etc/old-enrollment")).unwrap();
                    fs::DirBuilder::new()
                        .mode(0o700)
                        .create(&directory)
                        .unwrap();
                    fs::write(
                        directory.join(FILE),
                        b"{\"version\":1,\"enrolled_uid\":1001}",
                    )
                    .unwrap();
                }
                true
            }),
            Err(ProvisionError::Ambiguous)
        );
        assert!(f.enrollment().exists());

        let f = Fixture::new();
        assert_eq!(
            f.run(|point| {
                if point == Checkpoint::FileSynced {
                    let enrolled = f.enrollment();
                    let bytes = fs::read(&enrolled).unwrap();
                    fs::rename(&enrolled, f.root.join("etc/omavless-netguard/old-record")).unwrap();
                    fs::write(&enrolled, bytes).unwrap();
                    fs::set_permissions(&enrolled, fs::Permissions::from_mode(0o600)).unwrap();
                }
                true
            }),
            Err(ProvisionError::Ambiguous)
        );
        assert!(f.enrollment().exists());
    }

    #[test]
    fn hardlinked_passwd_and_existing_hardlinked_enrollment_refuse() {
        let f = Fixture::new();
        fs::hard_link(f.root.join("etc/passwd"), f.root.join("etc/passwd-alias")).unwrap();
        assert_eq!(f.run(|_| true), Err(ProvisionError::AccountUnavailable));
        assert!(!f.enrollment().exists());

        let f = Fixture::new();
        fs::create_dir(f.root.join("etc/omavless-netguard")).unwrap();
        fs::write(f.root.join("existing"), b"existing").unwrap();
        fs::hard_link(f.root.join("existing"), f.enrollment()).unwrap();
        let before = fs::read(f.enrollment()).unwrap();
        assert_eq!(f.run(|_| true), Err(ProvisionError::UnsafeOrExisting));
        assert_eq!(fs::read(f.enrollment()).unwrap(), before);
    }
}
