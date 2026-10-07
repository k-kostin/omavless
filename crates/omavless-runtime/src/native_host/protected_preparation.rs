// SPDX-License-Identifier: MIT
//! Private protected preparation/start on the original NativeLifecycleHost.
//! Coverage issuance is restricted to the qualified development Rule policy.
//! Canonical source/validation/readiness are
//! not socket-mark evidence; ordinary templates and staged paths stay separate.

use super::*;
use crate::desired::RoutingMode;
use omavless_profile::canonical::CanonicalProfile;
use omavless_profile::vless_query::{VlessSecurity, VlessTransport};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs::{File, Metadata, OpenOptions};
use std::io::Write;
use std::net::Ipv4Addr;
use std::os::unix::fs::{FileExt, OpenOptionsExt};
mod interval;
mod validation;

const STAGING: &str = ".config.k1.candidate.json";
const MAX_CONFIG: u64 = 64 * 1024;
const MAX_CORE: u64 = 128 * 1024 * 1024;
const PROFILE: &str = "OMAVLESS_K1";
const RESOLVER: &str = "https://1.1.1.1/dns-query#PROXY";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PreparationError {
    Unsupported,
    Refused,
    Changed,
    OutcomeUnknown,
}

/// Move-only, private-field binding. No public constructor or deserialization.
pub(crate) struct ArmAdmission {
    desired: DesiredState,
    config: [u8; 32],
    core: [u8; 32],
    coverage: Coverage,
}
struct Coverage {
    core: [u8; 32],
    pair: crate::managed_pair::ProtectedPairIdentity,
    policy: PolicyVersion,
    config: [u8; 32],
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum PolicyVersion {
    RuleTcpVerifiedTlsDohV1,
}
struct RenderedPolicy {
    version: PolicyVersion,
    config: [u8; 32],
}
// Static development-only decision, no flag, argument or imported receipt.
// Rule42 qualified this exact policy/core/package; the retained package holder
// below still enforces that closed identity. Native integration and product
// acceptance remain separate. Coverage is created only AFTER original
// validation and the same Bound's postchecks, never by this decision alone.
fn approved_policy_decision() -> Result<(), PreparationError> {
    Ok(())
}
/// Only constructed locally AFTER original Validation::complete and restored
/// host-owned Bound postchecks; cannot escape or outlive those originals.
struct ValidatedPreparation<'a> {
    bound: &'a Bound,
    pair: &'a ManagedPair,
}
fn issue_coverage(validated: ValidatedPreparation<'_>) -> Result<Coverage, PreparationError> {
    let bound = validated.bound;
    let package = bound.package.as_ref().ok_or(PreparationError::Refused)?;
    package
        .recheck(validated.pair, &bound.core.file)
        .map_err(|_| PreparationError::Changed)?;
    if bound.policy.version != PolicyVersion::RuleTcpVerifiedTlsDohV1
        || bound.policy.config != bound.config.digest
    {
        return Err(PreparationError::Changed);
    }
    Ok(Coverage {
        core: bound.core.digest,
        pair: package.identity(),
        policy: bound.policy.version,
        config: bound.config.digest,
    })
}

/// No Debug, Clone, serialization, external constructor or copied receipt.
pub(super) struct Preparation {
    // None records entry into publication even if that operation fails.
    bound: Option<Bound>,
    admitted: bool,
    started: bool,
}

struct Bound {
    desired: DesiredState,
    store_digest: [u8; 32],
    core: HeldFile,
    config: HeldFile,
    data: HeldDirectory,
    scratch: Option<interval::Scratch>,
    policy: RenderedPolicy,
    package: Option<crate::managed_pair::ProtectedPackage>,
    capacity: PreparationCapacity,
}
struct AcquiredBound(Option<Bound>);
impl Drop for AcquiredBound {
    fn drop(&mut self) {
        if let Some(bound) = self.0.take() {
            std::mem::forget(bound);
        }
    }
}

