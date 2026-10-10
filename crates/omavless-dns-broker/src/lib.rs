//! Fixed-purpose root DNS broker, with distinct experimental and release identities.
#[cfg(feature = "k1-managed-device")]
pub(crate) const ENROLLMENT_POLICY: &str = "omavless0-ipv4-development-v1";
#[cfg(all(feature = "release-package", not(feature = "k1-managed-device")))]
pub(crate) const ENROLLMENT_POLICY: &str = "meta-ipv4-release-v1";
#[cfg(not(feature = "release-package"))]
pub(crate) const ENROLLMENT_POLICY: &str = "meta-ipv4-v1";
mod access;
pub mod admin;
pub mod admission;
mod diagnostic;
pub mod journal;
mod server;
mod transaction;
pub use server::{Error, serve};

#[cfg(test)]
mod device_policy_tests {
    #[test]
    fn selected_enrollment_cannot_promote_another_flavor() {
        let expected = if cfg!(feature = "k1-managed-device") {
            "omavless0-ipv4-development-v1"
        } else if cfg!(feature = "release-package") {
            "meta-ipv4-release-v1"
        } else {
            "meta-ipv4-v1"
        };
        assert_eq!(super::ENROLLMENT_POLICY, expected);
    }
}
