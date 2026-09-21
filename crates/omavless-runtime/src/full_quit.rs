// SPDX-License-Identifier: MIT

//! Explicit full application shutdown, completed outside the daemon being shut
//! down. This is not panel close, plugin unload, a generic service manager, or
//! an emergency process-kill interface. The daemon first owns disconnect and
//! graceful shutdown; only after its owner lock is released and an empty host
//! is verified may this fixed CLI disable its unit and finally hide the plugin.

use crate::desired::{DesiredPaths, read_desired_snapshot};
use crate::production_observation::{
    ProductionOwnershipObserver, RUST_SERVICE, SERVICE_QUERY_TIMEOUT, bounded_fixed_query,
    cutover_service_installation, service_state_with_timeout,
};
use crate::{OwnerLock, RuntimePaths};
use nix::sys::socket::{getsockopt, sockopt::PeerCredentials};
use nix::unistd::Uid;
use serde_json::{Value, json};
use std::fmt;
use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

pub mod removal;

const SYSTEMCTL: &str = "/usr/bin/systemctl";
const OMARCHY: &str = "/usr/bin/omarchy";
const PLUGIN: &str = "kdk.omavless";
const UNIT: &str = "/usr/lib/systemd/user/omavless-runtime.service";
// Existing provider jobs can take 25 seconds to drain after cancellation.
// Do not misclassify accepted shutdown as failed while that bounded join runs.
const STOP_WAIT: Duration = Duration::from_secs(60);
const QUIT_WAIT: Duration = Duration::from_secs(120);

/// Fixed credential-free fallback messages. Child output and private protocol
/// errors are never embedded in this type or its Display implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FullQuitError {
    InvalidArgument,
    PreconditionsFailed,
    RequestFailed,
    ShutdownNotVerified,
    RuntimeDisableFailed,
    PluginDisableFailed,
}

impl fmt::Display for FullQuitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidArgument => "OmaVLESS quit arguments are invalid",
            Self::PreconditionsFailed => "OmaVLESS quit host preconditions failed",
            Self::RequestFailed => "OmaVLESS quit was not confirmed; refresh runtime state",
            Self::ShutdownNotVerified => {
                "OmaVLESS shutdown could not be verified; plugin remains enabled"
            }
            Self::RuntimeDisableFailed => {
                "OmaVLESS stopped but runtime disable was not verified; plugin remains enabled"
            }
            Self::PluginDisableFailed => "OmaVLESS stopped but plugin disable was not confirmed",
        })
    }
}
impl std::error::Error for FullQuitError {}
type Result<T> = std::result::Result<T, FullQuitError>;

// The trait is private and fixed-purpose. No production caller supplies a
// command, executable, unit, path, timeout, or shell fragment.
trait Host {
    fn preflight(&mut self) -> Result<()>;
    fn request_shutdown(&mut self, instance: &str, revision: u64, operation: &str) -> Result<()>;
    fn wait_and_lock(&mut self) -> Result<()>;
    fn verify_stopped(&mut self) -> Result<()>;
    fn disable_runtime(&mut self) -> Result<()>;
    fn verify_runtime_disabled(&mut self) -> Result<()>;
    fn disable_plugin(&mut self) -> Result<()>;
    fn verify_plugin_disabled(&mut self) -> Result<()>;
}

fn arguments_valid(instance: &str, revision: u64, operation: &str) -> bool {
    let ascii = |text: &str, maximum: usize| {
        !text.is_empty()
            && text.len() <= maximum
            && text.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
    };
    ascii(
        instance,
        crate::long_operation_protocol::MAX_INSTANCE_ID_BYTES,
    ) && ascii(operation, omavless_control_protocol::MAX_ID_LENGTH)
        && revision <= omavless_control_protocol::MAX_REVISION
}

