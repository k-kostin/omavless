// SPDX-License-Identifier: MIT
//! Closed development classes. Selection is not permission or an effect proof.

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Class {
    Tests,
    InstalledRuntime,
    #[cfg(feature = "product-epochs")]
    Product,
}

impl Class {
    pub(crate) const fn client(self) -> &'static str {
        match self {
            Self::Tests => "/usr/lib/omavless-image/development-runtime-tests",
            Self::InstalledRuntime => "/usr/bin/omavless",
            #[cfg(feature = "product-epochs")]
            Self::Product => "/usr/bin/omavless",
        }
    }

    pub(crate) const fn enrollment(self) -> &'static str {
        match self {
            Self::Tests => "/var/lib/omavless-image/development-enrollment-v1",
            Self::InstalledRuntime => "/var/lib/omavless-image/development-runtime-enrollment-v1",
            #[cfg(feature = "product-epochs")]
            Self::Product => "/var/lib/omavless-image-product/runtime.enrollment",
        }
    }

    pub(crate) const fn schema(self) -> &'static str {
        match self {
            Self::Tests => "omavless-development-current-image-v1",
            Self::InstalledRuntime => "omavless-development-installed-runtime-current-image-v1",
            #[cfg(feature = "product-epochs")]
            Self::Product => "omavless-product-current-image-v1",
        }
    }

    pub(crate) const fn directory(self) -> &'static str {
        match self {
            Self::Tests => "/run/omavless-image",
            Self::InstalledRuntime => "/run/omavless-image-runtime",
            #[cfg(feature = "product-epochs")]
            Self::Product => "/run/omavless-image-product",
        }
    }

    pub(crate) const fn socket(self) -> &'static str {
        match self {
            Self::Tests => "/run/omavless-image/control.sock",
            Self::InstalledRuntime => "/run/omavless-image-runtime/control.sock",
            #[cfg(feature = "product-epochs")]
            Self::Product => "/run/omavless-image-product/control.sock",
        }
    }

    pub(crate) const fn product(self) -> bool {
        match self {
            #[cfg(feature = "product-epochs")]
            Self::Product => true,
            _ => false,
        }
    }

    pub(crate) fn admits_uid(self, uid: u32) -> bool {
        uid != 0 && uid != u32::MAX && (self.product() || uid == 1000)
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

    #[cfg(feature = "product-epochs")]
    #[test]
    fn product_root_record_not_caller_uid_selects_a_distinct_class() {
        let class = Class::Product;
        for development in [Class::Tests, Class::InstalledRuntime] {
            assert_ne!(class.enrollment(), development.enrollment());
            assert_ne!(class.schema(), development.schema());
            assert_ne!(class.directory(), development.directory());
            assert_ne!(class.socket(), development.socket());
            assert!(!development.admits_uid(1001));
            assert!(!development.product());
        }
        assert!(class.admits_uid(1000));
        assert!(class.admits_uid(1001));
        assert!(!class.admits_uid(0));
        assert!(!class.admits_uid(u32::MAX));
        assert_eq!(class.client(), "/usr/bin/omavless");
    }
}