struct PreparationCapacity {
    ceiling: usize,
}
impl PreparationCapacity {
    fn inventory() -> Result<usize, PreparationError> {
        fs::read_dir("/proc/self/fd")
            .map_err(|_| PreparationError::Refused)?
            .try_fold(0usize, |n, entry| {
                entry.map_err(|_| PreparationError::Refused)?;
                n.checked_add(1)
                    .filter(|v| *v <= 256)
                    .ok_or(PreparationError::Refused)
            })
    }
    fn reserve() -> Result<Self, PreparationError> {
        // Three package originals + staged/config/data/scratch/parent publication and
        // validator null/exec-error plumbing, with transient iterator/hash margin.
        let (soft, _) = nix::sys::resource::getrlimit(nix::sys::resource::Resource::RLIMIT_NOFILE)
            .map_err(|_| PreparationError::Refused)?;
        Self::from_inventory(Self::inventory()?, soft)
    }
    fn from_inventory(count: usize, soft: u64) -> Result<Self, PreparationError> {
        let ceiling = count.checked_add(17).ok_or(PreparationError::Refused)?;
        if ceiling > 256 || ceiling as u64 > soft {
            return Err(PreparationError::Refused);
        }
        Ok(Self { ceiling })
    }
    fn check(&self) -> Result<(), PreparationError> {
        let (soft, _) = nix::sys::resource::getrlimit(nix::sys::resource::Resource::RLIMIT_NOFILE)
            .map_err(|_| PreparationError::Changed)?;
        if Self::inventory()? > self.ceiling || soft < self.ceiling as u64 {
            return Err(PreparationError::Changed);
        }
        Ok(())
    }
}

struct HeldDirectory {
    file: File,
    metadata: Metadata,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct DirectoryIdentity {
    dev: u64,
    ino: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    nlink: u64,
}
impl DirectoryIdentity {
    #[cfg(all(test, feature = "netguard-native-scenario"))]
    fn first_difference(self, original: Self) -> crate::protected_native_diagnostic::PostIdentity {
        use crate::protected_native_diagnostic::PostIdentity;
        // Diagnostic-only classification of already captured values, after the
        // unchanged production predicate rejected. Match its field order.
        if self.dev != original.dev {
            PostIdentity::Dev
        } else if self.ino != original.ino {
            PostIdentity::Ino
        } else if self.mode != original.mode {
            PostIdentity::Mode
        } else if self.uid != original.uid {
            PostIdentity::Uid
        } else if self.gid != original.gid {
            PostIdentity::Gid
        } else if self.nlink != original.nlink {
            PostIdentity::Nlink
        } else {
            PostIdentity::NotRecorded
        }
    }

    fn of(metadata: &Metadata) -> Self {
        Self {
            dev: metadata.dev(),
            ino: metadata.ino(),
            mode: metadata.mode(),
            uid: metadata.uid(),
            gid: metadata.gid(),
            nlink: metadata.nlink(),
        }
    }
}
impl HeldDirectory {
    fn capture(path: &Path, uid: u32) -> Result<Self, PreparationError> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_DIRECTORY)
            .open(path)
            .map_err(|_| PreparationError::Refused)?;
        let metadata = file.metadata().map_err(|_| PreparationError::Refused)?;
        if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o7777 != 0o700 {
            return Err(PreparationError::Refused);
        }
        let held = Self { file, metadata };
        held.recheck(path)?;
        Ok(held)
    }
    fn recheck(&self, path: &Path) -> Result<(), PreparationError> {
        if !same(
            &self.metadata,
            &self
                .file
                .metadata()
                .map_err(|_| PreparationError::Changed)?,
        ) || !same(
            &self.metadata,
            &fs::symlink_metadata(path).map_err(|_| PreparationError::Changed)?,
        ) {
            return Err(PreparationError::Changed);
        }
        Ok(())
    }
}

struct HeldFile {
    file: File,
    metadata: Metadata,
    digest: [u8; 32],
    limit: u64,
}

fn same(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.mode() == b.mode()
        && a.uid() == b.uid()
        && a.gid() == b.gid()
        && a.nlink() == b.nlink()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}

fn digest_file(file: &File, limit: u64) -> Result<[u8; 32], PreparationError> {
    let mut digest = Sha256::new();
    let mut offset = 0;
    let mut bytes = [0_u8; 16384];
    loop {
        let read = file
            .read_at(&mut bytes, offset)
            .map_err(|_| PreparationError::Changed)?;
        if read == 0 {
            break;
        }
        offset += read as u64;
        if offset > limit {
            return Err(PreparationError::Refused);
        }
        digest.update(&bytes[..read]);
    }
    Ok(digest.finalize().into())
}

