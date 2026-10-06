// SPDX-License-Identifier: MIT

//! Explicit reviewed namespace fixture only; ignored, never a runtime adapter.
//! The fixed core is copied to a sealed memfd before bwrap consumes it on FD0.
//! No core execution is permitted before inside-namespace admission succeeds.

use super::*;
use nix::fcntl::{FcntlArg, OFlag, SealFlag, fcntl};
use nix::sys::memfd::{MFdFlags, memfd_create};
use nix::sys::statvfs::{FsFlags, statvfs};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Seek, SeekFrom};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::sync::atomic::{AtomicU64, Ordering};

const CORE_PATH: &str = "/usr/bin/mihomo";
const CORE_HASH: &str = "316eddc4eafde7aef1c77d7d00e3cd56f493e99478f60c6a25ce17dfbe4f4a5f";
const CORE_SIZE: u64 = 62_054_520;
const BWRAP_PATH: &str = "/usr/bin/bwrap";
const BWRAP_HASH: &str = "7c44fa8e7326e62e81ab3f70ff682bfc0eb3b447b39cf9fbb779a31948364762";
const CORE_PORT: u16 = 18080;
const SELECTOR: &str = "OMAVLESS_TEST_S1_ISOLATED_CORE";
const NAMESPACES: &str = "OMAVLESS_TEST_S1_PARENT_NAMESPACES";
const INNER: &str = "app_proxy::child_scope::isolated_core::tests::namespace_consumer";
const CLIENT: &str = "app_proxy::child_scope::isolated_core::tests::namespace_http_client";
const LOSS_READY: &[u8] = b"S1_FIXED_CORE_PARENT_LOSS_READY\n";
const CAPTURE_LIMIT: usize = 64 * 1024;
const NS: [&str; 6] = ["user", "net", "pid", "mnt", "ipc", "uts"];

fn phase<T>(label: &'static str, result: Result<T>) -> Result<T> {
    if let Err(error) = &result {
        eprintln!("S1 isolated fixture phase={label} code={error:?}");
    }
    result
}

// Reviewed against upstream v1.19.32 / 88dcbf7f. Explicitly override defaults
// that otherwise retain DNS servers, TUN auto-route, profile persistence or UI URL.
const CONFIG: &str = r#"port: 18080
bind-address: 127.0.0.1
allow-lan: false
socks-port: 0
mixed-port: 0
redir-port: 0
tproxy-port: 0
ss-config: ''
vmess-config: ''
mode: direct
ipv6: false
find-process-mode: off
log-level: silent
geo-auto-update: false
external-controller: ''
external-controller-tls: ''
external-controller-unix: ''
external-controller-pipe: ''
external-ui: ''
external-ui-url: ''
external-doh-server: ''
geox-url: {geoip: '', geosite: '', mmdb: '', asn: ''}
profile: {store-selected: false, store-fake-ip: false}
dns:
  enable: false
  listen: ''
  use-hosts: false
  use-system-hosts: false
  nameserver: []
  # Parser-required numeric placeholder; DNS disabled, origin already numeric.
  default-nameserver: ['127.0.0.1']
  fallback: []
  proxy-server-nameserver: []
  direct-nameserver: []
tun:
  enable: false
  auto-route: false
  auto-redirect: false
  auto-detect-interface: false
  dns-hijack: []
ntp: {enable: false, write-to-system: false}
iptables: {enable: false, dns-redirect: false}
tuic-server: {enable: false}
sniffer: {enable: false}
proxies: []
proxy-groups: []
proxy-providers: {}
rule-providers: {}
listeners: []
tunnels: []
rules: ['MATCH,DIRECT']
"#;

fn fixed_source(path: &str, hash: &str, size: u64) -> Result<Vec<u8>> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(OFlag::O_NOFOLLOW.bits())
        .open(path)
        .map_err(|_| Error::Unavailable)?;
    let before = file.metadata().map_err(|_| Error::Unavailable)?;
    if !before.is_file()
        || before.uid() != 0
        || before.mode() & 0o7777 != 0o755
        || before.nlink() != 1
        || before.len() != size
    {
        return Err(Error::Unavailable);
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(size + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Unavailable)?;
    let after = file.metadata().map_err(|_| Error::Unavailable)?;
    if bytes.len() as u64 != size
        || format!("{:x}", Sha256::digest(&bytes)) != hash
        || before.dev() != after.dev()
        || before.ino() != after.ino()
        || before.len() != after.len()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.mtime() != after.mtime()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
        || before.mode() != after.mode()
    {
        return Err(Error::Unavailable);
    }
    Ok(bytes)
}

