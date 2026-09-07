// SPDX-License-Identifier: MIT

//! Explicit client-side desktop adapters. Never registered on the daemon socket.
//! Successful data is private; errors contain only fixed public vocabulary.

use nix::fcntl::{FcntlArg, OFlag, fcntl};
use nix::unistd::Uid;
use std::env;
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

pub const MAX_TEXT_BYTES: usize = 64 * 1024;
pub const MAX_PATH_BYTES: usize = 4096;
const MAX_PNG_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    MissingClipboard,
    MissingPicker,
    MissingEditor,
    MissingQr,
    Cancelled,
    TimedOut,
    InvalidInput,
    TooLarge,
    Unavailable,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MissingClipboard => "Clipboard unavailable: install wl-clipboard",
            Self::MissingPicker => "File picker unavailable: install zenity, kdialog or yad",
            Self::MissingEditor => "Profile editor unavailable: install zenity",
            Self::MissingQr => "QR encoder unavailable: install qrencode",
            Self::Cancelled => "Desktop action cancelled",
            Self::TimedOut => "Desktop helper timed out",
            Self::InvalidInput => "Desktop helper input is invalid",
            Self::TooLarge => "Desktop helper input or output exceeds its size limit",
            Self::Unavailable => "Desktop helper could not complete the action",
        })
    }
}

impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;

/// Helper names and preference are fixed; relative/empty PATH entries are ignored.
pub struct DesktopHelpers {
    search: Vec<PathBuf>,
}

impl DesktopHelpers {
    pub fn current() -> Self {
        Self::from_path(env::var_os("PATH").as_deref().unwrap_or_default())
    }

    pub fn from_path(path: &std::ffi::OsStr) -> Self {
        Self {
            search: env::split_paths(path).filter(|p| p.is_absolute()).collect(),
        }
    }

