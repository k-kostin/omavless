// SPDX-License-Identifier: MIT

//! Bounded, read-only observation of an already-owned core process group.
//!
//! This does not establish ownership and never authorizes signalling by itself.
//! The caller must separately pin the group identity to its unreaped child.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

const MAX_PROC_ENTRIES: usize = 32_768;
const MAX_STAT_BYTES: usize = 4096;
const MAX_LIVE_MEMBERS: usize = 64;

pub(crate) fn group_has_live_members(proc_root: &Path, group: i32) -> Result<bool, ()> {
    scan(proc_root, group, MAX_PROC_ENTRIES, MAX_LIVE_MEMBERS)
}

fn scan(proc_root: &Path, group: i32, entry_limit: usize, member_limit: usize) -> Result<bool, ()> {
    if group <= 1 {
        return Err(());
    }
    let mut live_members = 0;
    for (index, entry) in fs::read_dir(proc_root).map_err(|_| ())?.enumerate() {
        if index >= entry_limit {
            return Err(());
        }
        let Some(entry) = absent_ok(entry)? else {
            continue;
        };
        let name = entry.file_name();
        let bytes = name.as_encoded_bytes();
        if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
            continue;
        }
        let Some(kind) = absent_ok(entry.file_type())? else {
            continue;
        };
        if !kind.is_dir() {
            continue;
        }
        let pid = positive_pid(bytes)?;
        let Some(file) = absent_ok(File::open(entry.path().join("stat")))? else {
            // A process may disappear between directory enumeration and open.
            continue;
        };
        let mut raw = Vec::new();
        let Some(_) = absent_ok(file.take((MAX_STAT_BYTES + 1) as u64).read_to_end(&mut raw))?
        else {
            continue;
        };
        if raw.len() > MAX_STAT_BYTES {
            return Err(());
        }
        let stat = parse_stat(&raw)?;
        if stat.pid != pid {
            return Err(());
        }
        if stat.group == group && stat.live {
            live_members += 1;
            if live_members > member_limit {
                return Err(());
            }
        }
        // Never return early: a later malformed/unreadable entry or exceeded
        // bound must prevent a successful completeness observation.
    }
    Ok(live_members != 0)
}

fn absent_ok<T>(result: io::Result<T>) -> Result<Option<T>, ()> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(()),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ProcessStat {
    pid: i32,
    group: i32,
    live: bool,
}

fn positive_pid(raw: &[u8]) -> Result<i32, ()> {
    let value = unsigned_pid(raw)?;
    (value > 0).then_some(value).ok_or(())
}

fn unsigned_pid(raw: &[u8]) -> Result<i32, ()> {
    if raw.is_empty() || !raw.iter().all(u8::is_ascii_digit) {
        return Err(());
    }
    raw.iter().try_fold(0_i32, |value, digit| {
        value
            .checked_mul(10)
            .and_then(|value| value.checked_add(i32::from(digit - b'0')))
            .ok_or(())
    })
}

