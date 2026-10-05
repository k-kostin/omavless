//! Fixed noninstalled local-binding probe. No canonical authority or datagrams.
#![forbid(unsafe_code)]

use k1_real_namespace_binder_review::{LocalBinding, Refused};
use std::fs::File;
use std::io::{self, Write};
use std::mem::ManuallyDrop;
use std::process::ExitCode;
use std::time::{Duration, Instant};

const ARG: &str = "--fixed-k1-local-binder-not-production";
const NEGATIVE_ARG: &str = "--fixed-k1-local-binder-expect-mismatch-not-production";
const MATCH: &[u8] = b"K1_BINDER_LOCAL_MATCH_NOT_PRODUCTION\n";
const MISMATCH: &[u8] = b"K1_BINDER_LOCAL_MISMATCH_NOT_PRODUCTION\n";

fn budget(deadline: Instant) -> Result<(), ()> {
    if Instant::now() < deadline {
        Ok(())
    } else {
        Err(())
    }
}

fn actual(deadline: Instant) -> Result<(), Refused> {
    budget(deadline).map_err(|_| Refused::Expired)?;
    // First file opener. This reopens the inherited namespace object, not the
    // same open file description; descriptor number alone proves no origin.
    let anchor = File::open("/proc/self/fd/3").map(ManuallyDrop::new);
    budget(deadline).map_err(|_| Refused::Expired)?;
    let anchor = anchor.map_err(|_| Refused::Unavailable)?;
    let mut owner = LocalBinding::bind_untrusted_anchor(ManuallyDrop::into_inner(anchor))?;
    budget(deadline).map_err(|_| Refused::Expired)?;
    owner.verify_local()?;
    budget(deadline).map_err(|_| Refused::Expired)?;
    // Library originals remain retained through process exit, including errors.
    Ok(())
}

// Private fixed sequencing seam; main supplies only actual and Instant checks.
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

fn main() -> ExitCode {
    let Some(deadline) = Instant::now().checked_add(Duration::from_secs(2)) else {
        return ExitCode::from(2);
    };
    let mut args = std::env::args_os();
    let _ = args.next();
    let arg = args.next();
    let expect_mismatch = arg.as_deref() == Some(std::ffi::OsStr::new(NEGATIVE_ARG));
    // Configuration guards only, never manager authentication. No supplied PID
    // is queried, and no supplied name/path becomes an opener argument.
    let allowed = (arg.as_deref() == Some(std::ffi::OsStr::new(ARG)) || expect_mismatch)
        && args.next().is_none()
        && std::env::var("LISTEN_FDS").as_deref() == Ok("1")
        && std::env::var("LISTEN_FDNAMES").as_deref() == Ok("k1-untrusted-local-anchor")
        && std::env::var("LISTEN_PID").ok().as_deref()
            == Some(std::process::id().to_string().as_str());
    let result = entry(
        allowed,
        expect_mismatch,
        &mut || budget(deadline),
        || actual(deadline),
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
            self.late.set(true);
            Ok(())
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
        for (short, fail) in [(true, false), (false, true), (false, false)] {
            let late = Cell::new(false);
            let mut output = Output {
                writes: 0,
                flushes: 0,
                short,
                fail,
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