    fn find(&self, name: &str) -> Option<PathBuf> {
        self.search.iter().map(|p| p.join(name)).find(|p| {
            fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
    }

    fn picker(&self) -> Option<(&'static str, PathBuf)> {
        ["zenity", "kdialog", "yad"]
            .into_iter()
            .find_map(|name| self.find(name).map(|p| (name, p)))
    }

    pub fn capabilities(&self) -> serde_json::Value {
        serde_json::json!({
            "schemaVersion": 1,
            "clipboardReadAvailable": self.find("wl-paste").is_some(),
            "clipboardWriteAvailable": self.find("wl-copy").is_some(),
            "filePicker": self.picker().map(|(name, _)| name),
            "configEditorAvailable": self.find("zenity").is_some(),
            "qrEncoderAvailable": self.find("qrencode").is_some(),
            "gtk4FallbackAvailable": false,
        })
    }

    pub fn clipboard_read(&self) -> Result<Vec<u8>> {
        let tool = self.find("wl-paste").ok_or(Error::MissingClipboard)?;
        let started = Instant::now();
        let types = match execute(
            &tool,
            &["--list-types".into()],
            &[],
            8192,
            Some(Duration::from_secs(5)),
            false,
        ) {
            Ok(types) => types,
            Err(Error::Unavailable) => Vec::new(),
            Err(error) => return Err(error),
        };
        let mut args = vec!["--no-newline".into()];
        if types
            .split(|b| *b == b'\n')
            .any(|line| line == b"text/plain")
        {
            args.extend(["--type".into(), "text/plain".into()]);
        }
        let budget = Duration::from_secs(5)
            .checked_sub(started.elapsed())
            .ok_or(Error::TimedOut)?;
        let data = execute(&tool, &args, &[], MAX_TEXT_BYTES, Some(budget), false)?;
        text(&data)?;
        Ok(data)
    }

    pub fn clipboard_copy(&self, data: &[u8]) -> Result<()> {
        text(data)?;
        let tool = self.find("wl-copy").ok_or(Error::MissingClipboard)?;
        execute(
            &tool,
            &["--type".into(), "text/plain".into()],
            data,
            0,
            Some(Duration::from_secs(5)),
            false,
        )?;
        Ok(())
    }

    /// The selected file's contents, not its filename, feed the existing classifier.
    /// Dialogs intentionally have no artificial human-response deadline.
    pub fn pick_import(&self) -> Result<Vec<u8>> {
        let (provider, tool) = self.picker().ok_or(Error::MissingPicker)?;
        let args: Vec<OsString> = match provider {
            "zenity" => [
                "--file-selection",
                "--title=Import profile link",
                "--file-filter=Profile link | *.txt *.url *.conf",
                "--file-filter=All files | *",
            ]
            .map(Into::into)
            .to_vec(),
            "kdialog" => ["--getopenfilename", "/", "*.txt *.url *.conf|Profile link"]
                .map(Into::into)
                .to_vec(),
            _ => ["--file", "--title=Import profile link"]
                .map(Into::into)
                .to_vec(),
        };
        let selected = execute(&tool, &args, &[], MAX_PATH_BYTES + 2, None, true)?;
        let selected = std::str::from_utf8(&selected)
            .map_err(|_| Error::InvalidInput)?
            .trim_end_matches(['\r', '\n']);
        if selected.is_empty() {
            return Err(Error::Cancelled);
        }
        read_import_file(selected.as_bytes())
    }

    /// The caller supplies the explicit editor seed; no store access or confirmation mutation.
    pub fn edit(&self, seed: &[u8], runtime: &Path) -> Result<Vec<u8>> {
        text(seed)?;
        validate_private_runtime(runtime)?;
        let tool = self.find("zenity").ok_or(Error::MissingEditor)?;
        let mut temporary = PrivateTemporary::new(runtime)?;
        temporary
            .file
            .write_all(seed)
            .map_err(|_| Error::Unavailable)?;
        let mut filename = OsString::from("--filename=");
        filename.push(&temporary.path);
        let output = execute(
            &tool,
            &[
                "--text-info".into(),
                "--editable".into(),
                filename,
                "--title=Edit profile".into(),
                "--width=700".into(),
                "--height=300".into(),
            ],
            &[],
            MAX_TEXT_BYTES,
            None,
            true,
        )?;
        text(&output)?;
        Ok(output)
    }

    /// Binary PNG goes only to the explicit private caller, never a public result envelope.
    pub fn qr_png(&self, data: &[u8]) -> Result<Vec<u8>> {
        text(data)?;
        let tool = self.find("qrencode").ok_or(Error::MissingQr)?;
        let output = execute(
            &tool,
            &[
                "-o".into(),
                "-".into(),
                "-s".into(),
                "8".into(),
                "-m".into(),
                "2".into(),
            ],
            data,
            MAX_PNG_BYTES,
            Some(Duration::from_secs(10)),
            false,
        )?;
        if !output.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err(Error::Unavailable);
        }
        Ok(output)
    }
}

fn text(data: &[u8]) -> Result<&str> {
    if data.len() > MAX_TEXT_BYTES {
        return Err(Error::TooLarge);
    }
    let value = std::str::from_utf8(data).map_err(|_| Error::InvalidInput)?;
    if value.contains('\0') {
        return Err(Error::InvalidInput);
    }
    Ok(value)
}

fn selected_path(data: &[u8]) -> Result<PathBuf> {
    if data.len() > MAX_PATH_BYTES {
        return Err(Error::TooLarge);
    }
    let value = std::str::from_utf8(data).map_err(|_| Error::InvalidInput)?;
    let path = PathBuf::from(value);
    if !path.is_absolute()
        || value.bytes().any(|b| b < 32 || b == 127)
        || path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(Error::InvalidInput);
    }
    Ok(path)
}

pub fn read_import_file(path: &[u8]) -> Result<Vec<u8>> {
    let path = selected_path(path)?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags((OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK).bits())
        .open(path)
        .map_err(|_| Error::Unavailable)?;
    if !file.metadata().is_ok_and(|m| m.is_file()) {
        return Err(Error::Unavailable);
    }
    let mut data = Vec::new();
    file.take((MAX_TEXT_BYTES + 1) as u64)
        .read_to_end(&mut data)
        .map_err(|_| Error::Unavailable)?;
    text(&data)?;
    Ok(data)
}

/// Explicit same-user export, atomic mode 0600. Caller must obtain confirmation first.
pub fn export_file(path: &[u8], data: &[u8]) -> Result<()> {
    text(data)?;
    let path = selected_path(path)?;
    let parent = path.parent().ok_or(Error::InvalidInput)?;
    if let Ok(metadata) = fs::symlink_metadata(&path)
        && (!metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.uid() != Uid::current().as_raw())
    {
        return Err(Error::Unavailable);
    }
    let mut temporary = PrivateTemporary::new(parent)?;
    temporary
        .file
        .write_all(data)
        .map_err(|_| Error::Unavailable)?;
    temporary.file.sync_all().map_err(|_| Error::Unavailable)?;
    fs::rename(&temporary.path, &path).map_err(|_| Error::Unavailable)?;
    File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(|_| Error::Unavailable)
}

