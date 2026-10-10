// SPDX-License-Identifier: MIT
//! Explicit, bounded private TUI projection of the owned core's connections.
//! This method is never part of ordinary status or support diagnostics.
use crate::mutation_protocol::MutationProtocolError;
use serde_json::{Value, json};
use std::net::IpAddr;

const MAX_ROWS: usize = 128;

#[derive(Clone)]
pub struct ConnectionRows {
    total: usize,
    rows: Vec<Value>,
}

impl std::fmt::Debug for ConnectionRows {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ConnectionRows([private])")
    }
}

pub(crate) fn validate(request: &Value) -> Result<(), MutationProtocolError> {
    omavless_control_protocol::validate_request(request)
        .map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "runtime.connection_rows" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    if !request["params"].as_object().is_some_and(|p| p.is_empty()) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    Ok(())
}

fn host(value: Option<&Value>) -> Option<&str> {
    let host = value?.as_str()?;
    if host.is_empty()
        || host.len() > 120
        || !host
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_'))
    {
        return None;
    }
    Some(host)
}

fn ip(value: Option<&Value>) -> Option<String> {
    let ip = value?.as_str()?.parse::<IpAddr>().ok()?;
    Some(ip.to_string())
}

fn port(value: Option<&Value>) -> Option<u16> {
    let value = value?;
    if let Some(number) = value.as_u64() {
        return u16::try_from(number).ok().filter(|port| *port != 0);
    }
    value
        .as_str()?
        .parse::<u16>()
        .ok()
        .filter(|port| *port != 0)
}

fn network(value: Option<&Value>) -> Option<&'static str> {
    match value {
        Some(Value::String(value)) if value.eq_ignore_ascii_case("tcp") => Some("tcp"),
        Some(Value::String(value)) if value.eq_ignore_ascii_case("udp") => Some("udp"),
        None | Some(Value::Null) => Some("other"),
        Some(Value::String(value)) if value.len() <= 32 => Some("other"),
        _ => None,
    }
}

fn route(value: Option<&Value>) -> Option<&'static str> {
    let Some(value) = value else {
        return Some("unclassified");
    };
    if value.is_null() {
        return Some("unclassified");
    }
    let chains = value.as_array()?;
    if chains.len() > 16
        || !chains
            .iter()
            .all(|c| c.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 256))
    {
        return None;
    }
    if chains.len() == 1 && chains[0] == "DIRECT" {
        Some("direct")
    } else if chains.len() == 1 && matches!(chains[0].as_str(), Some("REJECT" | "REJECT-DROP")) {
        Some("blocked")
    } else if chains.iter().any(|c| c == "PROXY")
        && !chains
            .iter()
            .any(|c| matches!(c.as_str(), Some("DIRECT" | "REJECT" | "REJECT-DROP")))
    {
        Some("vpn")
    } else {
        Some("unclassified")
    }
}

pub(crate) fn extract(payload: &Value) -> Option<ConnectionRows> {
    let rows = match payload.get("connections")? {
        Value::Null => {
            return Some(ConnectionRows {
                total: 0,
                rows: Vec::new(),
            });
        }
        Value::Array(rows) if rows.len() <= crate::connections_summary::MAX_CONNECTIONS => rows,
        _ => return None,
    };
    let mut projected = Vec::with_capacity(rows.len().min(MAX_ROWS));
    for (index, row) in rows.iter().enumerate() {
        let row = row.as_object()?;
        let metadata = match row.get("metadata") {
            Some(Value::Object(metadata)) => Some(metadata),
            None | Some(Value::Null) => None,
            _ => return None,
        };
        let get = |key| metadata.and_then(|m| m.get(key));
        let network = network(get("network"))?;
        let route = route(row.get("chains"))?;
        if index >= MAX_ROWS {
            continue;
        }
        projected.push(json!({
            "host": host(get("host")),
            "ip": ip(get("destinationIP")),
            "port": port(get("destinationPort")),
            "network": network,
            "route": route,
        }));
    }
    Some(ConnectionRows {
        total: rows.len(),
        rows: projected,
    })
}

pub(crate) fn project(rows: Option<ConnectionRows>) -> Value {
    match rows {
        Some(rows) => json!({
            "schemaVersion": 1,
            "scope": "owned_core_private_connection_rows",
            "availability": "observed",
            "total": rows.total,
            "shown": rows.rows.len(),
            "truncated": rows.total > rows.rows.len(),
            "rows": rows.rows,
        }),
        None => json!({
            "schemaVersion": 1,
            "scope": "owned_core_private_connection_rows",
            "availability": "unavailable",
            "total": null,
            "shown": 0,
            "truncated": false,
            "rows": [],
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_only_explicit_destination_fields_and_categories() {
        let payload = json!({"connections":[{"id":"secret-id","metadata":{
            "host":"example.org","destinationIP":"203.0.113.2","destinationPort":"443",
            "network":"tcp","process":"/private/app"},"chains":["private-node","PROXY"]}]});
        let result = project(extract(&payload));
        assert_eq!(
            result["rows"][0],
            json!({"host":"example.org","ip":"203.0.113.2","port":443,"network":"tcp","route":"vpn"})
        );
        for secret in ["secret-id", "private-node", "/private/app"] {
            assert!(!result.to_string().contains(secret));
        }
        let private = extract(&payload).unwrap();
        assert!(!format!("{private:?}").contains("example.org"));
    }

    #[test]
    fn unsafe_host_is_dropped_and_conflicting_chain_is_unclassified() {
        let result = project(extract(&json!({"connections":[{"metadata":{
            "host":"evil\ncontrol","destinationIP":"bad-ip","destinationPort":0},
            "chains":["DIRECT","PROXY"]}]})));
        assert!(result["rows"][0]["host"].is_null());
        assert!(result["rows"][0]["ip"].is_null());
        assert!(result["rows"][0]["port"].is_null());
        assert_eq!(result["rows"][0]["route"], "unclassified");
    }

    #[test]
    fn bounded_and_no_arguments() {
        let result = project(extract(&json!({"connections":vec![json!({});129]})));
        assert_eq!(result["total"], 129);
        assert_eq!(result["shown"], 128);
        assert_eq!(result["truncated"], true);
        for params in [json!({}), json!({"id":"secret"}), json!([])] {
            let request = json!({"api":"omavless.control","version":1,"id":"test","method":"runtime.connection_rows","params":params});
            assert_eq!(validate(&request).is_ok(), params == json!({}));
        }
    }

    #[test]
    fn malformed_later_rows_refuse_whole_projection() {
        let mut rows = vec![json!({}); 128];
        rows.push(json!({"metadata":{"network":false}}));
        assert_eq!(
            project(extract(&json!({"connections":rows})))["availability"],
            "unavailable"
        );
        assert_eq!(
            project(extract(
                &json!({"connections":vec![json!({});crate::connections_summary::MAX_CONNECTIONS+1]})
            ))["availability"],
            "unavailable"
        );
    }
}