impl HeldFile {
    fn capture(path: &Path, uid: u32, mode: u32, limit: u64) -> Result<Self, PreparationError> {
        let metadata = fs::symlink_metadata(path).map_err(|_| PreparationError::Refused)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.uid() != uid
            || metadata.mode() & 0o7777 != mode
            || metadata.nlink() != 1
            || metadata.len() == 0
            || metadata.len() > limit
        {
            return Err(PreparationError::Refused);
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| PreparationError::Refused)?;
        if !same(
            &metadata,
            &file.metadata().map_err(|_| PreparationError::Changed)?,
        ) {
            return Err(PreparationError::Changed);
        }
        let held = Self {
            digest: digest_file(&file, limit)?,
            file,
            metadata,
            limit,
        };
        held.recheck(path)?;
        Ok(held)
    }

    fn recheck(&self, path: &Path) -> Result<(), PreparationError> {
        for metadata in [self.file.metadata(), fs::symlink_metadata(path)] {
            if !same(
                &self.metadata,
                &metadata.map_err(|_| PreparationError::Changed)?,
            ) {
                return Err(PreparationError::Changed);
            }
        }
        if digest_file(&self.file, self.limit)? != self.digest
            || !same(
                &self.metadata,
                &self
                    .file
                    .metadata()
                    .map_err(|_| PreparationError::Changed)?,
            )
            || !same(
                &self.metadata,
                &fs::symlink_metadata(path).map_err(|_| PreparationError::Changed)?,
            )
        {
            return Err(PreparationError::Changed);
        }
        Ok(())
    }
}

#[cfg(test)]
fn render(profile: CanonicalProfile, controller: &Path) -> Result<Vec<u8>, PreparationError> {
    render_bound(profile, controller).map(|(bytes, _)| bytes)
}
fn render_bound(
    profile: CanonicalProfile,
    controller: &Path,
) -> Result<(Vec<u8>, RenderedPolicy), PreparationError> {
    let CanonicalProfile::Vless(vless) = &profile else {
        return Err(PreparationError::Unsupported);
    };
    let facts = vless.facts();
    if facts.transport != VlessTransport::Tcp
        || facts.security != VlessSecurity::Tls
        || facts.flow.is_some()
        || facts.packet_encoding.is_some()
        || facts.allow_insecure
        || facts.encryption_enabled
        || facts.experimental_feature_count != 0
        || facts.compatibility_note_present
    {
        return Err(PreparationError::Unsupported);
    }
    let ip: Ipv4Addr = profile
        .private_endpoint()
        .parse()
        .map_err(|_| PreparationError::Unsupported)?;
    if ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_broadcast()
    {
        return Err(PreparationError::Unsupported);
    }
    // This is the canonical renderer's structured result, never arbitrary YAML.
    // Any newly emitted option requires its own review rather than inheriting
    // a future renderer extension into the protected candidate.
    let mut proxy = profile.private_diagnostic_model();
    let object = proxy.as_object_mut().ok_or(PreparationError::Refused)?;
    let allowed = [
        "name",
        "type",
        "server",
        "port",
        "uuid",
        "udp",
        "network",
        "encryption",
        "tls",
        "servername",
    ];
    if object.keys().any(|key| !allowed.contains(&key.as_str()))
        || object.get("type") != Some(&json!("vless"))
        || object.get("network") != Some(&json!("tcp"))
        || object.get("tls") != Some(&json!(true))
        || object.get("encryption") != Some(&json!("none"))
    {
        return Err(PreparationError::Unsupported);
    }
    object.insert("name".into(), json!(PROFILE));
    // UDP=false alone does not prevent VLESS packet forwarding in global mode.
    // The fixed first NETWORK rule below rejects non-hijacked UDP instead.
    // TLS verification remains enabled.
    object.insert("udp".into(), json!(false));
    let controller = controller
        .to_str()
        .filter(|s| !s.chars().any(char::is_control))
        .ok_or(PreparationError::Refused)?;
    // JSON is canonical YAML input to Mihomo; no string replacement, merge keys,
    // inherited provider/geodata paths, cached selection or bootstrap hostname.
    let value = json!({
        "mode": "rule", "log-level": "silent", "ipv6": false,
        "port": 0, "socks-port": 0, "mixed-port": 0, "redir-port": 0, "tproxy-port": 0,
        "allow-lan": false, "find-process-mode": "off",
        "external-controller-unix": controller,
        "routing-mark": omavless_netguard::nft::CORE_MARK,
        "tun": {"enable": true, "stack": "system", "device": omavless_netguard::nft::TUN,
            "auto-route": true, "auto-detect-interface": true, "strict-route": true,
            "auto-redirect": false, "dns-hijack": ["any:53"],
            "disable-icmp-forwarding": true,
            "disable-system-dns": true, "omavless-dns-broker": true},
        "dns": {"enable": true, "ipv6": false, "use-hosts": false, "use-system-hosts": false,
            "enhanced-mode": "redir-host", "default-nameserver": ["1.1.1.1"],
            "nameserver": [RESOLVER], "proxy-server-nameserver": [RESOLVER]},
        "profile": {"store-selected": false, "store-fake-ip": false},
        "proxies": [proxy],
        "proxy-groups": [{"name": "PROXY", "type": "select", "proxies": [PROFILE]},
            {"name": "GLOBAL", "type": "select", "proxies": ["PROXY"], "default-selected": "PROXY"}],
        "rules": ["NETWORK,UDP,REJECT", "MATCH,PROXY"]
    });
    let bytes = serde_json::to_vec(&value).map_err(|_| PreparationError::Refused)?;
    if bytes.len() as u64 > MAX_CONFIG {
        return Err(PreparationError::Refused);
    }
    let policy = RenderedPolicy {
        version: PolicyVersion::RuleTcpVerifiedTlsDohV1,
        config: Sha256::digest(&bytes).into(),
    };
    Ok((bytes, policy))
}

