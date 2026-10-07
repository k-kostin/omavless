// SPDX-License-Identifier: MIT
use crate::{Error, Result};

pub(crate) const LIMIT: usize = 16384;
/// Kernel-only fields remain private and have no Debug/serialization surface.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Status {
    pub pid: u32,
    pub parent: u32,
    pub uids: [u32; 4],
    pub start: u64,
}
fn decimal(raw: &str) -> Option<u32> {
    if raw.is_empty()
        || raw.len() > 10
        || (raw.len() > 1 && raw.starts_with('0'))
        || !raw.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    raw.parse().ok()
}
fn text(raw: &[u8]) -> Result<&str> {
    if raw.is_empty()
        || raw.len() > LIMIT
        || !raw.ends_with(b"\n")
        || raw.iter().any(|b| !b.is_ascii() || *b == 0 || *b == b'\r')
    {
        return Err(Error::Refused);
    }
    std::str::from_utf8(raw).map_err(|_| Error::Refused)
}
pub(crate) fn pidfd(raw: &[u8]) -> Result<u32> {
    let mut pid = None;
    let mut namespace = None;
    for line in text(raw)?.split_terminator('\n') {
        let (key, value) = line.split_once(':').ok_or(Error::Refused)?;
        let value = value.trim_matches([' ', '\t']);
        let unique = |field: &mut Option<u32>| -> Result<()> {
            if field
                .replace(decimal(value).ok_or(Error::Refused)?)
                .is_some()
            {
                Err(Error::Refused)
            } else {
                Ok(())
            }
        };
        match key {
            "Pid" => unique(&mut pid)?,
            "NSpid" => unique(&mut namespace)?,
            _ => (),
        }
    }
    let pid = pid.ok_or(Error::Refused)?;
    if pid == 0 || namespace != Some(pid) {
        return Err(Error::Refused);
    }
    Ok(pid)
}
pub(crate) fn process(status: &[u8], stat: &[u8]) -> Result<Status> {
    let mut fields = [None; 6];
    for line in text(status)?.split_terminator('\n') {
        let (key, value) = line.split_once(':').ok_or(Error::Refused)?;
        let n = match key {
            "Pid" => 0,
            "Tgid" => 1,
            "PPid" => 2,
            "Uid" => 3,
            "NSpid" => 4,
            "State" => 5,
            _ => continue,
        };
        if fields[n].replace(value.trim_matches([' ', '\t'])).is_some() {
            return Err(Error::Refused);
        }
    }
    let get = |n: usize| fields[n].ok_or(Error::Refused);
    let pid = decimal(get(0)?).filter(|n| *n > 0).ok_or(Error::Refused)?;
    if decimal(get(1)?) != Some(pid) || decimal(get(4)?) != Some(pid) {
        return Err(Error::Refused);
    }
    let state = get(5)?
        .split_ascii_whitespace()
        .next()
        .ok_or(Error::Refused)?;
    if !matches!(state, "R" | "S" | "D") {
        return Err(Error::Refused);
    }
    let uids: Vec<_> = get(3)?.split_ascii_whitespace().map(decimal).collect();
    if uids.len() != 4 {
        return Err(Error::Refused);
    }
    let stat = text(stat)?;
    let end = stat.rfind(") ").ok_or(Error::Refused)?;
    let lead = stat[..end].split_once(" (").ok_or(Error::Refused)?.0;
    let tail: Vec<_> = stat[end + 2..].split_ascii_whitespace().collect();
    // Scheduling can legitimately move R/S/D between these two reads. Both
    // must independently be non-exited; copied state equality is not identity.
    if decimal(lead) != Some(pid) || tail.len() < 20 || !matches!(tail[0], "R" | "S" | "D") {
        return Err(Error::Refused);
    }
    let parent = decimal(get(2)?).ok_or(Error::Refused)?;
    if decimal(tail[1]) != Some(parent) || !tail[19].bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::Refused);
    }
    let start = tail[19].parse::<u64>().map_err(|_| Error::Refused)?;
    if start == 0 {
        return Err(Error::Refused);
    }
    Ok(Status {
        pid,
        parent,
        uids: [
            uids[0].ok_or(Error::Refused)?,
            uids[1].ok_or(Error::Refused)?,
            uids[2].ok_or(Error::Refused)?,
            uids[3].ok_or(Error::Refused)?,
        ],
        start,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pidfd_requires_current_positive_single_namespace_kernel_pid() {
        assert_eq!(pidfd(b"pos:\t0\nPid:\t42\nNSpid:\t42\n").unwrap(), 42);
        for raw in [
            b"Pid:\t42\n".as_slice(),
            b"Pid:\t0\nNSpid:\t0\n",
            b"Pid:\t-1\nNSpid:\t-1\n",
            b"Pid:\t42\nNSpid:\t42\t4\n",
            b"Pid:\t42\nPid:\t42\nNSpid:\t42\n",
            b"Pid:\t42\nNSpid:\t43\n",
            b"Pid:\t042\nNSpid:\t42\n",
        ] {
            assert!(pidfd(raw).is_err());
        }
    }
    #[test]
    fn whole_leader_state_start_parent_uid_and_namespace_are_bound() {
        let status=b"Pid:\t42\nTgid:\t42\nPPid:\t1\nUid:\t1000\t1000\t1000\t1000\nNSpid:\t42\nState:\tS (sleeping)\n";
        let stat = format!("42 (mihomo) S 1 {} 10\n", vec!["0"; 17].join(" "));
        let valid = process(status, stat.as_bytes()).unwrap();
        assert_eq!(valid.start, 10);
        for (a, b) in [
            ("Tgid:\t42", "Tgid:\t43"),
            ("NSpid:\t42", "NSpid:\t42\t4"),
            ("State:\tS", "State:\tZ"),
            ("PPid:\t1", "PPid:\t2"),
            ("Uid:\t1000\t1000\t1000\t1000", "Uid:\t1000"),
        ] {
            let changed = std::str::from_utf8(status).unwrap().replace(a, b);
            assert!(process(changed.as_bytes(), stat.as_bytes()).is_err());
        }
        assert!(process(status, stat.replace(" 10\n", " 0\n").as_bytes()).is_err());
        assert!(pidfd(&vec![b'x'; LIMIT + 1]).is_err());
    }
}