fn run_host(host: &mut impl Host, instance: &str, revision: u64, operation: &str) -> Result<()> {
    if !arguments_valid(instance, revision, operation) {
        return Err(FullQuitError::InvalidArgument);
    }
    host.preflight()?;
    finish_runtime(host, instance, revision, operation)?;
    host.disable_plugin()?;
    host.verify_plugin_disabled()
}

fn finish_runtime(
    host: &mut impl Host,
    instance: &str,
    revision: u64,
    operation: &str,
) -> Result<()> {
    host.request_shutdown(instance, revision, operation)?;
    host.wait_and_lock()?;
    host.verify_stopped()?;
    host.disable_runtime()?;
    host.verify_runtime_disabled()?;
    // Repeat the host proof immediately before the only UI-hiding effect.
    host.verify_stopped()
}

/// An explicitly confirmed Settings action or fixed semantic CLI invokes this
/// with its observed instance/revision and one stable operation ID. Transport
/// uncertainty never triggers a new operation or a fallback process kill.
pub fn run(instance: &str, revision: u64, operation: &str) -> Result<()> {
    let mut host = InstalledHost {
        paths: RuntimePaths::current().map_err(|_| FullQuitError::PreconditionsFailed)?,
        stream: None,
        owner_lock: None,
    };
    run_host(&mut host, instance, revision, operation)
}

struct InstalledHost {
    paths: RuntimePaths,
    stream: Option<UnixStream>,
    // Held through unit disable and plugin disable. A concurrent daemon start
    // cannot become another runtime owner between the empty-host checks.
    owner_lock: Option<OwnerLock>,
}

fn installation(text: &str, require_disabled: bool) -> bool {
    let Some(object) = installation_fields(text) else {
        return false;
    };
    (object[0] == "disabled" || (!require_disabled && object[0] == "enabled"))
        && object[1] == UNIT
        && object[2].is_empty()
        && object[3] == "no"
}

fn installation_fields(text: &str) -> Option<[&str; 4]> {
    let mut result = [None; 4];
    let fields = [
        "UnitFileState",
        "FragmentPath",
        "DropInPaths",
        "NeedDaemonReload",
    ];
    for line in text.lines() {
        let (key, value) = line.split_once('=')?;
        let index = fields.iter().position(|field| *field == key)?;
        if result[index].replace(value).is_some() {
            return None;
        }
    }
    Some([result[0]?, result[1]?, result[2]?, result[3]?])
}

fn installed_unit(require_disabled: bool) -> bool {
    cutover_service_installation(Path::new(SYSTEMCTL), RUST_SERVICE)
        .is_ok_and(|text| installation(&text, require_disabled))
}

fn omarchy_command(arguments: &[&str]) -> std::result::Result<String, ()> {
    let mut command = Command::new(OMARCHY);
    command
        .args(arguments)
        .env("PATH", "/usr/bin:/usr/share/omarchy/bin");
    bounded_fixed_query(command, Duration::from_secs(10)).map_err(|_| ())
}

fn plugin_enabled(text: &str) -> Option<bool> {
    // The fixed trusted shell projection is bounded by bounded_fixed_query.
    // Retain only the exact plugin's boolean; no names/paths reach CLI output.
    let value: Value = serde_json::from_str(text).ok()?;
    let rows = value.as_array()?;
    if rows.len() > 1024 {
        return None;
    }
    let mut matches = rows.iter().filter(|row| row["id"] == PLUGIN);
    let enabled = matches.next()?["enabled"].as_bool()?;
    matches.next().is_none().then_some(enabled)
}

fn stopped_unit() -> bool {
    let mut command = Command::new(SYSTEMCTL);
    command.args([
        "--user",
        "show",
        RUST_SERVICE,
        "--no-pager",
        "--property=ActiveState",
        "--property=MainPID",
    ]);
    bounded_fixed_query(command, SERVICE_QUERY_TIMEOUT).is_ok_and(|text| {
        let mut lines = text.lines().collect::<Vec<_>>();
        lines.sort_unstable();
        lines == ["ActiveState=inactive", "MainPID=0"]
    })
}