fn sealed_bytes(bytes: &[u8]) -> Result<File> {
    let fd = memfd_create(
        "s1-fixed-core",
        MFdFlags::MFD_CLOEXEC | MFdFlags::MFD_ALLOW_SEALING,
    )
    .map_err(|_| Error::Unavailable)?;
    let mut file = File::from(fd);
    file.write_all(bytes).map_err(|_| Error::Unavailable)?;
    let seals = SealFlag::F_SEAL_WRITE
        | SealFlag::F_SEAL_GROW
        | SealFlag::F_SEAL_SHRINK
        | SealFlag::F_SEAL_SEAL;
    fcntl(&file, FcntlArg::F_ADD_SEALS(seals)).map_err(|_| Error::Unavailable)?;
    if fcntl(&file, FcntlArg::F_GET_SEALS).map_err(|_| Error::Unavailable)? != seals.bits() {
        return Err(Error::Unavailable);
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| Error::Unavailable)?;
    Ok(file)
}

fn namespaces() -> Result<String> {
    NS.into_iter()
        .map(|name| {
            fs::read_link(format!("/proc/self/ns/{name}"))
                .map_err(|_| Error::Unavailable)?
                .into_os_string()
                .into_string()
                .map_err(|_| Error::Unavailable)
        })
        .collect::<Result<Vec<_>>>()
        .map(|ids| ids.join("|"))
}

// Preserve HOME verbatim. Every other inherited variable is absent, including
// proxies, CLASH_POST_UP/DOWN, controller overrides, loader/Go/startup selectors.
// No process-global environment setter or rewritten HOME/XDG path exists.
fn scrub(command: &mut Command) {
    for (name, _) in std::env::vars_os() {
        if name != "HOME" {
            command.env_remove(name);
        }
    }
}