impl NativeLifecycleHost {
    /// Inactive SOURCE entry. Does not run Mihomo -t, arm, start or populate the
    /// ordinary .config.candidate.yaml/readiness/profile fields. Real core
    /// validation and a protected-start consuming seam are still required.
    fn prepare_protected_candidate(
        &mut self,
        desired: &DesiredState,
    ) -> Result<(), PreparationError> {
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::mark(
            crate::protected_native_diagnostic::Cut::PackageVerify,
        );
        let pair = self
            .paths
            .managed_pair
            .as_ref()
            .ok_or(PreparationError::Unsupported)?;
        pair.verify().map_err(|_| PreparationError::Changed)?;
        if pair.core_path() != self.paths.core {
            return Err(PreparationError::Changed);
        }
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::mark(
            crate::protected_native_diagnostic::Cut::CoreCapture,
        );
        let core = HeldFile::capture(&self.paths.core, 0, 0o755, MAX_CORE)?;
        pair.verify().map_err(|_| PreparationError::Changed)?;
        self.prepare_bound_candidate(desired, core)?;
        let pair = self
            .paths
            .managed_pair
            .as_ref()
            .ok_or(PreparationError::Refused)?;
        let bound = self
            .protected_preparation
            .as_mut()
            .and_then(|p| p.bound.as_mut())
            .ok_or(PreparationError::Refused)?;
        bound.capacity.check()?;
        bound.package = Some(
            pair.capture_protected(&bound.core.file)
                .map_err(|_| PreparationError::Changed)?,
        );
        self.recheck_protected_candidate(desired)
    }

