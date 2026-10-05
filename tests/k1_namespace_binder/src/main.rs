//! Fixed noninstalled local-binding probe. No canonical authority or datagrams.
#![forbid(unsafe_code)]

use k1_real_namespace_binder_review::{FixedAttempt, Refused};
use std::io::{self, Write};
use std::process::ExitCode;

const ARG: &str = "--fixed-k1-local-binder-not-production";
const NEGATIVE_ARG: &str = "--fixed-k1-local-binder-expect-mismatch-not-production";
const MATCH: &[u8] = b"K1_BINDER_LOCAL_MATCH_NOT_PRODUCTION\n";
const MISMATCH: &[u8] = b"K1_BINDER_LOCAL_MISMATCH_NOT_PRODUCTION\n";

// Private sequencing seam; main supplies one fixed attempt and its elapsed gate.
// No retry or error output after any write/flush attempt.
fn entry(
    allowed: bool,
    expect_mismatch: bool,
    gate: &mut impl FnMut() -> Result<(), ()>,
    binding: impl FnOnce() -> Result<(), Refused>,
    output: &mut impl Write,
) -> Result<u8, ()> {
    if !allowed {
        return Err(());
    }
    gate()?;
    let result = binding();
    gate()?;
    let raw = match result {
        Ok(()) if !expect_mismatch => MATCH,
        Err(Refused::Mismatch) if expect_mismatch => MISMATCH,
        Err(_) => return Err(()),
        Ok(()) => return Err(()),
    };
    gate()?;
    if output.write(raw).map_err(|_| ())? != raw.len() {
        return Err(());
    }
    gate()?;
    output.flush().map_err(|_| ())?;
    gate()?;
    Ok(0)
}

fn configuration(
    arg: Option<&std::ffi::OsStr>,
    extra: bool,
    fds: Option<&str>,
    names: Option<&str>,
    pid: Option<&str>,
    own_pid: u32,
) -> Option<bool> {
    let negative = arg == Some(std::ffi::OsStr::new(NEGATIVE_ARG));
    if (arg != Some(std::ffi::OsStr::new(ARG)) && !negative)
        || extra
        || fds != Some("1")
        || names != Some("k1-untrusted-local-anchor")
        || pid != Some(own_pid.to_string().as_str())
    {
        return None;
    }
    Some(negative)
}