impl InstalledHost {
    fn authenticate(&mut self, require_enabled: bool) -> Result<()> {
        let error = FullQuitError::PreconditionsFailed;
        crate::cutover_activation::packaged_identity().map_err(|_| error)?;
        crate::cutover_activation::environment::with_current(|| ()).map_err(|_| error)?;
        if !installed_unit(false)
            || !fs::symlink_metadata(OMARCHY).is_ok_and(|metadata| {
                metadata.is_file()
                    && metadata.uid() == 0
                    && metadata.mode() & 0o022 == 0
                    && metadata.mode() & 0o111 != 0
            })
            || (require_enabled
                && omarchy_command(&["plugin", "list", "--json"])
                    .ok()
                    .and_then(|text| plugin_enabled(&text))
                    != Some(true))
        {
            return Err(error);
        }
        let uid = Uid::current().as_raw();
        crate::validate_client_directory(&self.paths.directory, uid).map_err(|_| error)?;
        let socket = fs::symlink_metadata(&self.paths.socket).map_err(|_| error)?;
        if !socket.file_type().is_socket() || socket.uid() != uid || socket.mode() & 0o7777 != 0o600
        {
            return Err(error);
        }
        let stream = UnixStream::connect(&self.paths.socket).map_err(|_| error)?;
        let peer = getsockopt(&stream, PeerCredentials).map_err(|_| error)?;
        let unit =
            service_state_with_timeout(Path::new(SYSTEMCTL), RUST_SERVICE, SERVICE_QUERY_TIMEOUT)
                .map_err(|_| error)?;
        if peer.uid() != uid
            || peer.pid() <= 0
            || unit.main_pid != peer.pid() as u32
            || !unit.active
            || unit.exit_status != 0
            || unit.result != "success"
        {
            return Err(error);
        }
        let peer_binary = fs::metadata(format!("/proc/{}/exe", peer.pid())).map_err(|_| error)?;
        let current = fs::metadata("/proc/self/exe").map_err(|_| error)?;
        if peer_binary.dev() != current.dev() || peer_binary.ino() != current.ino() {
            return Err(error);
        }
        // This exact authenticated stream is retained for the mutation: do not
        // reconnect to a potentially replaced socket after the identity check.
        self.stream = Some(stream);
        Ok(())
    }
}

impl Host for InstalledHost {
    fn preflight(&mut self) -> Result<()> {
        self.authenticate(true)
    }

    fn request_shutdown(&mut self, instance: &str, revision: u64, operation: &str) -> Result<()> {
        let stream = self
            .stream
            .take()
            .ok_or(FullQuitError::PreconditionsFailed)?;
        let response = crate::call_stream_with_timeout(
            stream,
            Uid::current().as_raw(),
            "runtime.quit",
            json!({"instanceId":instance,"expectedRevision":revision,"operationId":operation}),
            QUIT_WAIT,
        )
        .map_err(|_| FullQuitError::RequestFailed)?;
        let result = &response["result"];
        if response["ok"] != true
            || result["schemaVersion"] != 1
            || result["instanceId"] != instance
            || result["operationId"] != operation
            || result["disconnected"] != true
            || result["runtimeStopping"] != true
        {
            return Err(FullQuitError::RequestFailed);
        }
        Ok(())
    }