    fn prepare_bound_candidate(
        &mut self,
        desired: &DesiredState,
        core: HeldFile,
    ) -> Result<(), PreparationError> {
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::mark(
            crate::protected_native_diagnostic::Cut::BoundEligibility,
        );
        if self.protected_preparation.is_some()
            || self.core.is_some()
            || self.profile_id.is_some()
            || self.readiness.is_some()
            || self.previous_config.is_some()
            || self.active_install_attempted
            || !self.auxiliary.mutation_safe()
            || desired.validate().is_err()
            || !desired.connected
            || desired.mode != RoutingMode::Global
            || !private_directory(&self.paths.config_directory, self.uid)
        {
            return Err(PreparationError::Refused);
        }
        if !matches!(fs::symlink_metadata(self.paths.sys_class_net.join(omavless_netguard::nft::TUN)), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
        {
            return Err(PreparationError::Refused);
        }
        let capacity = PreparationCapacity::reserve()?;
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::mark(
            crate::protected_native_diagnostic::Cut::ProfileParse,
        );
        let store = read_private_utf8(&self.paths.store, self.uid)
            .map_err(|_| PreparationError::Refused)?;
        let store_digest = Sha256::digest(store.as_bytes()).into();
        let mut profiles = parse_private_store(&store)
            .map_err(|_| PreparationError::Refused)?
            .into_profile_probe_profiles(Some(&desired.profile_id))
            .map_err(|_| PreparationError::Refused)?;
        if profiles.len() != 1 {
            return Err(PreparationError::Refused);
        }
        let (_, profile) = profiles.pop().ok_or(PreparationError::Refused)?;
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::mark(
            crate::protected_native_diagnostic::Cut::PolicyRender,
        );
        let (bytes, policy) = render_bound(profile, &self.paths.controller_socket)?;
        let path = self.paths.config_directory.join(STAGING);
        // Consume before create/write/sync. Errors are not retry authorization.
        self.protected_preparation = Some(Preparation {
            bound: None,
            admitted: false,
            started: false,
        });
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(&path)
            .map_err(|_| PreparationError::OutcomeUnknown)?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| PreparationError::OutcomeUnknown)?;
        File::open(&self.paths.config_directory)
            .and_then(|f| f.sync_all())
            .map_err(|_| PreparationError::OutcomeUnknown)?;
        let config = HeldFile::capture(&path, self.uid, 0o600, MAX_CONFIG)?;
        if config.digest != <[u8; 32]>::from(Sha256::digest(&bytes)) {
            return Err(PreparationError::Changed);
        }
        // Ordinary current paths alias data and config. Publish the owned child
        // directory BEFORE capturing data's original nlink, never rebase it.
        let scratch = interval::Scratch::prepare(&self.paths.config_directory, self.uid)
            .map_err(|_| PreparationError::OutcomeUnknown)?;
        capacity.check()?;
        let bound = Bound {
            desired: desired.clone(),
            store_digest,
            core,
            config,
            data: HeldDirectory::capture(&self.paths.data_directory, self.uid)?,
            scratch: Some(scratch),
            policy,
            package: None,
            capacity,
        };
        self.protected_preparation
            .as_mut()
            .ok_or(PreparationError::Refused)?
            .bound = Some(bound);
        self.recheck_protected_candidate(desired)
    }

