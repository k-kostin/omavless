// SPDX-License-Identifier: MIT
//! Read-only, credential-free operator projection of one runtime observation.
//! These local facts do not establish routes, DNS, Internet or VPN health.
use serde_json::{Value, json};

fn actual(value: &str) -> Option<&'static str> {
    Some(match value {
        "disconnected" => "disconnected",
        "starting" => "starting",
        "connected" => "connected",
        "reconnecting" => "reconnecting",
        "stopping" => "stopping",
        "failed" => "failed",
        "manualRecoveryRequired" => "manual_recovery_required",
        _ => return None,
    })
}

/// Discards all free-form response fields, including transition metadata and
/// any future private details. No aggregate health verdict is inferred.
pub fn project(response: &Value) -> Result<Value, &'static str> {
    let invalid = "OmaVLESS doctor observation is invalid";
    if response["ok"] != true {
        return Err("OmaVLESS doctor observation is unavailable");
    }
    let result = &response["result"];
    if result["schemaVersion"] != 1 || result["scope"] != "local_runtime_observation" {
        return Err(invalid);
    }
    let desired = result["desired"]["connected"].as_bool().ok_or(invalid)?;
    let actual = actual(result["lastKnownActual"].as_str().ok_or(invalid)?).ok_or(invalid)?;
    let recovery = result["manualRecoveryRequired"].as_bool().ok_or(invalid)?;
    if recovery != (actual == "manual_recovery_required") {
        return Err(invalid);
    }
    let availability = result["availability"].as_str().ok_or(invalid)?;
    let facts = &result["facts"];
    let local = match (availability, facts) {
        ("unavailable", Value::Null) => json!({
            "ownedCore":"unknown", "controllerConfiguration":"unknown",
            "desiredProfileMatchesOwned":"unknown", "tunScopeInventory":"unknown"
        }),
        ("observed", Value::Object(_)) => {
            let core = facts["ownedCoreRunning"].as_bool().ok_or(invalid)?;
            let controller = facts["ownedControllerConfigVerified"]
                .as_bool()
                .ok_or(invalid)?;
            let profile = facts["desiredProfileMatchesOwned"]
                .as_bool()
                .ok_or(invalid)?;
            let tun = facts["managedTunCount"]
                .as_u64()
                .filter(|n| *n <= u8::MAX as u64)
                .ok_or(invalid)?;
            json!({
                "ownedCore":if core { "running" } else { "stopped" },
                "controllerConfiguration":if controller { "verified_local" } else { "not_verified" },
                "desiredProfileMatchesOwned":if profile { "yes" } else { "no" },
                "tunScopeInventory":match tun { 0 => "none", 1 => "one", _ => "multiple" }
            })
        }
        _ => return Err(invalid),
    };
    Ok(json!({
        "schemaVersion":1, "scope":"local_read_only_doctor",
        "requested":if desired { "connected" } else { "disconnected" },
        "lastKnownActual":actual,
        "observation":availability,
        "manualRecoveryRequired":recovery,
        "localFacts":local,
        "tunOwnership":"not_proven",
        "networkHealth":"not_tested",
        "note":"Local inventory is not TUN ownership; routes, DNS and Internet are not tested"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_projection_never_emits_private_fields_or_health_claim() {
        let source = json!({"ok":true,"result":{
            "schemaVersion":1,"scope":"local_runtime_observation",
            "desired":{"connected":true,"profileId":"private-profile"},
            "lastKnownActual":"connected","manualRecoveryRequired":false,
            "availability":"observed",
            "facts":{"ownedCoreRunning":true,"ownedControllerConfigVerified":true,
                "desiredProfileMatchesOwned":true,"managedTunCount":1,
                "hostname":"private-host"},
            "transition":{"credential":"private-secret"}
        }});
        let report = project(&source).unwrap();
        let output = report.to_string();
        for private in ["private-profile", "private-host", "private-secret"] {
            assert!(!output.contains(private));
        }
        assert_eq!(report["networkHealth"], "not_tested");
        assert_eq!(report["localFacts"]["ownedCore"], "running");
        assert_eq!(report["localFacts"]["tunScopeInventory"], "one");
        assert_eq!(report["tunOwnership"], "not_proven");
        assert!(report.get("connected").is_none());
    }

    #[test]
    fn unavailable_is_unknown_even_when_cached_actual_says_connected() {
        let source = json!({"ok":true,"result":{
            "schemaVersion":1,"scope":"local_runtime_observation",
            "desired":{"connected":true},"lastKnownActual":"connected",
            "manualRecoveryRequired":false,"availability":"unavailable","facts":null
        }});
        let report = project(&source).unwrap();
        assert_eq!(report["localFacts"]["tunScopeInventory"], "unknown");
        assert_eq!(report["observation"], "unavailable");
        let mut inconsistent = source.clone();
        inconsistent["result"]["facts"] = json!({});
        assert!(project(&inconsistent).is_err());
    }

    #[test]
    fn malformed_or_unknown_values_fail_without_echoing_input() {
        let source = json!({"ok":true,"result":{
            "schemaVersion":1,"scope":"local_runtime_observation",
            "desired":{"connected":false},"lastKnownActual":"manualRecoveryRequired",
            "manualRecoveryRequired":true,"availability":"observed",
            "facts":{"ownedCoreRunning":false,"ownedControllerConfigVerified":false,
                "desiredProfileMatchesOwned":false,"managedTunCount":0}
        }});
        assert_eq!(project(&source).unwrap()["manualRecoveryRequired"], true);
        let mut many = source.clone();
        many["result"]["facts"]["managedTunCount"] = json!(255);
        assert_eq!(
            project(&many).unwrap()["localFacts"]["tunScopeInventory"],
            "multiple"
        );
        for pointer in [
            "/result/scope",
            "/result/lastKnownActual",
            "/result/availability",
            "/result/facts/ownedCoreRunning",
            "/result/facts/managedTunCount",
        ] {
            let mut changed = source.clone();
            *changed.pointer_mut(pointer).unwrap() = json!("private-invalid-value");
            let error = project(&changed).unwrap_err();
            assert!(!error.contains("private-invalid-value"));
        }
    }
}
