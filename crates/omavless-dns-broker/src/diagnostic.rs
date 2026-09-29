//! Fixed admission diagnostics. Never accept bus/kernel text or client values.
use omavless_dns_channel::Error as ChannelError;
use omavless_dns_resolved::Error as ResolvedError;
use omavless_dns_tun::Error as TunError;
use std::fmt;

pub(crate) enum Refusal {
    Channel(ChannelError),
    Tun(TunError),
    Resolved(ResolvedError),
    Baseline(ResolvedError),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Channel(error) => write!(f, "DNS broker acquire refused: {error}"),
            Self::Tun(error) => write!(f, "DNS broker TUN refused: {error}"),
            Self::Resolved(error) => write!(f, "DNS broker resolver refused: {}", error.code()),
            Self::Baseline(error) => write!(f, "DNS broker baseline refused: {}", error.code()),
        }
    }
}

pub(crate) fn report(refusal: Refusal) {
    // All payloads are closed enums without strings/IDs/paths. One message per
    // rejected authenticated acquisition; no successful polling/idle log spam.
    eprintln!("{refusal}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refusal_messages_are_bounded_fixed_public_classifications() {
        let mut messages = Vec::new();
        for error in [
            ChannelError::Idle,
            ChannelError::InvalidFrame,
            ChannelError::InvalidDescriptor,
            ChannelError::PeerRejected,
            ChannelError::Unavailable,
            ChannelError::Timeout,
            ChannelError::ChannelLost,
            ChannelError::InvalidState,
        ] {
            messages.push(Refusal::Channel(error).to_string());
        }
        for error in [
            TunError::UnsupportedPlatform,
            TunError::InvalidDescriptor,
            TunError::InvalidTun,
            TunError::NamespaceMismatch,
            TunError::Changed,
            TunError::KernelUnavailable,
        ] {
            messages.push(Refusal::Tun(error).to_string());
        }
        for error in [
            ResolvedError::Unavailable,
            ResolvedError::InvalidReply,
            ResolvedError::ReadbackMismatch,
            ResolvedError::AuthorizationRefused,
            ResolvedError::OutcomeUnknown,
            ResolvedError::RecoveryRequired,
            ResolvedError::NonPristine,
            ResolvedError::UnsupportedPolicy,
            ResolvedError::LeaseLost,
            ResolvedError::OwnershipChanged,
        ] {
            messages.push(Refusal::Resolved(error).to_string());
            messages.push(Refusal::Baseline(error).to_string());
        }
        assert_eq!(messages.len(), 34);
        for message in messages {
            assert!(message.starts_with("DNS broker "));
            assert!(message.len() <= 160);
            assert!(message.bytes().all(|b| b.is_ascii_graphic() || b == b' '));
            assert!(!message.contains('/'));
            assert!(!message.contains('@'));
        }
        assert_eq!(
            Refusal::Baseline(ResolvedError::NonPristine).to_string(),
            "DNS broker baseline refused: dns_baseline_not_pristine"
        );
    }
}