    fn recheck_protected_candidate(&self, desired: &DesiredState) -> Result<(), PreparationError> {
        let bound = self
            .protected_preparation
            .as_ref()
            .and_then(|p| p.bound.as_ref())
            .ok_or(PreparationError::Refused)?;
        if &bound.desired != desired
            || self.core.is_some()
            || self.profile_id.is_some()
            || self.readiness.is_some()
            || !private_directory(&self.paths.config_directory, self.uid)
            || !self.auxiliary.mutation_safe()
            || !matches!(fs::symlink_metadata(self.paths.sys_class_net.join(omavless_netguard::nft::TUN)), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
        {
            return Err(PreparationError::Changed);
        }
        let store = read_private_utf8(&self.paths.store, self.uid)
            .map_err(|_| PreparationError::Changed)?;
        if <[u8; 32]>::from(Sha256::digest(store.as_bytes())) != bound.store_digest {
            return Err(PreparationError::Changed);
        }
        bound.core.recheck(&self.paths.core)?;
        if bound.policy.config != bound.config.digest
            || bound.policy.version != PolicyVersion::RuleTcpVerifiedTlsDohV1
        {
            return Err(PreparationError::Changed);
        }
        bound.capacity.check()?;
        if let Some(package) = &bound.package {
            package
                .recheck(
                    self.paths
                        .managed_pair
                        .as_ref()
                        .ok_or(PreparationError::Changed)?,
                    &bound.core.file,
                )
                .map_err(|_| PreparationError::Changed)?;
        }
        bound.data.recheck(&self.paths.data_directory)?;
        bound
            .scratch
            .as_ref()
            .ok_or(PreparationError::Changed)?
            .recheck()
            .map_err(|_| PreparationError::Changed)?;
        bound
            .config
            .recheck(&self.paths.config_directory.join(STAGING))
    }

    /// Private post-prepare / pre-reservation-and-Arm consuming seam. The
    /// exact package/config bytes alone never admit: original validation and
    /// same-owner postchecks still precede private coverage issuance.
    fn admit_prepared_protection(
        &mut self,
        desired: &DesiredState,
    ) -> Result<ArmAdmission, PreparationError> {
        self.recheck_protected_candidate(desired)?;
        approved_policy_decision()?;
        let preparation = self
            .protected_preparation
            .as_mut()
            .ok_or(PreparationError::Refused)?;
        if preparation.admitted {
            return Err(PreparationError::Refused);
        }
        // Refuse BEFORE executing a real validator until its network behavior
        // and runtime coverage for this exact core/policy have been accepted.
        let pair = self
            .paths
            .managed_pair
            .as_ref()
            .ok_or(PreparationError::Refused)?;
        let candidate = preparation
            .bound
            .as_ref()
            .ok_or(PreparationError::Refused)?;
        candidate
            .package
            .as_ref()
            .ok_or(PreparationError::Refused)?
            .recheck(pair, &candidate.core.file)
            .map_err(|_| PreparationError::Changed)?;
        preparation.admitted = true;
        let mut acquired = AcquiredBound(Some(
            preparation.bound.take().ok_or(PreparationError::Refused)?,
        ));
        let staged = self.paths.config_directory.join(STAGING);
        let deadline = Instant::now() + VALIDATION_TIMEOUT;
        let child = std::process::Command::new(&self.paths.core)
            .env_clear()
            .env("LANG", "C")
            .args(["-t", "-d"])
            .arg(&self.paths.data_directory)
            .arg("-f")
            .arg(&staged)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|_| PreparationError::OutcomeUnknown)?;
        // No fallible operation between the reported child and its whole owner.
        let original = validation::Validation::new(child, acquired.0.take().expect("held bound"));
        let bound = original
            .complete(
                deadline,
                Instant::now,
                || std::thread::sleep(Duration::from_millis(10)),
                |bound| {
                    bound.capacity.check().map_err(|_| ())?;
                    bound
                        .package
                        .as_ref()
                        .ok_or(())?
                        .recheck(pair, &bound.core.file)
                        .map_err(|_| ())?;
                    bound.core.recheck(&self.paths.core).map_err(|_| ())?;
                    bound
                        .data
                        .recheck(&self.paths.data_directory)
                        .map_err(|_| ())?;
                    bound.scratch.as_ref().ok_or(())?.recheck()?;
                    bound.config.recheck(&staged).map_err(|_| ())
                },
            )
            .map_err(|_| PreparationError::OutcomeUnknown)?;
        self.protected_preparation
            .as_mut()
            .ok_or(PreparationError::Refused)?
            .bound = Some(bound);
        self.recheck_protected_candidate(desired)?;
        let bound = self
            .protected_preparation
            .as_ref()
            .and_then(|p| p.bound.as_ref())
            .ok_or(PreparationError::Refused)?;
        let coverage = issue_coverage(ValidatedPreparation { bound, pair })?;
        let admission = ArmAdmission {
            desired: desired.clone(),
            config: bound.config.digest,
            core: bound.core.digest,
            coverage,
        };
        Ok(admission)
    }
}

#[cfg(test)]
mod tests;

impl NativeLifecycleHost {
    pub(super) fn protected_eligibility(&self) -> Result<(), HostStepError> {
        if self.protected_preparation.is_some()
            || self.core.is_some()
            || !self.auxiliary.mutation_safe()
        {
            return Err(HostStepError::Prepare);
        }
        let pair = self
            .paths
            .managed_pair
            .as_ref()
            .ok_or(HostStepError::Prepare)?;
        if pair.core_path() != self.paths.core {
            return Err(HostStepError::Prepare);
        }
        pair.verify_protected()
    }
}

