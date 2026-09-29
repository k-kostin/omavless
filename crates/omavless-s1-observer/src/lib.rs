// SPDX-License-Identifier: MIT

//! Separate, opt-in S1 host observer. The production runtime does not link this
//! crate, invoke the helper, or admit App proxy writes.

use omavless_runtime::app_proxy::{Snapshot, codec};
use std::fmt;

#[cfg(feature = "gio-observation")]
mod gio_host;

const MAGIC: &[u8; 8] = b"OMAS1OBS";
const MAX_FRAME: usize = 2 * 16 * 1024 + 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    UnsupportedSchema,
    UnsupportedBackend,
    InvalidValue,
    ManagerUnavailable,
    OwnerChanged,
    IdentityUnverified,
    IncompleteObservation,
    InvalidFrame,
}

/// The GIO and manager readings are observations only. No activation or
/// session provenance is established, so this cannot authorize host writes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    Unverified,
}

pub struct Observation {
    desktop: codec::DesktopSnapshot,
    manager: codec::EnvironmentSnapshot,
    provenance: Provenance,
}

impl fmt::Debug for Observation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("S1Observation([private], unverified)")
    }
}

impl Observation {
    /// Private observations are useful for a future runner's readback, but
    /// cannot be converted into a write capability by this crate.
    pub fn desktop(&self) -> &codec::DesktopSnapshot {
        &self.desktop
    }

    pub fn manager(&self) -> &codec::EnvironmentSnapshot {
        &self.manager
    }

    pub fn provenance(&self) -> Provenance {
        self.provenance
    }

    /// Private length-framed pipe payload. It is never a diagnostic or log.
    pub fn encode_private_frame(&self) -> Result<Vec<u8>, Error> {
        let desktop = self.desktop.encode().map_err(|_| Error::InvalidValue)?;
        let manager = self.manager.encode().map_err(|_| Error::InvalidValue)?;
        let first = desktop.bytes().ok_or(Error::IncompleteObservation)?;
        let second = manager.bytes().ok_or(Error::IncompleteObservation)?;
        let mut frame = Vec::with_capacity(17 + first.len() + second.len());
        frame.extend_from_slice(MAGIC);
        frame.push(1); // version
        frame.extend_from_slice(&(first.len() as u32).to_be_bytes());
        frame.extend_from_slice(&(second.len() as u32).to_be_bytes());
        frame.extend_from_slice(first);
        frame.extend_from_slice(second);
        if frame.len() > MAX_FRAME {
            return Err(Error::InvalidFrame);
        }
        Ok(frame)
    }

    /// Reject truncated, trailing and noncanonical private helper responses.
    pub fn decode_private_frame(frame: &[u8]) -> Result<Self, Error> {
        if frame.len() < 17 || frame.len() > MAX_FRAME || &frame[..8] != MAGIC || frame[8] != 1 {
            return Err(Error::InvalidFrame);
        }
        let first =
            u32::from_be_bytes(frame[9..13].try_into().map_err(|_| Error::InvalidFrame)?) as usize;
        let second =
            u32::from_be_bytes(frame[13..17].try_into().map_err(|_| Error::InvalidFrame)?) as usize;
        if first > 16 * 1024 || second > 16 * 1024 || frame.len() != 17 + first + second {
            return Err(Error::InvalidFrame);
        }
        let desktop =
            Snapshot::new(Some(frame[17..17 + first].to_vec())).map_err(|_| Error::InvalidFrame)?;
        let manager =
            Snapshot::new(Some(frame[17 + first..].to_vec())).map_err(|_| Error::InvalidFrame)?;
        let desktop = codec::DesktopSnapshot::decode(&desktop).map_err(|_| Error::InvalidFrame)?;
        let manager =
            codec::EnvironmentSnapshot::decode(&manager).map_err(|_| Error::InvalidFrame)?;
        if desktop.encode().map_err(|_| Error::InvalidFrame)?
            != Snapshot::new(Some(frame[17..17 + first].to_vec()))
                .map_err(|_| Error::InvalidFrame)?
            || manager.encode().map_err(|_| Error::InvalidFrame)?
                != Snapshot::new(Some(frame[17 + first..].to_vec()))
                    .map_err(|_| Error::InvalidFrame)?
        {
            return Err(Error::InvalidFrame);
        }
        Ok(Self {
            desktop,
            manager,
            provenance: Provenance::Unverified,
        })
    }

    /// Deliberately unavailable until a separate runtime admission proves the
    /// bus/session activation relationship and exact per-field write recovery.
    pub fn admit_writes(&self) -> Result<(), Error> {
        Err(Error::IdentityUnverified)
    }
}

#[cfg(feature = "gio-observation")]
pub fn observe_read_only() -> Result<Observation, Error> {
    gio_host::observe()
}

