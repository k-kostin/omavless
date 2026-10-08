// SPDX-License-Identifier: MIT
//! Pure, window-local ordering of an already validated private projection.
//! No cloned row cache, controller request, mutation handle or persisted target.
use crate::inspection::{ConnectionRow, ConnectionRows};
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Order {
    #[default]
    Original,
    Destination,
    Network,
    Route,
}
impl Order {
    pub fn next(self) -> Self {
        match self {
            Self::Original => Self::Destination,
            Self::Destination => Self::Network,
            Self::Network => Self::Route,
            Self::Route => Self::Original,
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Original => "tui.connection_order_original",
            Self::Destination => "tui.connection_order_destination",
            Self::Network => "tui.connection_order_network",
            Self::Route => "tui.connection_order_route",
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
    let query = query.to_ascii_lowercase();
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
    }
    rows
}
