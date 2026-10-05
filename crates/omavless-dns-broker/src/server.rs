//! Single serialized fixed-purpose root listener. No client-selected targets.
use crate::{
    admission::RootContext,
    diagnostic::{Refusal, report},
    journal::Journal,
    transaction::{Lease, Outcome},
};
use omavless_dns_channel::{Error as ChannelError, Listener, Response, Session};
use omavless_dns_resolved::ManagedResolved;
use omavless_dns_retention::Retention;
use omavless_dns_tun::HeldTun;
use std::{
    fmt,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

const SOCKET: &str = "/run/omavless-dns/control.sock";
pub(crate) fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(30)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AdmissionRefused,
    Unavailable,
    RecoveryRequired,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::AdmissionRefused => "DNS broker admission was refused.",
            Self::Unavailable => "DNS broker is unavailable.",
            Self::RecoveryRequired => "DNS broker state requires administrator recovery.",
        })
    }
}
impl std::error::Error for Error {}

/// Only valid in the fixed admitted root service. Not wired to current runtime
/// or package installation. Unexpected pending state is never automatically reset.
pub fn serve() -> Result<(), Error> {
    // The handler only latches a request. It performs no cleanup or DNS work.
    // Never reset this flag or turn signal death into a successful exit.
    let stop = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&stop))
        .map_err(|_| Error::Unavailable)?;
    let context = RootContext::admit().map_err(|_| Error::AdmissionRefused)?;
    let mut journal = Journal::open_root().map_err(|_| Error::RecoveryRequired)?;
    if journal.requires_recovery() || journal.phase().is_some() {
        return Err(Error::RecoveryRequired);
    }
    let until = deadline();
    context
        .set_deadline(until)
        .map_err(|_| Error::Unavailable)?;
    fresh_retention(&context, until)?;
    let listener = Listener::bind(Path::new(SOCKET), context.enrolled_uid())
        .map_err(|_| Error::Unavailable)?;
    let access =
        crate::access::SocketAccess::grant(&context).map_err(|_| Error::AdmissionRefused)?;
    context.notify_ready().map_err(|_| Error::Unavailable)?;
    loop {
        if stop.load(Ordering::Acquire) {
            return verify_idle_stop(&context, &access, &journal);
        }
        context
            .set_deadline(Instant::now() + Duration::from_secs(5))
            .map_err(|_| Error::Unavailable)?;
        context.recheck().map_err(|error| {
            report(Refusal::Authority(error));
            Error::RecoveryRequired
        })?;
        access.recheck().map_err(|_| Error::AdmissionRefused)?;
        if stop.load(Ordering::Acquire) {
            return verify_idle_stop(&context, &access, &journal);
        }
        // Existing accept() keeps its ten-second contract. This caller polls
        // only idle admission briefly so a stop request can reach the proof.
        let mut session = match listener.accept_until(Instant::now() + Duration::from_secs(1)) {
            Ok(session) => session,
            Err(ChannelError::Timeout | ChannelError::PeerRejected) => continue,
            Err(_) => return Err(Error::Unavailable),
        };
        // A raced accepted peer has not begun lease admission or DNS effects.
        // Drop only that unstarted channel, then prove the same idle boundary.
        if stop.load(Ordering::Acquire) {
            drop(session);
            return verify_idle_stop(&context, &access, &journal);
        }
        let proof = match session.receive_acquire().and_then(|fd| {
            fd.try_clone_to_owned()
                .map_err(|_| ChannelError::InvalidDescriptor)
        }) {
            Ok(proof) => proof,
            Err(error) => {
                report(Refusal::Channel(error));
                continue;
            }
        };
        let until = deadline();
        context
            .set_deadline(until)
            .map_err(|_| Error::Unavailable)?;
        // Kernel admission is mandatory even for an enrolled same-user process.
        let held = match HeldTun::admit(proof) {
            Ok(held) => held,
            Err(error) => {
                report(Refusal::Tun(error));
                let _ = session.reply(Response::Rejected);
                continue;
            }
        };
        let index = held.interface_index();
        let resolved = match ManagedResolved::from_admitted_parts_with_deadline(
            context.connection().clone(),
            context.resolved_owner().to_owned(),
            held,
            until,
        ) {
            Ok(resolved) => resolved,
            Err(error) => {
                report(Refusal::Resolved(error));
                let _ = session.reply(Response::Rejected);
                continue;
            }
        };
        let retention = fresh_retention(&context, until)?;
        let proof = session
            .proof()
            .ok_or(Error::Unavailable)?
            .try_clone_to_owned()
            .map_err(|_| Error::Unavailable)?;
        if session.reply(Response::Applying).is_err() {
            continue;
        }
        let mut lease = Lease::new(&context, &mut journal, resolved, retention, proof, index);
        match lease.apply() {
            Outcome::Refused | Outcome::Clean => {
                let _ = session.reply(Response::Rejected);
            }
            Outcome::RecoveryRequired => {
                let _ = session.reply(Response::RecoveryRequired);
                return Err(Error::RecoveryRequired);
            }
            Outcome::Ready => {
                // Loss here does not mean DNS cleanup. All our writes are joined;
                // run the same owned-object cleanup before releasing retention.
                let explicit = if session.reply(Response::Ready).is_ok() {
                    match wait_release(&mut session, &mut lease) {
                        Some(explicit) => explicit,
                        None => {
                            let _ = session.reply(Response::RecoveryRequired);
                            return Err(Error::RecoveryRequired);
                        }
                    }
                } else {
                    false
                };
                if explicit {
                    let _ = session.reply(Response::Releasing);
                }
                if lease.release() != Outcome::Clean {
                    let _ = session.reply(Response::RecoveryRequired);
                    return Err(Error::RecoveryRequired);
                }
                if explicit {
                    let _ = session.reply(Response::Released);
                }
            }
        }
    }
}