fn environment_name(key: codec::EnvironmentKey) -> &'static str {
    use codec::EnvironmentKey as Key;
    match key {
        Key::Http => "http_proxy",
        Key::HttpUpper => "HTTP_PROXY",
        Key::Https => "https_proxy",
        Key::HttpsUpper => "HTTPS_PROXY",
        Key::Ftp => "ftp_proxy",
        Key::FtpUpper => "FTP_PROXY",
        Key::All => "all_proxy",
        Key::AllUpper => "ALL_PROXY",
        Key::No => "no_proxy",
        Key::NoUpper => "NO_PROXY",
    }
}

/// The caller must pass a complete, successfully decoded manager Environment
/// property. Unrelated assignments are discarded before serialization.
pub fn project_manager_environment(
    assignments: &[String],
) -> Result<codec::EnvironmentSnapshot, Error> {
    if assignments.len() > 4096 {
        return Err(Error::IncompleteObservation);
    }
    let mut entries: Vec<_> = codec::EnvironmentKey::ALL
        .into_iter()
        .map(|key| codec::EnvironmentEntry {
            key,
            value: codec::EnvironmentValue::Absent,
        })
        .collect();
    for assignment in assignments {
        let (name, value) = match assignment.split_once('=') {
            Some(parts) => parts,
            None => {
                if codec::EnvironmentKey::ALL
                    .iter()
                    .any(|key| environment_name(*key) == assignment)
                {
                    return Err(Error::InvalidValue);
                }
                continue;
            }
        };
        if let Some(entry) = entries
            .iter_mut()
            .find(|entry| environment_name(entry.key) == name)
        {
            if !matches!(entry.value, codec::EnvironmentValue::Absent) {
                return Err(Error::InvalidValue);
            }
            entry.value = codec::EnvironmentValue::Present(value.to_owned());
        }
    }
    codec::EnvironmentSnapshot::capture(entries).map_err(|_| Error::InvalidValue)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_assignments_preserve_empty_equals_and_case() {
        let snapshot = project_manager_environment(&[
            "http_proxy=".to_owned(),
            "HTTP_PROXY=http://a=b".to_owned(),
            "OTHER_PRIVATE=discard".to_owned(),
        ])
        .unwrap();
        assert!(matches!(
            &snapshot.entries()[0].value,
            codec::EnvironmentValue::Present(value) if value.is_empty()
        ));
        assert!(matches!(
            &snapshot.entries()[1].value,
            codec::EnvironmentValue::Present(value) if value == "http://a=b"
        ));
        assert!(matches!(
            &snapshot.entries()[2].value,
            codec::EnvironmentValue::Absent
        ));
    }

    #[test]
    fn duplicate_or_malformed_selected_assignment_refuses() {
        assert_eq!(
            project_manager_environment(&["http_proxy".to_owned()]).err(),
            Some(Error::InvalidValue)
        );
        assert_eq!(
            project_manager_environment(&[
                "http_proxy=".to_owned(),
                "http_proxy=changed".to_owned()
            ])
            .err(),
            Some(Error::InvalidValue)
        );
    }

    fn synthetic_observation() -> Observation {
        let desktop = codec::DesktopSnapshot::capture(
            codec::DesktopKey::ALL
                .into_iter()
                .map(|key| {
                    let value = match key {
                        codec::DesktopKey::Mode => codec::DesktopValue::String("none".into()),
                        codec::DesktopKey::IgnoreHosts => codec::DesktopValue::Strings(vec![]),
                        _ => match key.schema_key_type().2 {
                            "s" => codec::DesktopValue::String(String::new()),
                            "b" => codec::DesktopValue::Bool(false),
                            "i" => codec::DesktopValue::Int(0),
                            _ => unreachable!(),
                        },
                    };
                    codec::DesktopEntry {
                        key,
                        effective: value.clone(),
                        default: value,
                        user: codec::Override::Absent,
                        writable: true,
                    }
                })
                .collect(),
        )
        .unwrap();
        Observation {
            desktop,
            manager: project_manager_environment(&[]).unwrap(),
            provenance: Provenance::Unverified,
        }
    }

    #[test]
    fn private_frame_is_exact_and_never_admits_writes() {
        let observation = synthetic_observation();
        let frame = observation.encode_private_frame().unwrap();
        let decoded = Observation::decode_private_frame(&frame).unwrap();
        assert_eq!(decoded.desktop, observation.desktop);
        assert_eq!(decoded.manager, observation.manager);
        assert_eq!(decoded.admit_writes(), Err(Error::IdentityUnverified));
        assert!(format!("{decoded:?}").contains("[private]"));
        assert_eq!(
            Observation::decode_private_frame(&frame[..frame.len() - 1]).err(),
            Some(Error::InvalidFrame)
        );
        let mut trailing = frame;
        trailing.push(0);
        assert_eq!(
            Observation::decode_private_frame(&trailing).err(),
            Some(Error::InvalidFrame)
        );
    }
}
