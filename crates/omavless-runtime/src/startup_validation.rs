// SPDX-License-Identifier: MIT
//! Fixed-purpose login preflight; never stages or stops an active core.
use crate::desired::DesiredState;
use crate::lifecycle::HostStepError;
use crate::native_host::NativeHostPaths;
use nix::fcntl::{FcntlArg, OFlag, fcntl};
use omavless_domain::private_store::parse_private_store;
use omavless_store::read_private_utf8;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn tun_capabilities(getcap: &Path, core: &Path) -> bool {
    let Ok(mut child) = Command::new(getcap)
        .arg(core)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let mut stdout = child.stdout.take().expect("piped stdout");
    let result = (|| {
        fcntl(&stdout, FcntlArg::F_SETFL(OFlag::O_NONBLOCK)).ok()?;
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut output = Vec::new();
        loop {
            if Instant::now() >= deadline {
                return None;
            }
            let mut chunk = [0u8; 1024];
            match stdout.read(&mut chunk) {
                Ok(0) => break,
                Ok(count) => {
                    output.extend_from_slice(&chunk[..count]);
                    if output.len() > 8192 {
                        return None;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5))
                }
                Err(_) => return None,
            }
        }
        loop {
            match child.try_wait().ok()? {
                Some(status) if status.success() => break,
                Some(_) => return None,
                None if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
                None => return None,
            }
        }
        let text = std::str::from_utf8(&output).ok()?;
        let (_, caps) = text.trim().rsplit_once(' ')?;
        Some(
            ["cap_net_admin", "cap_net_raw", "cap_net_bind_service"]
                .iter()
                .all(|cap| {
                    caps.split('=')
                        .next()
                        .unwrap_or("")
                        .split(',')
                        .any(|item| item == *cap)
                })
                && caps
                    .split('=')
                    .nth(1)
                    .is_some_and(|flags| flags.contains('e') && flags.contains('p')),
        )
    })()
    .unwrap_or(false);
    let _ = child.kill();
    let _ = child.wait();
    result
}

pub(crate) fn validate(
    paths: &NativeHostPaths,
    uid: u32,
    desired: &DesiredState,
) -> Result<(), HostStepError> {
    if !crate::native_host::private_directory(&paths.runtime_directory, uid) {
        return Err(HostStepError::Prepare);
    }
    if !tun_capabilities(Path::new("/usr/bin/getcap"), &paths.core) {
        return Err(HostStepError::Prepare);
    }
    let store = read_private_utf8(&paths.store, uid).map_err(|_| HostStepError::Prepare)?;
    let store = parse_private_store(&store).map_err(|_| HostStepError::Prepare)?;
    let template = read_private_utf8(&paths.template, uid).map_err(|_| HostStepError::Prepare)?;
    if template.len() > omavless_domain::config::MAX_TEMPLATE_BYTES {
        return Err(HostStepError::Prepare);
    }
    let controller = paths
        .controller_socket
        .to_str()
        .ok_or(HostStepError::Prepare)?;
    let config = store
        .prepare_config_mode(
            &desired.profile_id,
            &template,
            controller,
            desired.mode.as_str(),
        )
        .map_err(|_| HostStepError::Prepare)?;
    let path = paths.runtime_directory.join(".startup-check.yaml");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|_| HostStepError::Prepare)?;
    let result = file
        .write_all(config.as_bytes())
        .map_err(|_| HostStepError::Prepare)
        .and_then(|()| {
            drop(file);
            omavless_mihomo::validate_config(
                &paths.core,
                &paths.data_directory,
                &path,
                Duration::from_secs(3),
            )
            .map(|_| ())
            .map_err(|_| HostStepError::Prepare)
        });
    fs::remove_file(&path).map_err(|_| HostStepError::Cleanup)?;
    result
}