/// Only reached between complete serialized sessions. A flag, cached journal
/// phase or successful channel close is not settled-release evidence. Active
/// apply/release/wait paths never call this helper or observe the stop flag.
fn verify_idle_stop(
    context: &RootContext,
    access: &crate::access::SocketAccess,
    journal: &Journal,
) -> Result<(), Error> {
    let until = Instant::now() + Duration::from_secs(5);
    context
        .set_deadline(until)
        .map_err(|_| Error::Unavailable)?;
    idle_stop_checks(until, |step| match step {
        IdleProof::Context => context.recheck().map_err(|_| Error::RecoveryRequired),
        IdleProof::Access => access.recheck().map_err(|_| Error::AdmissionRefused),
        IdleProof::Journal => journal
            .verify_idle_empty()
            .map_err(|_| Error::RecoveryRequired),
        IdleProof::Retention => fresh_retention(context, until).map(|_| ()),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IdleProof {
    Context,
    Access,
    Journal,
    Retention,
}

// Private read-only composition seam: callers cannot select steps or bypass
// the complete prefix/final checks. Tests inject failure, not host authority.
fn idle_stop_checks(
    until: Instant,
    mut check: impl FnMut(IdleProof) -> Result<(), Error>,
) -> Result<(), Error> {
    for step in [
        IdleProof::Context,
        IdleProof::Access,
        IdleProof::Journal,
        IdleProof::Retention,
        IdleProof::Context,
        IdleProof::Access,
        IdleProof::Journal,
    ] {
        if Instant::now() >= until {
            return Err(Error::Unavailable);
        }
        check(step)?;
        if Instant::now() >= until {
            return Err(Error::Unavailable);
        }
    }
    Ok(())
}

fn wait_release(session: &mut Session, lease: &mut Lease<'_>) -> Option<bool> {
    loop {
        if !lease.check_active() {
            return None;
        }
        match session.receive_release() {
            Ok(()) => return Some(true),
            Err(ChannelError::Idle) => continue,
            Err(_) => return Some(false),
        }
    }
}

fn fresh_retention(context: &RootContext, until: Instant) -> Result<Retention, Error> {
    let mut retention = Retention::from_admitted_parts_with_deadline(
        context.connection().clone(),
        context.manager_owner(),
        context.duplicate_notify().map_err(|_| Error::Unavailable)?,
        until,
    )
    .map_err(|_| Error::RecoveryRequired)?;
    retention
        .verify_fresh()
        .map_err(|_| Error::RecoveryRequired)?;
    Ok(retention)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_stop_requires_complete_original_prefix_and_final_rechecks() {
        let mut seen = Vec::new();
        idle_stop_checks(Instant::now() + Duration::from_secs(1), |step| {
            seen.push(step);
            Ok(())
        })
        .unwrap();
        assert_eq!(
            seen,
            [
                IdleProof::Context,
                IdleProof::Access,
                IdleProof::Journal,
                IdleProof::Retention,
                IdleProof::Context,
                IdleProof::Access,
                IdleProof::Journal
            ]
        );
        for failed in 0..seen.len() {
            let mut prefix = Vec::new();
            assert_eq!(
                idle_stop_checks(Instant::now() + Duration::from_secs(1), |step| {
                    prefix.push(step);
                    if prefix.len() - 1 == failed {
                        Err(Error::RecoveryRequired)
                    } else {
                        Ok(())
                    }
                }),
                Err(Error::RecoveryRequired)
            );
            assert_eq!(prefix, seen[..=failed]);
        }
    }

    #[test]
    fn idle_stop_expiry_is_not_zero_and_never_dispatches_next_check() {
        assert_eq!(
            idle_stop_checks(Instant::now(), |_| panic!("expired first check")),
            Err(Error::Unavailable)
        );
        let mut calls = 0;
        let until = Instant::now() + Duration::from_millis(5);
        assert_eq!(
            idle_stop_checks(until, |_| {
                calls += 1;
                std::thread::sleep(Duration::from_millis(10));
                Ok(())
            }),
            Err(Error::Unavailable)
        );
        assert_eq!(calls, 1);
    }
}