    fn wait_and_lock(&mut self) -> Result<()> {
        let end = Instant::now() + STOP_WAIT;
        while Instant::now() < end {
            if stopped_unit() {
                crate::validate_client_directory(&self.paths.directory, Uid::current().as_raw())
                    .map_err(|_| FullQuitError::ShutdownNotVerified)?;
                match OwnerLock::acquire(&self.paths.owner_lock, Uid::current().as_raw()) {
                    Ok(lock) => {
                        self.owner_lock = Some(lock);
                        return Ok(());
                    }
                    Err(crate::RuntimeError::AlreadyRunning) => {}
                    Err(_) => return Err(FullQuitError::ShutdownNotVerified),
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err(FullQuitError::ShutdownNotVerified)
    }

    fn verify_stopped(&mut self) -> Result<()> {
        let error = FullQuitError::ShutdownNotVerified;
        if self.owner_lock.is_none() || !stopped_unit() {
            return Err(error);
        }
        crate::cutover_activation::environment::with_current(|| ()).map_err(|_| error)?;
        let desired = read_desired_snapshot(
            &DesiredPaths::current().map_err(|_| error)?,
            Uid::current().as_raw(),
        )
        .map_err(|_| error)?;
        if desired.connected {
            return Err(error);
        }
        ProductionOwnershipObserver::current()
            .map_err(|_| error)?
            .verify_native_empty()
            .map_err(|_| error)
    }

    fn disable_runtime(&mut self) -> Result<()> {
        if self.owner_lock.is_none() || !installed_unit(false) {
            return Err(FullQuitError::RuntimeDisableFailed);
        }
        let mut command = Command::new(SYSTEMCTL);
        // No --now/stop: the runtime already completed its semantic shutdown.
        command.args(["--user", "disable", RUST_SERVICE]);
        bounded_fixed_query(command, SERVICE_QUERY_TIMEOUT)
            .map(|_| ())
            .map_err(|_| FullQuitError::RuntimeDisableFailed)
    }

    fn verify_runtime_disabled(&mut self) -> Result<()> {
        installed_unit(true)
            .then_some(())
            .ok_or(FullQuitError::RuntimeDisableFailed)
    }

    fn disable_plugin(&mut self) -> Result<()> {
        if self.owner_lock.is_none() {
            return Err(FullQuitError::ShutdownNotVerified);
        }
        omarchy_command(&["plugin", "disable", PLUGIN])
            .map(|_| ())
            .map_err(|_| FullQuitError::PluginDisableFailed)
    }

    fn verify_plugin_disabled(&mut self) -> Result<()> {
        (omarchy_command(&["plugin", "list", "--json"])
            .ok()
            .and_then(|text| plugin_enabled(&text))
            == Some(false))
        .then_some(())
        .ok_or(FullQuitError::PluginDisableFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        calls: Vec<&'static str>,
        fail_at: Option<usize>,
    }
    impl Fake {
        fn step(&mut self, name: &'static str) -> Result<()> {
            self.calls.push(name);
            if self.fail_at == Some(self.calls.len() - 1) {
                Err(FullQuitError::ShutdownNotVerified)
            } else {
                Ok(())
            }
        }
    }
    impl Host for Fake {
        fn preflight(&mut self) -> Result<()> {
            self.step("preflight")
        }
        fn request_shutdown(
            &mut self,
            instance: &str,
            revision: u64,
            operation: &str,
        ) -> Result<()> {
            assert_eq!(
                (instance, revision, operation),
                ("instance", 7, "operation")
            );
            self.step("request")
        }
        fn wait_and_lock(&mut self) -> Result<()> {
            self.step("lock")
        }
        fn verify_stopped(&mut self) -> Result<()> {
            self.step("empty")
        }
        fn disable_runtime(&mut self) -> Result<()> {
            self.step("disable-unit")
        }
        fn verify_runtime_disabled(&mut self) -> Result<()> {
            self.step("unit-disabled")
        }
        fn disable_plugin(&mut self) -> Result<()> {
            self.step("disable-plugin")
        }
        fn verify_plugin_disabled(&mut self) -> Result<()> {
            self.step("plugin-disabled")
        }
    }
    const STEPS: [&str; 9] = [
        "preflight",
        "request",
        "lock",
        "empty",
        "disable-unit",
        "unit-disabled",
        "empty",
        "disable-plugin",
        "plugin-disabled",
    ];

    #[test]
    fn full_quit_orders_verified_shutdown_before_plugin_hide() {
        let mut host = Fake {
            calls: vec![],
            fail_at: None,
        };
        run_host(&mut host, "instance", 7, "operation").unwrap();
        assert_eq!(host.calls, STEPS);
    }

    #[test]
    fn every_failure_stops_before_all_later_effects() {
        for index in 0..STEPS.len() {
            let mut host = Fake {
                calls: vec![],
                fail_at: Some(index),
            };
            assert!(run_host(&mut host, "instance", 7, "operation").is_err());
            assert_eq!(host.calls, STEPS[..=index]);
            if index < 7 {
                assert!(!host.calls.contains(&"disable-plugin"));
            }
        }
    }

    #[test]
    fn invalid_metadata_has_no_host_effects() {
        for (instance, revision, operation) in [
            ("".to_owned(), 0, "op".to_owned()),
            ("x".repeat(129), 0, "op".to_owned()),
            ("a b".to_owned(), 0, "op".to_owned()),
            ("instance".to_owned(), u64::MAX, "op".to_owned()),
            ("instance".to_owned(), 0, "".to_owned()),
            ("instance".to_owned(), 0, "x".repeat(65)),
            ("instance".to_owned(), 0, "op\n".to_owned()),
            ("instance".to_owned(), 0, "пароль".to_owned()),
        ] {
            let mut host = Fake {
                calls: vec![],
                fail_at: None,
            };
            assert_eq!(
                run_host(&mut host, &instance, revision, &operation),
                Err(FullQuitError::InvalidArgument)
            );
            assert!(host.calls.is_empty());
        }
        assert!(arguments_valid(
            &"x".repeat(128),
            omavless_control_protocol::MAX_REVISION,
            &"x".repeat(64)
        ));
    }

    #[test]
    fn installation_rejects_override_reload_ambiguous_or_wrong_unit() {
        let valid = format!(
            "UnitFileState=enabled\nFragmentPath={UNIT}\nDropInPaths=\nNeedDaemonReload=no\n"
        );
        assert!(installation(&valid, false));
        assert!(!installation(&valid, true));
        assert!(installation(&valid.replace("enabled", "disabled"), true));
        for invalid in [
            valid.replace("enabled", "masked"),
            valid.replace(UNIT, "/tmp/unit"),
            valid.replace("DropInPaths=", "DropInPaths=/tmp/private"),
            valid.replace("Reload=no", "Reload=yes"),
            format!("{valid}UnitFileState=disabled\n"),
            format!("{valid}Other=value\n"),
            valid.replace("NeedDaemonReload=no\n", ""),
        ] {
            assert!(!installation(&invalid, false));
        }
    }

    #[test]
    fn shell_projection_requires_one_exact_boolean() {
        assert_eq!(
            plugin_enabled(r#"[{"id":"kdk.omavless","enabled":true,"name":"private"}]"#),
            Some(true)
        );
        assert_eq!(
            plugin_enabled(r#"[{"id":"kdk.omavless","enabled":false}]"#),
            Some(false)
        );
        for text in [
            "{}",
            "[]",
            "private malformed data",
            r#"[{"id":"other","enabled":true}]"#,
            r#"[{"id":"kdk.omavless","enabled":"false"}]"#,
            r#"[{"id":"kdk.omavless","enabled":false},{"id":"kdk.omavless","enabled":true}]"#,
        ] {
            assert_eq!(plugin_enabled(text), None);
        }
    }

    #[test]
    fn fixed_errors_are_bounded_and_have_no_private_input_carrier() {
        for error in [
            FullQuitError::InvalidArgument,
            FullQuitError::PreconditionsFailed,
            FullQuitError::RequestFailed,
            FullQuitError::ShutdownNotVerified,
            FullQuitError::RuntimeDisableFailed,
            FullQuitError::PluginDisableFailed,
        ] {
            let message = error.to_string();
            assert!(message.is_ascii() && message.len() <= 160);
            assert!(!message.contains('/'));
            assert!(!message.contains("private"));
        }
    }
}
