//! Inactive, single-client-at-a-time owner for an already bound Unix listener.
//! This is not a socket publisher, installed helper, or kernel authority.

use crate::effect_port::EffectPort;
use crate::locked_state::{ExchangeError, LockedState};
use crate::receipt::NamespaceObservation;
use crate::transport_candidate::TransportError;
use std::io::{self, ErrorKind};
use std::os::unix::net::{UnixListener, UnixStream};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SessionProgress {
    Idle,
    Served,
    Refused(ExchangeError),
    AcceptUnavailable,
    AuthorityLost,
}

/// One owner retains the enrollment binding and the durable-state lock across
/// clients. No worker threads, queue, retry or automatic recovery are created.
/// A future installed service must separately prove the listener's path,
/// ownership, group, backlog, namespace and kernel-port provenance.
pub(crate) struct SessionOwner<K: EffectPort> {
    listener: UnixListener,
    state: LockedState,
    kernel: K,
    namespace: NamespaceObservation,
    authority_lost: bool,
}

impl<K: EffectPort> SessionOwner<K> {
    pub(crate) fn from_prebound(
        listener: UnixListener,
        state: LockedState,
        kernel: K,
        namespace: NamespaceObservation,
    ) -> io::Result<Self> {
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener,
            state,
            kernel,
            namespace,
            authority_lost: false,
        })
    }

    /// Never loops on an accept error or on a stalled peer. Each call accepts
    /// at most one connection, then closes that stream after exactly one
    /// existing enrollment-bound exchange. A lost reply is not replayed.
    pub(crate) fn poll_one(&mut self) -> SessionProgress {
        self.poll_with(|listener| listener.accept().map(|(stream, _)| stream))
    }

    fn poll_with(
        &mut self,
        accept: impl FnOnce(&UnixListener) -> io::Result<UnixStream>,
    ) -> SessionProgress {
        if self.authority_lost || !self.state.enrollment_current() {
            self.authority_lost = true;
            return SessionProgress::AuthorityLost;
        }
        let stream = match accept(&self.listener) {
            Ok(stream) => stream,
            Err(error) if error.kind() == ErrorKind::WouldBlock => return SessionProgress::Idle,
            Err(_) => return SessionProgress::AcceptUnavailable,
        };
        if stream.set_nonblocking(false).is_err() {
            return SessionProgress::Refused(ExchangeError::Receive(TransportError::Unavailable));
        }
        match self
            .state
            .exchange_once(stream, self.namespace, &mut self.kernel)
        {
            Ok(()) => SessionProgress::Served,
            Err(error) => {
                if matches!(
                    error,
                    ExchangeError::NoEnrollment
                        | ExchangeError::Receive(TransportError::EnrollmentChanged)
                        | ExchangeError::ReplyDeliveryUnknown(TransportError::EnrollmentChanged)
                ) {
                    self.authority_lost = true;
                }
                SessionProgress::Refused(error)
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn test_poll_with(
        &mut self,
        accept: impl FnOnce(&UnixListener) -> io::Result<UnixStream>,
    ) -> SessionProgress {
        self.poll_with(accept)
    }

    #[cfg(test)]
    pub(crate) fn test_state(&self) -> &LockedState {
        &self.state
    }

    #[cfg(test)]
    pub(crate) fn test_kernel(&self) -> &K {
        &self.kernel
    }
}
