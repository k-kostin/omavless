//! Auditable symbolic policy, deliberately not executable nft syntax. Concrete
//! mark, TUN binding, hook priority and link-maintenance predicates require the
//! installed core/firewall review before an executor can be added.

pub const TABLE_FAMILY: &str = "inet";
pub const TABLE_NAME: &str = "omavless_netguard";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rule {
    AllowLoopback,
    AllowPackageOwnedTun,
    AllowPackageOwnedCoreMark,
    AllowNarrowDhcpV4,
    AllowNarrowDhcpV6,
    AllowNarrowIpv6NeighborDiscovery,
    DropAllOtherOutput,
}

/// No general established-flow, DNS, UID, physical-interface or LAN exception.
pub const FULL_VPN_RULES: &[Rule] = &[
    Rule::AllowLoopback,
    Rule::AllowPackageOwnedTun,
    Rule::AllowPackageOwnedCoreMark,
    Rule::AllowNarrowDhcpV4,
    Rule::AllowNarrowDhcpV6,
    Rule::AllowNarrowIpv6NeighborDiscovery,
    Rule::DropAllOtherOutput,
];

/// Corrupt/newer state cannot authorize even the normal bypass exceptions.
pub const EMERGENCY_RULES: &[Rule] = &[Rule::AllowLoopback, Rule::DropAllOtherOutput];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Policy {
    FullVpn,
    Emergency,
}

impl Policy {
    pub const fn rules(self) -> &'static [Rule] {
        match self {
            Self::FullVpn => FULL_VPN_RULES,
            Self::Emergency => EMERGENCY_RULES,
        }
    }
}
