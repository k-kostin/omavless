// SPDX-License-Identifier: MIT
//! Closed development classes. Selection is not permission or an effect proof.

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Class {
    Tests,
    InstalledRuntime,
}

impl Class {
    pub(crate) const fn client(self) -> &'static str {
        match self {
            Self::Tests => "/usr/lib/omavless-image/development-runtime-tests",
            Self::InstalledRuntime => "/usr/bin/omavless",
        }
    }

    pub(crate) const fn enrollment(self) -> &'static str {
        match self {
            Self::Tests => "/var/lib/omavless-image/development-enrollment-v1",
            Self::InstalledRuntime => "/var/lib/omavless-image/development-runtime-enrollment-v1",
        }
    }

    pub(crate) const fn schema(self) -> &'static str {
        match self {
            Self::Tests => "omavless-development-current-image-v1",
            Self::InstalledRuntime => "omavless-development-installed-runtime-current-image-v1",
        }
    }

    pub(crate) const fn directory(self) -> &'static str {
        match self {
            Self::Tests => "/run/omavless-image",
            Self::InstalledRuntime => "/run/omavless-image-runtime",
        }
    }

    pub(crate) const fn socket(self) -> &'static str {
        match self {
            Self::Tests => "/run/omavless-image/control.sock",
            Self::InstalledRuntime => "/run/omavless-image-runtime/control.sock",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_never_share_client_enrollment_schema_or_endpoint() {
        let old = Class::Tests;
        let runtime = Class::InstalledRuntime;
        assert_ne!(old.client(), runtime.client());
        assert_ne!(old.enrollment(), runtime.enrollment());
        assert_ne!(old.schema(), runtime.schema());
        assert_ne!(old.directory(), runtime.directory());
        assert_ne!(old.socket(), runtime.socket());
        assert_eq!(
            old.client(),
            "/usr/lib/omavless-image/development-runtime-tests"
        );
        assert_eq!(runtime.client(), "/usr/bin/omavless");
    }
}