fn main() -> ExitCode {
    let Ok(attempt) = FixedAttempt::start() else {
        return ExitCode::from(2);
    };
    let mut args = std::env::args_os();
    let _ = args.next();
    let arg = args.next();
    // Configuration guards only, never manager authentication. No supplied PID
    // is queried, and no supplied name/path becomes an opener argument.
    let config = configuration(
        arg.as_deref(),
        args.next().is_some(),
        std::env::var("LISTEN_FDS").ok().as_deref(),
        std::env::var("LISTEN_FDNAMES").ok().as_deref(),
        std::env::var("LISTEN_PID").ok().as_deref(),
        std::process::id(),
    );
    let result = entry(
        config.is_some(),
        config.unwrap_or(false),
        &mut || attempt.check_elapsed().map_err(|_| ()),
        || attempt.verify_inherited_local(),
        &mut io::stdout().lock(),
    );
    ExitCode::from(result.unwrap_or(2))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct Output<'a> {
        writes: usize,
        flushes: usize,
        short: bool,
        fail: bool,
        flush_fail: bool,
        late: &'a Cell<bool>,
    }
    impl Write for Output<'_> {
        fn write(&mut self, raw: &[u8]) -> io::Result<usize> {
            self.writes += 1;
            if self.fail {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            Ok(raw.len() - usize::from(self.short))
        }
        fn flush(&mut self) -> io::Result<()> {
            self.flushes += 1;
            if self.flush_fail {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            self.late.set(true);
            Ok(())
        }
    }

    #[test]
    fn exact_handoff_configuration_is_not_origin_authentication() {
        let good = std::ffi::OsStr::new(ARG);
        let negative = std::ffi::OsStr::new(NEGATIVE_ARG);
        assert_eq!(
            configuration(
                Some(good),
                false,
                Some("1"),
                Some("k1-untrusted-local-anchor"),
                Some("42"),
                42
            ),
            Some(false)
        );
        assert_eq!(
            configuration(
                Some(negative),
                false,
                Some("1"),
                Some("k1-untrusted-local-anchor"),
                Some("42"),
                42
            ),
            Some(true)
        );
        for (arg, extra, fds, name, pid) in [
            (
                None,
                false,
                Some("1"),
                Some("k1-untrusted-local-anchor"),
                Some("42"),
            ),
            (
                Some(good),
                true,
                Some("1"),
                Some("k1-untrusted-local-anchor"),
                Some("42"),
            ),
            (
                Some(good),
                false,
                Some("2"),
                Some("k1-untrusted-local-anchor"),
                Some("42"),
            ),
            (
                Some(good),
                false,
                Some("01"),
                Some("k1-untrusted-local-anchor"),
                Some("42"),
            ),
            (Some(good), false, Some("1"), Some("foreign"), Some("42")),
            (
                Some(good),
                false,
                Some("1"),
                Some("k1-untrusted-local-anchor:extra"),
                Some("42"),
            ),
            (
                Some(good),
                false,
                Some("1"),
                Some("k1-untrusted-local-anchor"),
                Some("1"),
            ),
            (
                Some(good),
                false,
                Some("1"),
                Some("k1-untrusted-local-anchor"),
                Some("042"),
            ),
        ] {
            assert_eq!(configuration(arg, extra, fds, name, pid, 42), None);
        }
    }

    #[test]
    fn fixed_result_classes_no_unavailable_promotion() {
        for (negative, result, expected, raw) in [
            (false, Ok(()), Ok(0), MATCH),
            (true, Err(Refused::Mismatch), Ok(0), MISMATCH),
            (true, Ok(()), Err(()), &b""[..]),
            (false, Err(Refused::Mismatch), Err(()), &b""[..]),
            (true, Err(Refused::Unavailable), Err(()), &b""[..]),
            (true, Err(Refused::Expired), Err(()), &b""[..]),
            (true, Err(Refused::Sealed), Err(()), &b""[..]),
        ] {
            let mut output = Vec::new();
            assert_eq!(
                entry(true, negative, &mut || Ok(()), || result, &mut output),
                expected
            );
            assert_eq!(output, raw);
        }
    }

    #[test]
    fn wrong_argument_or_late_binding_never_outputs() {
        let called = Cell::new(false);
        let mut output = Vec::new();
        assert!(entry(
            false,
            false,
            &mut || Ok(()),
            || {
                called.set(true);
                Ok(())
            },
            &mut output
        )
        .is_err());
        assert!(!called.get());
        assert!(entry(
            true,
            false,
            &mut || if called.get() { Err(()) } else { Ok(()) },
            || {
                called.set(true);
                Ok(())
            },
            &mut output
        )
        .is_err());
        assert!(output.is_empty());
    }

    #[test]
    fn short_throw_and_late_flush_have_one_output_attempt() {
        for (short, fail, flush_fail) in [
            (true, false, false),
            (false, true, false),
            (false, false, false),
            (false, false, true),
        ] {
            let late = Cell::new(false);
            let mut output = Output {
                writes: 0,
                flushes: 0,
                short,
                fail,
                flush_fail,
                late: &late,
            };
            assert!(entry(
                true,
                false,
                &mut || if late.get() { Err(()) } else { Ok(()) },
                || Ok(()),
                &mut output
            )
            .is_err());
            assert_eq!(output.writes, 1);
            assert_eq!(output.flushes, usize::from(!short && !fail));
        }
    }

    #[test]
    fn late_write_prevents_flush() {
        let count = Cell::new(0);
        let late = Cell::new(false);
        let mut output = Output {
            writes: 0,
            flushes: 0,
            short: false,
            fail: false,
            flush_fail: false,
            late: &late,
        };
        let mut gate = || {
            count.set(count.get() + 1);
            if count.get() == 4 {
                Err(())
            } else {
                Ok(())
            }
        };
        assert!(entry(true, false, &mut gate, || Ok(()), &mut output).is_err());
        assert_eq!((output.writes, output.flushes), (1, 0));
    }
}