fn fresh_scratch() -> Result<PathBuf> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let home = std::env::var_os("HOME").ok_or(Error::Unavailable)?;
    let base = PathBuf::from(home).join(".cache/ovtmp-root");
    let metadata = fs::symlink_metadata(&base).map_err(|_| Error::Unavailable)?;
    if !metadata.is_dir()
        || metadata.uid() != nix::unistd::geteuid().as_raw()
        || metadata.mode() & 0o7777 != 0o700
        || base.canonicalize().map_err(|_| Error::Unavailable)? != base
    {
        return Err(Error::Unavailable);
    }
    let path = base.join(format!(
        "s1-core-isolated-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&path)
        .map_err(|_| Error::Unavailable)?;
    Ok(path) // Always retained as known dedicated evidence; no recursive cleanup.
}

fn namespace_command(core: File, scratch: &Path, loss: bool) -> Result<Command> {
    // bwrap is a trusted root-owned package dependency, selected by fixed path.
    fixed_source(BWRAP_PATH, BWRAP_HASH, 84_464)?;
    fixed_source(
        "/usr/bin/getcap",
        "3d8bc2191227c1ee2fad5d83c025e8b62e283338673a1630babe6b81ee232814",
        14_352,
    )?;
    let mut probe = Command::new("/usr/bin/getcap");
    scrub(&mut probe);
    probe
        .args([BWRAP_PATH, CORE_PATH, "/usr/bin/getcap"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut probe = OwnedChild::spawn(probe, Instant::now() + Duration::from_secs(2))?;
    if !probe.finish()?.success() || !probe.out.bytes.is_empty() || !probe.err.bytes.is_empty() {
        return Err(Error::Unavailable);
    }
    let test = std::env::current_exe().map_err(|_| Error::Unavailable)?;
    let meta = fs::symlink_metadata(&test).map_err(|_| Error::Unavailable)?;
    if !meta.is_file() || meta.nlink() != 1 || meta.mode() & 0o7777 != 0o500 {
        return Err(Error::Unavailable); // Require the separately frozen test copy.
    }
    let mut command = Command::new(BWRAP_PATH);
    scrub(&mut command);
    command
        .args([
            "--ro-bind",
            "/",
            "/",
            "--dev",
            "/dev",
            "--proc",
            "/proc",
            "--tmpfs",
            "/run",
            "--unshare-user",
            "--unshare-net",
            "--unshare-pid",
            "--as-pid-1",
            "--unshare-ipc",
            "--unshare-uts",
            "--cap-drop",
            "ALL",
            "--new-session",
            "--die-with-parent",
            "--bind",
        ])
        .arg(scratch)
        .arg("/tmp")
        .args([
            "--perms",
            "0500",
            "--ro-bind-data",
            "0",
            "/tmp/fixed-core",
            "--ro-bind",
        ])
        .arg(test)
        .arg("/tmp/fixed-tests")
        .args([
            "--chdir",
            "/tmp",
            "--setenv",
            "TMPDIR",
            "/tmp",
            "--",
            "/tmp/fixed-tests",
            "--exact",
            INNER,
            "--ignored",
            "--test-threads=1",
        ])
        .env(SELECTOR, if loss { "parent-loss" } else { "exchange" })
        .env(NAMESPACES, namespaces()?)
        .stdin(Stdio::from(core))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    Ok(command)
}

struct Capture {
    bytes: Vec<u8>,
    eof: bool,
}
impl Capture {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            eof: false,
        }
    }
    fn drain(&mut self, pipe: &mut impl Read, end: Instant) -> Result<()> {
        loop {
            gate(end)?;
            let mut buffer = [0; 512];
            match pipe.read(&mut buffer) {
                Ok(0) => {
                    self.eof = true;
                    return Ok(());
                }
                Ok(n) => {
                    if self.bytes.len() + n > CAPTURE_LIMIT {
                        return Err(Error::InvalidFrame);
                    }
                    self.bytes.extend_from_slice(&buffer[..n]);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
                Err(_) => return Err(Error::Unavailable),
            }
        }
    }
}

// One original std Child; no PID search/adoption, exported authority or signals
// through a copied process identifier. Both pipes are drained nonblockingly.
struct OwnedChild {
    child: Option<Child>,
    end: Instant,
    unknown: bool,
    completed: Option<ExitStatus>,
    pipes_ready: bool,
    out: Capture,
    err: Capture,
}
impl OwnedChild {
    fn spawn(mut command: Command, end: Instant) -> Result<Self> {
        gate(end)?;
        let child = command.spawn().map_err(|_| Error::Unavailable)?;
        let mut owner = Self {
            child: Some(child),
            end,
            unknown: false,
            completed: None,
            pipes_ready: false,
            out: Capture::new(),
            err: Capture::new(),
        };
        if let Some(pipe) = owner.child.as_ref().and_then(|c| c.stdout.as_ref()) {
            fcntl(pipe, FcntlArg::F_SETFL(OFlag::O_NONBLOCK)).map_err(|_| Error::Unavailable)?;
        } else {
            owner.out.eof = true;
        }
        if let Some(pipe) = owner.child.as_ref().and_then(|c| c.stderr.as_ref()) {
            fcntl(pipe, FcntlArg::F_SETFL(OFlag::O_NONBLOCK)).map_err(|_| Error::Unavailable)?;
        } else {
            owner.err.eof = true;
        }
        owner.pipes_ready = true;
        gate(end)?;
        Ok(owner)
    }
    fn running(&mut self) -> Result<bool> {
        if self.unknown {
            return Err(Error::Unavailable);
        }
        if self.completed.is_some() {
            return Ok(false);
        }
        match self.child.as_mut().ok_or(Error::Unavailable)?.try_wait() {
            Ok(Some(status)) => {
                self.completed = Some(status);
                Ok(false)
            }
            Ok(None) => Ok(true),
            Err(_) => {
                self.unknown = true;
                Err(Error::Unavailable)
            }
        }
    }
    fn pump(&mut self) -> Result<()> {
        if !self.pipes_ready {
            return Err(Error::Unavailable);
        }
        let child = self.child.as_mut().ok_or(Error::Unavailable)?;
        if !self.out.eof
            && let Some(pipe) = child.stdout.as_mut()
        {
            self.out.drain(pipe, self.end)?;
        }
        if !self.err.eof
            && let Some(pipe) = child.stderr.as_mut()
        {
            self.err.drain(pipe, self.end)?;
        }
        Ok(())
    }
    fn finish(&mut self) -> Result<ExitStatus> {
        loop {
            gate(self.end)?;
            self.pump()?;
            let running = self.running()?;
            if !running && self.out.eof && self.err.eof {
                return self.completed.ok_or(Error::Unavailable);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn save(&self, directory: &Path, prefix: &str) -> Result<()> {
        for (suffix, bytes) in [("stdout", &self.out.bytes), ("stderr", &self.err.bytes)] {
            OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(directory.join(format!("{prefix}.{suffix}")))
                .map_err(|_| Error::Unavailable)?
                .write_all(bytes)
                .map_err(|_| Error::Unavailable)?;
        }
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        if self.unknown {
            return Err(Error::Unavailable);
        }
        if self.running()?
            && self
                .child
                .as_mut()
                .ok_or(Error::Unavailable)?
                .kill()
                .is_err()
        {
            self.unknown = true;
            return Err(Error::Unavailable);
        }
        self.finish().map(|_| ())
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if self.completed.is_none() && !self.unknown && self.stop().is_err() {
            self.unknown = true;
        }
        if self.unknown
            && let Some(child) = self.child.take()
        {
            std::mem::forget(child);
        }
    }
}

fn admission_refusal(check: &'static str) -> Error {
    eprintln!("S1 admission check={check} ok=false");
    Error::Unavailable
}

fn admission() -> Result<()> {
    if std::process::id() != 1 {
        return Err(admission_refusal("pid-1"));
    }
    let parent = std::env::var(NAMESPACES).map_err(|_| Error::Unavailable)?;
    let current = namespaces()?;
    let before: Vec<_> = parent.split('|').collect();
    let now: Vec<_> = current.split('|').collect();
    if parent.len() > 256
        || before.len() != NS.len()
        || before.iter().zip(&now).any(|(a, b)| a == b || a.is_empty())
    {
        return Err(admission_refusal("fresh-namespaces"));
    }
    let status = fs::read_to_string("/proc/self/status").map_err(|_| Error::Unavailable)?;
    for (field, value) in [
        ("NoNewPrivs:", "1"),
        ("CapInh:", "0000000000000000"),
        ("CapPrm:", "0000000000000000"),
        ("CapEff:", "0000000000000000"),
        ("CapBnd:", "0000000000000000"),
        ("CapAmb:", "0000000000000000"),
    ] {
        if !status
            .lines()
            .any(|line| line.strip_prefix(field).is_some_and(|v| v.trim() == value))
        {
            return Err(admission_refusal(field));
        }
    }
    let devices = fs::read_to_string("/proc/self/net/dev").map_err(|_| Error::Unavailable)?;
    let names: Vec<_> = devices
        .lines()
        .filter_map(|line| line.split_once(':').map(|(name, _)| name.trim()))
        .collect();
    if names != ["lo"]
        || !statvfs("/")
            .map_err(|_| Error::Unavailable)?
            .flags()
            .contains(FsFlags::ST_RDONLY)
        || !statvfs("/tmp/fixed-core")
            .map_err(|_| Error::Unavailable)?
            .flags()
            .contains(FsFlags::ST_RDONLY)
    {
        return Err(admission_refusal("loopback-and-read-only-mounts"));
    }
    let core = fs::symlink_metadata("/tmp/fixed-core").map_err(|_| Error::Unavailable)?;
    if !core.is_file()
        || core.mode() & 0o7777 != 0o500
        // bwrap 0.12.0 unlinks the private source inode after binding it;
        // the visible read-only bind remains alive with st_nlink == 0.
        || core.nlink() != 0
        || core.len() != CORE_SIZE
    {
        return Err(admission_refusal("copied-core-metadata"));
    }
    let bytes = fs::read("/tmp/fixed-core").map_err(|_| Error::Unavailable)?;
    if format!("{:x}", Sha256::digest(&bytes)) != CORE_HASH {
        return Err(admission_refusal("copied-core-hash"));
    }
    if fs::read_dir("/run")
        .map_err(|_| Error::Unavailable)?
        .next()
        .is_some()
    {
        return Err(admission_refusal("empty-run"));
    }
    if !matches!(fs::symlink_metadata("/dev/net/tun"), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
    {
        return Err(admission_refusal("absent-tun-device"));
    }
    // bwrap 0.12.0 itself sets PWD after --chdir. Admit only that fixed
    // generated value; scrub() removes it again before the core is spawned.
    if std::env::var_os("PWD").as_deref() != Some(std::ffi::OsStr::new("/tmp"))
        || std::env::vars_os().any(|(k, _)| {
            k != "HOME" && k != "TMPDIR" && k != SELECTOR && k != NAMESPACES && k != "PWD"
        })
    {
        return Err(admission_refusal("fixed-environment"));
    }
    Ok(())
}

fn core_started(owner: &mut OwnedChild, end: Instant) -> Result<()> {
    let address = SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, CORE_PORT));
    loop {
        gate(end)?;
        owner.pump()?;
        if !owner.running()? {
            return Err(Error::ChildFailed);
        }
        if TcpStream::connect_timeout(&address, Duration::from_millis(20)).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn proxy_connection_present() -> Result<bool> {
    let mut text = String::new();
    File::open("/proc/self/net/tcp")
        .map_err(|_| Error::Unavailable)?
        .take((CAPTURE_LIMIT + 1) as u64)
        .read_to_string(&mut text)
        .map_err(|_| Error::Unavailable)?;
    if text.len() > CAPTURE_LIMIT {
        return Err(Error::InvalidFrame);
    }
    let target = format!("0100007F:{CORE_PORT:04X}");
    Ok(text.lines().skip(1).any(|line| {
        let fields: Vec<_> = line.split_whitespace().collect();
        fields.len() >= 4 && fields[2] == target && fields[3] == "01"
    }))
}

fn exchange(origin: &TcpListener, proxy: bool, end: Instant) -> Result<()> {
    let address = fixture_address(origin)?;
    let mut command = Command::new("/tmp/fixed-tests");
    scrub(&mut command);
    command
        .args(["--exact", CLIENT, "--ignored", "--test-threads=1"])
        .env(MODE, if proxy { "http" } else { "baseline" })
        .env(ORIGIN, address.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if proxy {
        let endpoint = format!("http://127.0.0.1:{CORE_PORT}");
        for key in ["http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY"] {
            command.env(key, &endpoint);
        }
        command.env("no_proxy", "").env("NO_PROXY", "");
    }
    let mut client = OwnedChild::spawn(command, end)?;
    let mut stream = loop {
        gate(end)?;
        client.pump()?;
        if !client.running()? {
            return Err(Error::ChildFailed);
        }
        match origin.accept() {
            Ok((stream, _)) => break stream,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(5))
            }
            Err(_) => return Err(Error::Unavailable),
        }
    };
    if !request_matches(&read_header(&mut stream, end)?, address, false)
        || proxy_connection_present()? != proxy
    {
        return Err(Error::InvalidFrame);
    }
    socket_deadline(&stream, end)?;
    stream
        .write_all(&fixture_response(proxy))
        .map_err(|_| Error::Unavailable)?;
    drop(stream);
    if !client.finish()?.success()
        || !fixed_case_receipt(&client.out.bytes, &client.err.bytes, CLIENT)
    {
        return Err(Error::ChildFailed);
    }
    match origin.accept() {
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(()),
        _ => Err(Error::InvalidFrame),
    }
}

fn inner() -> Result<()> {
    let end = Instant::now() + Duration::from_secs(20);
    phase("admission", admission())?; // Before every core execution/fault entry.
    fs::DirBuilder::new()
        .mode(0o700)
        .create("/tmp/core-state")
        .map_err(|_| Error::Unavailable)?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open("/tmp/core-state/config.yaml")
        .map_err(|_| Error::Unavailable)?
        .write_all(CONFIG.as_bytes())
        .map_err(|_| Error::Unavailable)?;
    let mut command = Command::new("/tmp/fixed-core");
    scrub(&mut command);
    command
        .args(["-d", "/tmp/core-state", "-f", "/tmp/core-state/config.yaml"])
        .stdin(Stdio::null());
    let selected = std::env::var(SELECTOR).map_err(|_| Error::Unavailable)?;
    if selected != "parent-loss" && selected != "exchange" {
        return Err(Error::Unavailable);
    }
    let loss = selected == "parent-loss";
    if loss {
        command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    } else {
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
    }
    let mut core = OwnedChild::spawn(command, end)?;
    phase(
        "core-readiness",
        core_started(&mut core, end.min(Instant::now() + Duration::from_secs(5))),
    )?;
    if loss {
        if !core.running()? {
            return Err(Error::ChildFailed);
        }
        // Deliberately bypass Rust Drop while this original core is known live.
        // Its inherited pipe writers must disappear before the outer test passes.
        std::io::stdout()
            .write_all(LOSS_READY)
            .map_err(|_| Error::Unavailable)?;
        std::io::stdout().flush().map_err(|_| Error::Unavailable)?;
        std::process::exit(0);
    }
    let parent = ParentProxyEnvironment::capture();
    let origin = TcpListener::bind(("127.0.0.1", 0)).map_err(|_| Error::Unavailable)?;
    origin
        .set_nonblocking(true)
        .map_err(|_| Error::Unavailable)?;
    if fixture_address(&origin)?.port() == CORE_PORT {
        return Err(Error::Unavailable);
    }
    let result = phase("proxied-client", exchange(&origin, true, end))
        .and_then(|()| phase("direct-client", exchange(&origin, false, end)));
    let saved = core.save(Path::new("/tmp"), "core");
    result?;
    saved?;
    if !parent.unchanged() {
        return Err(Error::ParentChanged);
    }
    if !core.running()? {
        return Err(Error::ChildFailed);
    }
    phase("original-core-stop", core.stop())?;
    Ok(())
}

fn outer(loss: bool) -> Result<()> {
    let end = Instant::now() + Duration::from_secs(30);
    let parent = ParentProxyEnvironment::capture();
    let bytes = phase("specimen", fixed_source(CORE_PATH, CORE_HASH, CORE_SIZE))?;
    let core = phase("sealed-snapshot", sealed_bytes(&bytes))?;
    drop(bytes);
    let scratch = fresh_scratch()?;
    let command = phase(
        "namespace-prerequisites",
        namespace_command(core, &scratch, loss),
    )?;
    let mut sandbox = phase("namespace-spawn", OwnedChild::spawn(command, end))?;
    let result = phase("namespace-completion", sandbox.finish());
    let saved = sandbox.save(&scratch, "namespace");
    let status = result?;
    saved?;
    if !parent.unchanged() {
        return Err(Error::ParentChanged);
    }
    if !status.success() {
        return Err(Error::ChildFailed);
    }
    if loss {
        if sandbox
            .out
            .bytes
            .windows(LOSS_READY.len())
            .filter(|w| *w == LOSS_READY)
            .count()
            != 1
        {
            return Err(Error::ChildFailed);
        }
    } else {
        if !fixed_case_receipt(&sandbox.out.bytes, &sandbox.err.bytes, INNER) {
            return Err(Error::ChildFailed);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sealed_snapshot_cannot_be_rewritten_or_grown() {
        let mut file = sealed_bytes(b"fixed synthetic executable bytes").unwrap();
        assert!(file.write_all(b"other").is_err());
        assert!(file.set_len(1).is_err());
        assert!(file.set_len(100).is_err());
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"fixed synthetic executable bytes");
    }
    #[test]
    #[ignore = "ROOT+Astra reviewed exact frozen artifact only; executes fixed core in fresh namespaces"]
    fn isolated_core_http_exchange() {
        outer(false).expect("isolated original-core exchange");
    }
    #[test]
    #[ignore = "ROOT+Astra reviewed parent-loss test; no Drop-based core cleanup"]
    fn isolated_core_parent_loss() {
        outer(true).expect("isolated original supervisor loss");
    }
    #[test]
    #[ignore = "fixed namespace entry only, never directly select on the host"]
    fn namespace_consumer() {
        inner().expect("namespace core fixture");
    }
    #[test]
    #[ignore = "fixed ureq entry only inside the reviewed namespace"]
    fn namespace_http_client() {
        super::super::tests::fixed_consumer();
    }
}