fn parse_stat(raw: &[u8]) -> Result<ProcessStat, ()> {
    if raw.is_empty() || raw.len() > MAX_STAT_BYTES {
        return Err(());
    }
    // comm may contain spaces, parentheses and non-UTF-8 bytes. The numeric
    // suffix cannot contain ')', so only its LAST occurrence is authoritative.
    let close = raw.iter().rposition(|byte| *byte == b')').ok_or(())?;
    let open = raw.iter().position(|byte| *byte == b'(').ok_or(())?;
    if open < 2 || close < open || raw[open - 1] != b' ' {
        return Err(());
    }
    let pid = positive_pid(&raw[..open - 1])?;
    let suffix = &raw[close + 1..];
    if suffix.first() != Some(&b' ') {
        return Err(());
    }
    let mut fields = suffix
        .split(u8::is_ascii_whitespace)
        .filter(|field| !field.is_empty());
    let state = fields.next().ok_or(())?;
    if state.len() != 1 || !b"RSDTtZXxKWPI".contains(&state[0]) {
        return Err(());
    }
    // PPID can be zero, unlike the process ID.
    unsigned_pid(fields.next().ok_or(())?)?;
    // Kernel threads can legitimately have group zero. They are unrelated to
    // an owned userspace group; rejecting their records would block every scan.
    let group = unsigned_pid(fields.next().ok_or(())?)?;
    Ok(ProcessStat {
        pid,
        group,
        live: !b"ZXx".contains(&state[0]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct ProcFixture(PathBuf);

    impl ProcFixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "omavless-core-group-{}-{timestamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }

        fn process(&self, pid: i32, group: i32, state: char) {
            let directory = self.0.join(pid.to_string());
            fs::create_dir(&directory).unwrap();
            fs::write(
                directory.join("stat"),
                format!("{pid} (synthetic (helper)) {state} 1 {group} 0 0\n"),
            )
            .unwrap();
        }
    }

    impl Drop for ProcFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn parser_uses_last_parenthesis_and_accepts_non_utf8_comm() {
        for raw in [
            b"12 (strange ) S 9 999 ( helper)) S 1 42 0\n".as_slice(),
            b"12 (helper\xff\n\t)) R 0 42 0\n".as_slice(),
        ] {
            assert_eq!(
                parse_stat(raw),
                Ok(ProcessStat {
                    pid: 12,
                    group: 42,
                    live: true,
                })
            );
        }
    }

    #[test]
    fn parser_distinguishes_dead_from_supported_live_states() {
        for state in b"RSDTtZXxKWPI" {
            let raw = format!("12 (fixture) {} 1 42\n", char::from(*state));
            assert_eq!(
                parse_stat(raw.as_bytes()).unwrap().live,
                !b"ZXx".contains(state)
            );
        }
    }

    #[test]
    fn parser_rejects_malformed_and_oversized_stat() {
        for raw in [
            "",
            "12 fixture S 1 42",
            "12 (fixture S 1 42",
            "12) (S 1 42",
            "12(fixture) S 1 42",
            "12 (fixture)S 1 42",
            "12 (fixture)",
            "12 (fixture) SS 1 42",
            "12 (fixture) ? 1 42",
            "12 (fixture) S",
            "12 (fixture) S 1",
            "12 (fixture) S -1 42",
            "12 (fixture) S a 42",
            "12 (fixture) S 1 -42",
            "12 (fixture) S 1 +42",
            "12 (fixture) S 1 2147483648",
            "12 (fixture) S 2147483648 42",
            "0 (fixture) S 1 42",
            "2147483648 (fixture) S 1 42",
        ] {
            assert!(parse_stat(raw.as_bytes()).is_err());
        }
        let mut maximum = b"12 (fixture) S 1 42 ".to_vec();
        maximum.resize(MAX_STAT_BYTES, b'0');
        assert!(parse_stat(&maximum).is_ok());
        maximum.push(b'0');
        assert!(parse_stat(&maximum).is_err());
        assert_eq!(parse_stat(b"2 (kthreadd) S 0 0\n").unwrap().group, 0);
    }

    #[test]
    fn scan_empty_unrelated_zombie_and_live_members() {
        let fixture = ProcFixture::new();
        assert_eq!(group_has_live_members(&fixture.0, 42), Ok(false));
        fixture.process(10, 20, 'S');
        fixture.process(2, 0, 'S');
        fixture.process(11, 42, 'Z');
        fixture.process(12, 42, 'X');
        assert_eq!(group_has_live_members(&fixture.0, 42), Ok(false));
        fixture.process(13, 42, 'S');
        assert_eq!(group_has_live_members(&fixture.0, 42), Ok(true));
        assert_eq!(group_has_live_members(&fixture.0, 0), Err(()));
        assert_eq!(group_has_live_members(&fixture.0, 1), Err(()));
        assert_eq!(group_has_live_members(&fixture.0, -42), Err(()));
    }

    #[test]
    fn scan_skips_nonnumeric_nondirectory_and_disappeared_stat() {
        let fixture = ProcFixture::new();
        fs::create_dir(fixture.0.join("self")).unwrap();
        fs::write(fixture.0.join("self/stat"), b"not a stat record").unwrap();
        fs::write(fixture.0.join("123"), b"not a directory").unwrap();
        // Missing stat models exit between directory enumeration and open.
        fs::create_dir(fixture.0.join("124")).unwrap();
        assert_eq!(group_has_live_members(&fixture.0, 42), Ok(false));
        assert_eq!(
            absent_ok::<()>(Err(io::ErrorKind::NotFound.into())),
            Ok(None)
        );
        assert_eq!(
            absent_ok::<()>(Err(io::ErrorKind::PermissionDenied.into())),
            Err(())
        );
        assert_eq!(
            absent_ok::<()>(Err(io::ErrorKind::Interrupted.into())),
            Err(())
        );
    }

    #[test]
    fn scan_does_not_ignore_invalid_unrelated_stat_after_live_member() {
        let fixture = ProcFixture::new();
        fixture.process(10, 42, 'S');
        fixture.process(11, 99, 'S');
        let stat_path = fixture.0.join("11/stat");
        fs::write(&stat_path, b"malformed").unwrap();
        assert_eq!(group_has_live_members(&fixture.0, 42), Err(()));
        fs::write(&stat_path, vec![b'0'; MAX_STAT_BYTES + 1]).unwrap();
        assert_eq!(group_has_live_members(&fixture.0, 42), Err(()));
        fs::write(&stat_path, b"12 (wrong pid) S 1 99\n").unwrap();
        assert_eq!(group_has_live_members(&fixture.0, 42), Err(()));
        fs::remove_file(&stat_path).unwrap();
        fs::create_dir(&stat_path).unwrap();
        assert_eq!(group_has_live_members(&fixture.0, 42), Err(()));
    }

    #[test]
    fn scan_enforces_directory_and_member_budgets() {
        let fixture = ProcFixture::new();
        for pid in 1..=MAX_LIVE_MEMBERS {
            fixture.process(i32::try_from(pid).unwrap(), 42, 'S');
        }
        assert_eq!(group_has_live_members(&fixture.0, 42), Ok(true));
        fixture.process(65, 42, 'S');
        assert_eq!(group_has_live_members(&fixture.0, 42), Err(()));
        assert_eq!(scan(&fixture.0, 42, 64, 100), Err(()));
        assert_eq!(scan(&fixture.0, 42, 65, 100), Ok(true));

        let fixture = ProcFixture::new();
        for name in ["self", "sys", "net"] {
            fs::create_dir(fixture.0.join(name)).unwrap();
        }
        // Nonnumeric entries still consume the enumeration budget.
        assert_eq!(scan(&fixture.0, 42, 2, 64), Err(()));
        assert_eq!(scan(&fixture.0, 42, 3, 64), Ok(false));
        assert_eq!(MAX_PROC_ENTRIES, 32_768);
    }
}
