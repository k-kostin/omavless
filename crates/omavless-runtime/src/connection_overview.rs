// SPDX-License-Identifier: MIT
//! Categorical, count-only active-connection projection. Raw destinations,
//! processes, connection IDs and chain names never cross the owner IPC boundary.
use crate::mutation_protocol::MutationProtocolError;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnectionOverview {
    pub total: u32,
    pub tcp: u32,
    pub udp: u32,
    pub other_network: u32,
    pub direct: u32,
    pub blocked: u32,
    pub vpn: u32,
    pub unclassified: u32,
}

pub(crate) fn validate(request: &Value) -> Result<(), MutationProtocolError> {
    omavless_control_protocol::validate_request(request)
        .map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "runtime.connection_overview" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    if !request["params"].as_object().is_some_and(|p| p.is_empty()) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    Ok(())
}

pub(crate) fn aggregate(payload: &Value) -> Option<ConnectionOverview> {
    let mut overview = ConnectionOverview::default();
    let rows = match payload.get("connections")? {
        Value::Null => return Some(overview),
        Value::Array(rows) if rows.len() <= crate::connections_summary::MAX_CONNECTIONS => rows,
        _ => return None,
    };
    overview.total = rows.len() as u32;
    for row in rows {
        let object = row.as_object()?;
        let metadata = match object.get("metadata") {
            Some(Value::Object(metadata)) => Some(metadata),
            None | Some(Value::Null) => None,
            _ => return None,
        };
        match metadata.and_then(|metadata| metadata.get("network")) {
            Some(Value::String(network)) if network.eq_ignore_ascii_case("tcp") => {
                overview.tcp += 1
            }
            Some(Value::String(network)) if network.eq_ignore_ascii_case("udp") => {
                overview.udp += 1
            }
            None | Some(Value::Null) => overview.other_network += 1,
            Some(Value::String(network)) if network.len() <= 32 => overview.other_network += 1,
            _ => return None,
        }
        let chains = match object.get("chains") {
            Some(Value::Array(chains))
                if chains.len() <= 16
                    && chains
                        .iter()
                        .all(|v| v.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 256)) =>
            {
                chains
            }
            None | Some(Value::Null) => {
                overview.unclassified += 1;
                continue;
            }
            _ => return None,
        };
        if chains.len() == 1 && chains[0] == "DIRECT" {
            overview.direct += 1;
        } else if chains.len() == 1 && matches!(chains[0].as_str(), Some("REJECT" | "REJECT-DROP"))
        {
            overview.blocked += 1;
        } else if chains.iter().any(|v| v == "PROXY")
            && !chains
                .iter()
                .any(|v| matches!(v.as_str(), Some("DIRECT" | "REJECT" | "REJECT-DROP")))
        {
            overview.vpn += 1;
        } else {
            overview.unclassified += 1;
        }
    }
    Some(overview)
}

pub(crate) fn project(overview: Option<ConnectionOverview>) -> Value {
    match overview {
        Some(o) => json!({
            "schemaVersion":1,
            "scope":"owned_core_connection_categories",
            "availability":"observed",
            "total":o.total,
            "network":{"tcp":o.tcp,"udp":o.udp,"other":o.other_network},
            "outcome":{"direct":o.direct,"blocked":o.blocked,"vpn":o.vpn,"unclassified":o.unclassified}
        }),
        None => json!({
            "schemaVersion":1,
            "scope":"owned_core_connection_categories",
            "availability":"unavailable",
            "total":null,
            "network":null,
            "outcome":null
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_categories_survive_private_controller_rows() {
        let payload = json!({"connections":[
            {"id":"private-id","metadata":{"host":"private.invalid","process":"secret","network":"tcp"},"chains":["private-node","PROXY"]},
            {"metadata":{"network":"udp","host":"secret.invalid"},"chains":["DIRECT"]},
            {"metadata":{"network":"tcp"},"chains":["REJECT-DROP"]},
            {"metadata":{"network":"unknown"},"chains":["private-chain"]}
        ]});
        let value = project(aggregate(&payload));
        assert_eq!(value["total"], 4);
        assert_eq!(value["network"], json!({"tcp":2,"udp":1,"other":1}));
        assert_eq!(
            value["outcome"],
            json!({"vpn":1,"direct":1,"blocked":1,"unclassified":1})
        );
        for private in [
            "private-id",
            "private.invalid",
            "secret",
            "private-chain",
            "private-node",
        ] {
            assert!(!value.to_string().contains(private));
        }
    }
    #[test]
    fn malformed_or_oversized_input_never_becomes_a_partial_overview() {
        for payload in [
            json!({}),
            json!({"connections":true}),
            json!({"connections":[null]}),
            json!({"connections":[{"metadata":{"network":42},"chains":["DIRECT"]}]}),
            json!({"connections":[{"chains":[false]}]}),
            json!({"connections":[{"chains":vec!["PROXY";17]}]}),
            json!({"connections":vec![json!({});crate::connections_summary::MAX_CONNECTIONS+1]}),
        ] {
            let result = project(aggregate(&payload));
            assert_eq!(result["availability"], "unavailable");
            assert!(result["total"].is_null());
        }
        assert_eq!(project(aggregate(&json!({"connections":null})))["total"], 0);
    }
    #[test]
    fn ambiguous_and_conflicting_chains_are_not_called_vpn() {
        let payload = json!({"connections":[
            {"chains":["private-node"]},
            {"chains":["DIRECT","PROXY"]},
            {"chains":["REJECT","PROXY"]},
            {"chains":["private-node","PROXY"]}
        ]});
        let value = project(aggregate(&payload));
        assert_eq!(value["total"], 4);
        assert_eq!(value["outcome"]["unclassified"], 3);
        assert_eq!(value["outcome"]["vpn"], 1);
        assert!(!value.to_string().contains("private-node"));
    }
    #[test]
    fn method_is_fixed_and_argument_free() {
        for params in [json!({}), json!({"id":"private"}), json!([])] {
            let request = json!({"api":"omavless.control","version":1,"id":"test","method":"runtime.connection_overview","params":params});
            assert_eq!(validate(&request).is_ok(), params == json!({}));
        }
    }
}
