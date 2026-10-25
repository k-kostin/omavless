// SPDX-License-Identifier: MIT
//! Pure, window-local ordering of an already validated private projection.
//! No cloned row cache, controller request, mutation handle or persisted target.
use crate::inspection::{ConnectionRow, ConnectionRows};
use std::cmp::{Ordering, Reverse};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NetworkFilter {
    #[default]
    All,
    Tcp,
    Udp,
    Other,
}
impl NetworkFilter {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::All => Self::Tcp,
            Self::Tcp => Self::Udp,
            Self::Udp => Self::Other,
            Self::Other => Self::All,
        }
    }
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::All => "tui.connection_filter_all",
            Self::Tcp => "tui.connection_filter_tcp",
            Self::Udp => "tui.connection_filter_udp",
            Self::Other => "tui.connection_filter_other",
        }
    }
    pub(crate) fn includes(self, network: &str) -> bool {
        match self {
            Self::All => true,
            Self::Tcp => network == "tcp",
            Self::Udp => network == "udp",
            Self::Other => network == "other",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RouteFilter {
    #[default]
    All,
    Direct,
    Blocked,
    Proxy,
    Unclassified,
}
impl RouteFilter {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::All => Self::Direct,
            Self::Direct => Self::Blocked,
            Self::Blocked => Self::Proxy,
            Self::Proxy => Self::Unclassified,
            Self::Unclassified => Self::All,
        }
    }
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::All => "tui.connection_filter_all",
            Self::Direct => "tui.connection_route_direct",
            Self::Blocked => "tui.connection_route_blocked",
            Self::Proxy => "tui.connection_route_vpn",
            Self::Unclassified => "tui.connection_unclassified",
        }
    }
    pub(crate) fn includes(self, route: &str) -> bool {
        match self {
            Self::All => true,
            Self::Direct => route == "direct",
            Self::Blocked => route == "blocked",
            Self::Proxy => route == "vpn",
            Self::Unclassified => route == "unclassified",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Order {
    #[default]
    Original,
    Destination,
    Network,
    Route,
    Download,
    Upload,
}
impl Order {
    pub fn next(self) -> Self {
        match self {
            Self::Original => Self::Destination,
            Self::Destination => Self::Network,
            Self::Network => Self::Route,
            Self::Route => Self::Download,
            Self::Download => Self::Upload,
            Self::Upload => Self::Original,
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Original => "tui.connection_order_original",
            Self::Destination => "tui.connection_order_destination",
            Self::Network => "tui.connection_order_network",
            Self::Route => "tui.connection_order_route",
            Self::Download => "tui.connection_order_download",
            Self::Upload => "tui.connection_order_upload",
        }
    }
}
fn network(row: &ConnectionRow) -> u8 {
    match row.network {
        "tcp" => 0,
        "udp" => 1,
        _ => 2,
    }
}
fn route(row: &ConnectionRow) -> u8 {
    match row.route {
        "vpn" => 0,
        "direct" => 1,
        "blocked" => 2,
        _ => 3,
    }
}
fn destination(a: &ConnectionRow, b: &ConnectionRow) -> Ordering {
    let order = match (
        a.host.as_deref().or(a.ip.as_deref()),
        b.host.as_deref().or(b.ip.as_deref()),
    ) {
        (Some(a), Some(b)) => a
            .bytes()
            .map(|c| c.to_ascii_lowercase())
            .cmp(b.bytes().map(|c| c.to_ascii_lowercase())),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    };
    order
        .then_with(|| a.port.is_none().cmp(&b.port.is_none()))
        .then_with(|| a.port.cmp(&b.port))
}

/// Filter only the received bounded rows. Sorting borrows them for this render;
/// the canonical snapshot stays in its original order and is never copied here.
pub fn rows<'a>(
    projection: &'a ConnectionRows,
    query: &str,
    order: Order,
) -> Vec<&'a ConnectionRow> {
    let query = query.to_lowercase();
    let mut rows: Vec<_> = projection
        .rows
        .iter()
        .filter(|r| {
            query.is_empty()
                || r.host
                    .as_deref()
                    .is_some_and(|h| h.to_ascii_lowercase().contains(&query))
                || r.ip.as_deref().is_some_and(|ip| ip.contains(&query))
                || r.port.is_some_and(|p| p.to_string().contains(&query))
                || r.network.contains(&query)
                || r.route.contains(&query)
                || r.process_name
                    .as_deref()
                    .is_some_and(|name| name.to_lowercase().contains(&query))
                || r.rule_type
                    .is_some_and(|kind| kind.to_ascii_lowercase().contains(&query))
                || r.chain.as_ref().is_some_and(|chain| {
                    chain
                        .iter()
                        .any(|label| label.to_lowercase().contains(&query))
                })
        })
        .collect();
    // Stable sorts keep indistinguishable rows in their received order.
    match order {
        Order::Original => (),
        Order::Destination => rows.sort_by(|a, b| destination(a, b)),
        Order::Network => {
            rows.sort_by(|a, b| network(a).cmp(&network(b)).then_with(|| destination(a, b)))
        }
        Order::Route => {
            rows.sort_by(|a, b| route(a).cmp(&route(b)).then_with(|| destination(a, b)))
        }
        // Optional validated integer counters: descending puts known zero
        // before unknown. Equal values (including None) retain received order.
        Order::Download => rows.sort_by_key(|r| Reverse(r.download_bytes)),
        Order::Upload => rows.sort_by_key(|r| Reverse(r.upload_bytes)),
    }
    rows
}