struct PrivateTemporary {
    path: PathBuf,
    file: File,
}

fn validate_private_runtime(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| Error::Unavailable)?;
    if !path.is_absolute()
        || !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != Uid::current().as_raw()
        || metadata.permissions().mode() & 0o777 != 0o700
    {
        return Err(Error::Unavailable);
    }
    Ok(())
}

/// Client scratch storage is independent of the daemon's runtime directory.
/// Existing unsafe state is rejected, never chmodded, followed or replaced.
pub fn current_desktop_runtime() -> Result<PathBuf> {
    let base = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", Uid::current().as_raw())));
    prepare_desktop_runtime(&base)
}

fn prepare_desktop_runtime(base: &Path) -> Result<PathBuf> {
    validate_private_runtime(base)?;
    let path = base.join("omavless-desktop");
    match fs::DirBuilder::new().mode(0o700).create(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(Error::Unavailable),
    }
    validate_private_runtime(&path)?;
    Ok(path)
}

/// Reap only this helper's dead-process private editor seeds in the fixed runtime
/// directory. Active editors and symlinks are never removed.
pub fn cleanup(runtime: &Path) -> Result<usize> {
    validate_private_runtime(runtime)?;
    let entries = fs::read_dir(runtime).map_err(|_| Error::Unavailable)?;
    let mut removed = 0;
    for entry in entries.take(4096) {
        let entry = entry.map_err(|_| Error::Unavailable)?;
        let name = entry.file_name();
        let Some(name) = name
            .to_str()
            .and_then(|s| s.strip_prefix(".omavless-desktop-"))
        else {
            continue;
        };
        let Some((pid, counter)) = name.split_once('-') else {
            continue;
        };
        let Ok(pid) = pid.parse::<u32>() else {
            continue;
        };
        if pid == 0
            || pid == std::process::id()
            || counter.parse::<u64>().is_err()
            || Path::new("/proc").join(pid.to_string()).exists()
        {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path()).map_err(|_| Error::Unavailable)?;
        if metadata.is_file()
            && !metadata.file_type().is_symlink()
            && metadata.uid() == Uid::current().as_raw()
            && metadata.permissions().mode() & 0o777 == 0o600
        {
            fs::remove_file(entry.path()).map_err(|_| Error::Unavailable)?;
            removed += 1;
        }
    }
    Ok(removed)
}
impl PrivateTemporary {
    fn new(parent: &Path) -> Result<Self> {
        let metadata = fs::symlink_metadata(parent).map_err(|_| Error::Unavailable)?;
        if !parent.is_absolute()
            || !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.uid() != Uid::current().as_raw()
            || metadata.permissions().mode() & 0o022 != 0
        {
            return Err(Error::Unavailable);
        }
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        for _ in 0..16 {
            let path = parent.join(format!(
                ".omavless-desktop-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
            {
                Ok(file) => return Ok(Self { path, file }),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err(Error::Unavailable),
            }
        }
        Err(Error::Unavailable)
    }
}
impl Drop for PrivateTemporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn nonblocking(fd: &impl AsFd) -> Result<()> {
    let flags = fcntl(fd, FcntlArg::F_GETFL).map_err(|_| Error::Unavailable)?;
    fcntl(
        fd,
        FcntlArg::F_SETFL(OFlag::from_bits_truncate(flags) | OFlag::O_NONBLOCK),
    )
    .map_err(|_| Error::Unavailable)?;
    Ok(())
}

/// Private implementation, not a public arbitrary-command API. Simultaneous bounded
/// nonblocking stdin/stdout prevents pipe deadlocks and never logs child errors.
fn execute(
    tool: &Path,
    args: &[OsString],
    input: &[u8],
    maximum: usize,
    timeout: Option<Duration>,
    dialog: bool,
) -> Result<Vec<u8>> {
    let mut child = ChildGuard(
        Command::new(tool)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| Error::Unavailable)?,
    );
    let mut stdin = child.0.stdin.take();
    let mut stdout = child.0.stdout.take().ok_or(Error::Unavailable)?;
    nonblocking(&stdout)?;
    nonblocking(stdin.as_ref().ok_or(Error::Unavailable)?)?;
    let started = Instant::now();
    let mut written = 0;
    let mut result = Vec::new();
    loop {
        if timeout.is_some_and(|t| started.elapsed() >= t) {
            return Err(Error::TimedOut);
        }
        if let Some(pipe) = stdin.as_mut() {
            if written == input.len() {
                stdin = None;
            } else {
                match pipe.write(&input[written..]) {
                    Ok(n) => written += n,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                    Err(_) => return Err(Error::Unavailable),
                }
            }
        }
        let status = child.0.try_wait().map_err(|_| Error::Unavailable)?;
        if let Some(status) = status
            && !status.success()
        {
            return Err(if dialog && status.code() == Some(1) {
                Error::Cancelled
            } else {
                Error::Unavailable
            });
        }
        let mut buffer = [0u8; 8192];
        let mut drained = false;
        match stdout.read(&mut buffer) {
            Ok(0) => drained = true,
            Ok(n) => {
                if result.len() + n > maximum {
                    return Err(Error::TooLarge);
                }
                result.extend_from_slice(&buffer[..n]);
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => drained = true,
            Err(_) => return Err(Error::Unavailable),
        }
        if status.is_some() && drained {
            return if written == input.len() {
                Ok(result)
            } else {
                Err(Error::Unavailable)
            };
        }
        thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{DirBuilderExt, symlink};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static COUNT: AtomicU64 = AtomicU64::new(0);
            let root = env::temp_dir().join(format!(
                "omavless-desktop-test-{}-{}",
                std::process::id(),
                COUNT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
            Self(root)
        }
        fn tool(&self, name: &str, body: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::write(&path, format!("#!/bin/bash\n{body}\n")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            path
        }
        fn helpers(&self) -> DesktopHelpers {
            DesktopHelpers::from_path(self.0.as_os_str())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn discovery_is_fixed_and_clipboard_independent() {
        let f = Fixture::new();
        assert_eq!(
            f.helpers().capabilities()["filePicker"],
            serde_json::Value::Null
        );
        assert_eq!(f.helpers().clipboard_read(), Err(Error::MissingClipboard));
        assert_eq!(f.helpers().pick_import(), Err(Error::MissingPicker));
        assert_eq!(f.helpers().edit(b"seed", &f.0), Err(Error::MissingEditor));
        assert_eq!(f.helpers().qr_png(b"seed"), Err(Error::MissingQr));
        for name in ["yad", "kdialog", "zenity"] {
            f.tool(name, "exit 0");
            assert_eq!(f.helpers().capabilities()["filePicker"], name);
        }
        let relative = DesktopHelpers::from_path(std::ffi::OsStr::new(".:relative:"));
        assert!(relative.search.is_empty());
        assert!(
            !f.helpers().capabilities()["gtk4FallbackAvailable"]
                .as_bool()
                .unwrap()
        );
    }

    #[test]
    fn clipboard_reads_mime_and_copies_only_stdin() {
        let f = Fixture::new();
        f.tool("wl-paste", "if test \"$1\" = --list-types; then printf 'text/plain\\n'; else test \"$*\" = '--no-newline --type text/plain' || exit 4; printf 'synthetic input'; fi");
        assert!(f.helpers().clipboard_read().unwrap() == b"synthetic input");
        f.tool("wl-copy", "test \"$*\" = '--type text/plain' || exit 4; IFS= read -r value; test \"$value\" = 'synthetic input' || exit 5");
        f.helpers().clipboard_copy(b"synthetic input\n").unwrap();
        f.tool("wl-paste", "printf 'private-error' >&2; exit 7");
        assert_eq!(f.helpers().clipboard_read(), Err(Error::Unavailable));
        assert!(!Error::Unavailable.to_string().contains("private-error"));
    }

    #[test]
    fn process_bounds_timeout_cancel_and_duplex_do_not_deadlock() {
        let f = Fixture::new();
        let tool = f.tool("test-tool", "printf '12345'");
        assert_eq!(
            execute(&tool, &[], b"", 4, Some(Duration::from_secs(1)), false),
            Err(Error::TooLarge)
        );
        f.tool("test-tool", "while :; do :; done");
        assert_eq!(
            execute(&tool, &[], b"", 4, Some(Duration::from_millis(30)), false),
            Err(Error::TimedOut)
        );
        f.tool("test-tool", "exit 1");
        assert_eq!(
            execute(&tool, &[], b"", 4, None, true),
            Err(Error::Cancelled)
        );
        f.tool("test-tool", "exit 2");
        assert_eq!(
            execute(&tool, &[], b"", 4, None, true),
            Err(Error::Unavailable)
        );
        f.tool("test-tool", "printf '%32768s' ''; /usr/bin/cat");
        let input = vec![b'x'; MAX_TEXT_BYTES];
        let output = execute(
            &tool,
            &[],
            &input,
            MAX_TEXT_BYTES + 32768,
            Some(Duration::from_secs(2)),
            false,
        )
        .unwrap();
        assert_eq!(output.len(), MAX_TEXT_BYTES + 32768);
    }

    #[test]
    fn file_input_is_content_not_path_and_refuses_unsafe_shape() {
        let f = Fixture::new();
        let path = f.0.join("profile $(false); link.txt");
        fs::write(&path, b"synthetic profile\n").unwrap();
        let bytes = path.as_os_str().as_encoded_bytes();
        assert!(read_import_file(bytes).unwrap() == b"synthetic profile\n");
        assert!(read_import_file(b"relative").is_err());
        assert!(read_import_file(b"/tmp/bad\npath").is_err());
        let link = f.0.join("link");
        symlink(&path, &link).unwrap();
        assert!(read_import_file(link.as_os_str().as_encoded_bytes()).is_err());
        let fifo = f.0.join("fifo");
        nix::unistd::mkfifo(&fifo, nix::sys::stat::Mode::S_IRUSR).unwrap();
        assert!(read_import_file(fifo.as_os_str().as_encoded_bytes()).is_err());
        fs::write(&path, [0xff]).unwrap();
        assert_eq!(read_import_file(bytes), Err(Error::InvalidInput));
        fs::write(&path, vec![b'x'; MAX_TEXT_BYTES + 1]).unwrap();
        assert_eq!(read_import_file(bytes), Err(Error::TooLarge));
        fs::write(&path, vec![b'x'; MAX_TEXT_BYTES]).unwrap();
        assert_eq!(read_import_file(bytes).unwrap().len(), MAX_TEXT_BYTES);
    }

    #[test]
    fn chooser_reads_selected_content_and_rejects_cancel_and_injection() {
        let f = Fixture::new();
        let profile = f.0.join("profile.txt");
        fs::write(&profile, b"synthetic profile").unwrap();
        f.tool("zenity", &format!("printf '%s\\n' '{}'", profile.display()));
        assert!(f.helpers().pick_import().unwrap() == b"synthetic profile");
        f.tool("zenity", "exit 1");
        assert_eq!(f.helpers().pick_import(), Err(Error::Cancelled));
        f.tool("zenity", "printf '/tmp/a\\n/tmp/b\\n'");
        assert_eq!(f.helpers().pick_import(), Err(Error::InvalidInput));
    }

    #[test]
    fn editor_seed_is_private_generic_title_and_cleanup_is_unconditional() {
        let f = Fixture::new();
        f.tool("zenity", "test \"$4\" = '--title=Edit profile' || exit 5; file=${3#--filename=}; test \"$(/usr/bin/stat -c %a \"$file\")\" = 600 || exit 6; /usr/bin/cat -- \"$file\"");
        assert!(f.helpers().edit(b"private seed", &f.0).unwrap() == b"private seed");
        assert_eq!(fs::read_dir(&f.0).unwrap().count(), 1);
        f.tool("zenity", "exit 1");
        assert_eq!(
            f.helpers().edit(b"private seed", &f.0),
            Err(Error::Cancelled)
        );
        assert_eq!(fs::read_dir(&f.0).unwrap().count(), 1);
        f.tool("zenity", "printf '\\377'");
        assert_eq!(
            f.helpers().edit(b"private seed", &f.0),
            Err(Error::InvalidInput)
        );
        assert_eq!(fs::read_dir(&f.0).unwrap().count(), 1);
    }

    #[test]
    fn qr_private_stdin_binary_bounds_and_export_atomic_permissions() {
        let f = Fixture::new();
        f.tool("qrencode", "test \"$*\" = '-o - -s 8 -m 2' || exit 5; IFS= read -r value; test \"$value\" = 'synthetic profile' || exit 6; printf '\\211PNG\\r\\n\\032\\n'");
        assert!(
            f.helpers()
                .qr_png(b"synthetic profile\n")
                .unwrap()
                .starts_with(b"\x89PNG")
        );
        f.tool("qrencode", "printf 'private-invalid-image'");
        assert_eq!(f.helpers().qr_png(b"seed"), Err(Error::Unavailable));
        let path = f.0.join("export $(false).txt");
        let bytes = path.as_os_str().as_encoded_bytes();
        export_file(bytes, b"synthetic profile\n").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        export_file(bytes, b"replacement").unwrap();
        assert!(fs::read(&path).unwrap() == b"replacement");
        let link = f.0.join("symlink");
        symlink(&path, &link).unwrap();
        assert!(export_file(link.as_os_str().as_encoded_bytes(), b"bad").is_err());
        assert!(fs::read(&path).unwrap() == b"replacement");
        assert_eq!(fs::read_dir(&f.0).unwrap().count(), 3);
    }

    #[test]
    fn cleanup_only_reaps_matching_dead_private_regular_files() {
        let f = Fixture::new();
        let dead = f.0.join(".omavless-desktop-4294967295-1");
        fs::write(&dead, b"synthetic seed").unwrap();
        fs::set_permissions(&dead, fs::Permissions::from_mode(0o600)).unwrap();
        let live =
            f.0.join(format!(".omavless-desktop-{}-1", std::process::id()));
        fs::write(&live, b"synthetic seed").unwrap();
        fs::set_permissions(&live, fs::Permissions::from_mode(0o600)).unwrap();
        let link = f.0.join(".omavless-desktop-4294967295-2");
        symlink(&live, &link).unwrap();
        assert_eq!(cleanup(&f.0).unwrap(), 1);
        assert!(!dead.exists());
        assert!(live.exists());
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn scratch_creation_refuses_missing_symlink_or_nonprivate_parents() {
        let f = Fixture::new();
        let missing = f.0.join("missing");
        assert_eq!(prepare_desktop_runtime(&missing), Err(Error::Unavailable));
        assert!(!missing.exists());
        let alias = f.0.join("alias");
        symlink(&f.0, &alias).unwrap();
        assert_eq!(prepare_desktop_runtime(&alias), Err(Error::Unavailable));
        assert!(!f.0.join("omavless-desktop").exists());
        fs::set_permissions(&f.0, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(prepare_desktop_runtime(&f.0), Err(Error::Unavailable));
        fs::set_permissions(&f.0, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(f.0.join("omavless-desktop"), b"sentinel").unwrap();
        assert_eq!(prepare_desktop_runtime(&f.0), Err(Error::Unavailable));
        assert!(fs::read(f.0.join("omavless-desktop")).unwrap() == b"sentinel");
    }

    #[test]
    fn picker_order_matches_actual_python_oracle() {
        let script =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/desktop_helpers_parity.py");
        let output = Command::new("python3").arg(script).output().unwrap();
        assert!(output.status.success(), "Desktop helper oracle failed");
        let oracle: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let reference = oracle["discovery"].as_array().unwrap();
        assert_eq!(reference.len(), 8);
        for (mask, expected) in reference.iter().enumerate() {
            let f = Fixture::new();
            for (bit, name) in ["zenity", "kdialog", "yad"].iter().enumerate() {
                if mask & (1 << bit) != 0 {
                    f.tool(name, "exit 0");
                }
            }
            let capabilities = f.helpers().capabilities();
            let provider = capabilities["filePicker"].as_str().unwrap_or("");
            assert_eq!(provider, expected["provider"].as_str().unwrap());
            assert_eq!(capabilities["configEditorAvailable"], expected["editor"]);
        }
        use sha2::{Digest, Sha256};
        let corpus: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/parity_cases/desktop-clipboard-v1.json"
        ))
        .unwrap();
        let results = oracle["clipboard"].as_array().unwrap();
        assert_eq!(results.len(), 5);
        for (case, expected) in corpus.as_array().unwrap().iter().zip(results) {
            let f = Fixture::new();
            f.tool("wl-paste", case["script"].as_str().unwrap());
            let result = f.helpers().clipboard_read();
            assert_eq!(result.is_ok(), expected["ok"].as_bool().unwrap());
            if let Ok(data) = result {
                assert_eq!(
                    format!("{:x}", Sha256::digest(data)),
                    expected["digest"].as_str().unwrap()
                );
            }
        }
    }
}