impl crate::lifecycle::protected_candidate::ProtectedHost for NativeLifecycleHost {
    type Admission = ArmAdmission;
    type Interval = interval::Interval;
    fn begin_interval(&mut self, desired: &DesiredState) -> Result<Self::Interval, HostStepError> {
        self.recheck_interval(desired)?;
        let scratch = self
            .protected_preparation
            .as_mut()
            .and_then(|p| p.bound.as_mut())
            .and_then(|b| b.scratch.take())
            .ok_or(HostStepError::Observation)?;
        interval::Interval::begin(scratch, self.uid).map_err(|_| HostStepError::Observation)
    }
    fn complete_interval(&mut self, interval: &mut Self::Interval) -> Result<(), HostStepError> {
        interval.complete().map_err(|_| HostStepError::Observation)
    }
    fn recheck_interval(&mut self, desired: &DesiredState) -> Result<(), HostStepError> {
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::post_mark(
            crate::protected_native_diagnostic::PostGuard::IntervalState,
        );
        let preparation = self
            .protected_preparation
            .as_ref()
            .ok_or(HostStepError::Observation)?;
        let bound = preparation
            .bound
            .as_ref()
            .ok_or(HostStepError::Observation)?;
        if !preparation.admitted
            || !preparation.started
            || &bound.desired != desired
            || self.core.is_none()
            || self.tun_identity.is_none()
            || !private_directory(&self.paths.config_directory, self.uid)
            || !self.auxiliary.mutation_safe()
        {
            return Err(HostStepError::Observation);
        }
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::post_mark(
            crate::protected_native_diagnostic::PostGuard::IntervalPair,
        );
        self.paths
            .managed_pair
            .as_ref()
            .ok_or(HostStepError::Observation)?
            .verify_protected()?;
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::post_mark(
            crate::protected_native_diagnostic::PostGuard::IntervalPackage,
        );
        bound
            .package
            .as_ref()
            .ok_or(HostStepError::Observation)?
            .recheck(
                self.paths
                    .managed_pair
                    .as_ref()
                    .ok_or(HostStepError::Observation)?,
                &bound.core.file,
            )?;
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::post_mark(
            crate::protected_native_diagnostic::PostGuard::IntervalPolicy,
        );
        if bound.policy.version != PolicyVersion::RuleTcpVerifiedTlsDohV1
            || bound.policy.config != bound.config.digest
        {
            return Err(HostStepError::Observation);
        }
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::post_mark(
            crate::protected_native_diagnostic::PostGuard::IntervalStoreRead,
        );
        let store = read_private_utf8(&self.paths.store, self.uid)
            .map_err(|_| HostStepError::Observation)?;
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::post_mark(
            crate::protected_native_diagnostic::PostGuard::IntervalStoreDigest,
        );
        if <[u8; 32]>::from(Sha256::digest(store.as_bytes())) != bound.store_digest {
            return Err(HostStepError::Observation);
        }
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::post_mark(
            crate::protected_native_diagnostic::PostGuard::IntervalCoreFile,
        );
        bound
            .core
            .recheck(&self.paths.core)
            .map_err(|_| HostStepError::Observation)?;
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::post_mark(
            crate::protected_native_diagnostic::PostGuard::IntervalConfigFile,
        );
        bound
            .config
            .recheck(&self.paths.config_directory.join(STAGING))
            .map_err(|_| HostStepError::Observation)?;
        // Runtime may legitimately create its cache. Retain original directory
        // identity/ownership, not pre-start size or timestamps as a fake proof.
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::post_mark(
            crate::protected_native_diagnostic::PostGuard::IntervalDataMetadata,
        );
        for current in [
            bound.data.file.metadata(),
            fs::symlink_metadata(&self.paths.data_directory),
        ] {
            let current = current.map_err(|_| HostStepError::Observation)?;
            let original = &bound.data.metadata;
            #[cfg(all(test, feature = "netguard-native-scenario"))]
            crate::protected_native_diagnostic::post_mark(
                crate::protected_native_diagnostic::PostGuard::IntervalDataIdentity,
            );
            if DirectoryIdentity::of(&current) != DirectoryIdentity::of(original) {
                #[cfg(all(test, feature = "netguard-native-scenario"))]
                crate::protected_native_diagnostic::post_identity_rejected(
                    DirectoryIdentity::of(&current)
                        .first_difference(DirectoryIdentity::of(original)),
                );
                return Err(HostStepError::Observation);
            }
        }
        Ok(())
    }
    fn prepare_admitted(&mut self, desired: &DesiredState) -> Result<ArmAdmission, HostStepError> {
        self.prepare_protected_candidate(desired)
            .map_err(|_| HostStepError::Prepare)?;
        self.admit_prepared_protection(desired)
            .map_err(|_| HostStepError::Prepare)
    }
    fn recheck_admission(&self, admission: &ArmAdmission) -> Result<(), HostStepError> {
        self.recheck_protected_candidate(&admission.desired)
            .map_err(|_| HostStepError::Prepare)?;
        self.paths
            .managed_pair
            .as_ref()
            .ok_or(HostStepError::Prepare)?
            .verify_protected()?;
        if self
            .paths
            .managed_pair
            .as_ref()
            .ok_or(HostStepError::Prepare)?
            .protected_identity()
            != admission.coverage.pair
        {
            return Err(HostStepError::Prepare);
        }
        let preparation = self
            .protected_preparation
            .as_ref()
            .ok_or(HostStepError::Prepare)?;
        let bound = preparation.bound.as_ref().ok_or(HostStepError::Prepare)?;
        if !preparation.admitted
            || preparation.started
            || bound.config.digest != admission.config
            || bound.core.digest != admission.core
            || admission.coverage.core != admission.core
            || admission.coverage.config != bound.config.digest
            || admission.coverage.policy != bound.policy.version
            || bound.policy.config != bound.config.digest
        {
            return Err(HostStepError::Prepare);
        }
        Ok(())
    }
    fn start_admitted(&mut self, admission: ArmAdmission) -> Result<(), HostStepError> {
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::mark(
            crate::protected_native_diagnostic::Cut::StartAdmission,
        );
        self.recheck_admission(&admission)?;
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::mark(
            crate::protected_native_diagnostic::Cut::StartTunAbsent,
        );
        if !self.ping_slot.revoke() || self.managed_tuns()? != 0 {
            return Err(HostStepError::Start);
        }
        self.protected_preparation
            .as_mut()
            .ok_or(HostStepError::Start)?
            .started = true;
        self.profile_id = Some(admission.desired.profile_id);
        self.readiness = Some(ConfigReadiness::protected_full(PROFILE.to_owned()));
        self.tun_identity = None;
        self.remove_controller()?;
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::mark(
            crate::protected_native_diagnostic::Cut::CoreSpawn,
        );
        let core = OwnedCore::spawn_protected(
            &self.paths.core,
            &self.paths.data_directory,
            &self.paths.config_directory.join(STAGING),
            &self.paths.controller_socket,
        )
        .map_err(|_| HostStepError::Start)?;
        // Store the returned original BEFORE every readiness/panic cut.
        self.core = Some(core);
        let core = self.core.as_mut().ok_or(HostStepError::Start)?;
        self.core_diagnostics = Some(core.diagnostic_reader());
        let expected = self.readiness.as_ref().ok_or(HostStepError::Start)?;
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::mark(
            crate::protected_native_diagnostic::Cut::ConfiguredWait,
        );
        core.wait_configured(expected.startup_timeout(), expected)
            .map_err(|_| HostStepError::Start)?;
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::mark(
            crate::protected_native_diagnostic::Cut::ConfiguredPid,
        );
        let pid = core.pid().ok_or(HostStepError::Start)?;
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::mark(
            crate::protected_native_diagnostic::Cut::SecureOwned,
        );
        if !crate::controller_permissions::secure_owned(
            &self.paths.controller_socket,
            pid,
            self.uid,
        ) || !{
            #[cfg(all(test, feature = "netguard-native-scenario"))]
            crate::protected_native_diagnostic::mark(
                crate::protected_native_diagnostic::Cut::CoreRunning,
            );
            core.running().map_err(|_| HostStepError::Start)?
        } {
            return Err(HostStepError::Start);
        }
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::mark(crate::protected_native_diagnostic::Cut::TunCheck);
        if !self.verify_tun(pid)? {
            return Err(HostStepError::Start);
        }
        #[cfg(all(test, feature = "netguard-native-scenario"))]
        crate::protected_native_diagnostic::mark(
            crate::protected_native_diagnostic::Cut::StartComplete,
        );
        Ok(())
    }
    fn commit_protected(&mut self) -> Result<(), HostStepError> {
        let preparation = self
            .protected_preparation
            .as_ref()
            .ok_or(HostStepError::Commit)?;
        let bound = preparation.bound.as_ref().ok_or(HostStepError::Commit)?;
        if !preparation.started || self.core.is_none() || self.tun_identity.is_none() {
            return Err(HostStepError::Commit);
        }
        bound
            .core
            .recheck(&self.paths.core)
            .map_err(|_| HostStepError::Commit)?;
        bound
            .config
            .recheck(&self.paths.config_directory.join(STAGING))
            .map_err(|_| HostStepError::Commit)?;
        // Keep the exact held launch config. Never install/overwrite ordinary
        // config.yaml or delete this record while the protected child is live.
        Ok(())
    }
    fn discard_protected(&mut self) -> Result<(), HostStepError> {
        // This fixture retains its staged record until host retirement, even
        // after a known stop. No unlink/adoption/retry authority is introduced.
        if self.core.is_some() || self.managed_tuns()? != 0 || !self.auxiliary.mutation_safe() {
            return Err(HostStepError::Cleanup);
        }
        Ok(())
    }
}
