//! Closed developer-only identities; never deserialize a caller-selected path.
//! Sharing the validators does not transfer evidence between these fixtures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fixture {
    PrivateLifecycle,
    RetainedLease,
}

impl Fixture {
    pub(crate) const fn cgroup(self) -> &'static str {
        match self {
            Self::PrivateLifecycle => {
                "/sys/fs/cgroup/system.slice/omavless-k1-retained-private-lifecycle.service"
            }
            Self::RetainedLease => {
                "/sys/fs/cgroup/system.slice/omavless-k1-retained-lease-regression.service"
            }
        }
    }
    pub(crate) const fn unit_path(self) -> &'static str {
        match self {
            Self::PrivateLifecycle => {
                "/org/freedesktop/systemd1/unit/omavless_2dk1_2dretained_2dprivate_2dlifecycle_2eservice"
            }
            Self::RetainedLease => {
                "/org/freedesktop/systemd1/unit/omavless_2dk1_2dretained_2dlease_2dregression_2eservice"
            }
        }
    }
    pub(crate) const fn unit_sha(self) -> &'static str {
        match self {
            Self::PrivateLifecycle => {
                "198730a79751ccee045c6173d4cbb75db7ece33a784f5390f255cb65aa5e72b5"
            }
            Self::RetainedLease => {
                "f5d461381beeaf4846cc6113b2d81d2131ef28441cb33f6ee15362725527c78e"
            }
        }
    }
    pub(crate) const fn stage(self) -> &'static str {
        match self {
            Self::PrivateLifecycle => "/run/omavless-k1-retained-private-lifecycle",
            Self::RetainedLease => "/run/omavless-k1-retained-lease-regression",
        }
    }
    pub(crate) const fn unit(self) -> &'static str {
        match self {
            Self::PrivateLifecycle => "omavless-k1-retained-private-lifecycle.service",
            Self::RetainedLease => "omavless-k1-retained-lease-regression.service",
        }
    }
    pub(crate) const fn fragment(self) -> &'static str {
        match self {
            Self::PrivateLifecycle => {
                "/run/systemd/system/omavless-k1-retained-private-lifecycle.service"
            }
            Self::RetainedLease => {
                "/run/systemd/system/omavless-k1-retained-lease-regression.service"
            }
        }
    }
    pub(crate) const fn writer(self) -> &'static str {
        match self {
            Self::PrivateLifecycle => {
                "kernel_observer::creator_lifecycle::response_diagnostic::manager_private_lifecycle"
            }
            Self::RetainedLease => {
                "kernel_observer::creator_lifecycle::retained_lease::manager_retained_lease"
            }
        }
    }
    pub(crate) const fn environment(self) -> &'static str {
        match self {
            Self::PrivateLifecycle => "OMAVLESS_K1_RETAINED_LIFECYCLE_WRITER=1",
            Self::RetainedLease => "OMAVLESS_K1_RETAINED_LEASE_WRITER=1",
        }
    }
    pub(crate) const fn unit_bytes(self) -> &'static [u8] {
        match self {
            Self::PrivateLifecycle => {
                include_bytes!("../tests/fixtures/omavless-k1-retained-private-lifecycle.service")
            }
            Self::RetainedLease => {
                include_bytes!("../tests/fixtures/omavless-k1-retained-lease-regression.service")
            }
        }
    }
}

#[test]
fn fixed_identities_match_disjoint_original_units() {
    for fixture in [Fixture::PrivateLifecycle, Fixture::RetainedLease] {
        use sha2::{Digest, Sha256};
        assert_eq!(
            format!("{:x}", Sha256::digest(fixture.unit_bytes())),
            fixture.unit_sha()
        );
        let unit = std::str::from_utf8(fixture.unit_bytes()).unwrap();
        assert!(unit.contains(&format!(
            "ExecStart={}/probe --exact {} --ignored --nocapture --test-threads=1\n",
            fixture.stage(),
            fixture.writer()
        )));
        assert!(unit.contains(&format!("Environment={}\n", fixture.environment())));
        assert!(unit.contains(&format!(
            "StandardOutput=append:{}/native.stdout\n",
            fixture.stage()
        )));
        assert_eq!(
            fixture.fragment(),
            format!("/run/systemd/system/{}", fixture.unit())
        );
    }
    assert_ne!(
        Fixture::PrivateLifecycle.stage(),
        Fixture::RetainedLease.stage()
    );
    assert_ne!(
        Fixture::PrivateLifecycle.unit_bytes(),
        Fixture::RetainedLease.unit_bytes()
    );
}
