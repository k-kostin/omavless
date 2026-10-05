// SPDX-License-Identifier: MIT
//! Synthetic, credential-free IPC projections. Never accepted as live evidence.
use omavless_tui::client::Read;
use serde_json::{Value, json};

pub fn response(request: Read) -> Value {
    let result = match request {
        Read::Hello => {
            json!({"instanceId":"fixture-runtime", "version":1, "runtimeOwnership":true})
        }
        Read::Capabilities => {
            json!({"runtimeOwnership":true,"methods":["ui.snapshot","runtime.observation","runtime.traffic","diagnostics.summary","diagnostics.rules","diagnostics.providers","diagnostics.export"]})
        }
        Read::Snapshot => json!({
            "schemaVersion":1,"scope":"private_ui_metadata","instanceId":"fixture-runtime",
            "transition":null,"desired":{"connected":true,"profileId":"fixture-a","mode":"rule","generation":5},
            "lastKnownActual":"connected","healthFresh":false,"liveHealth":"unavailable",
            "profiles":[
                {"id":"fixture-a","name":"Fixture Helsinki","protocol":"vless","subscriptionId":null,"missing":false,"favorite":true},
                {"id":"fixture-b","name":"Fixture Frankfurt","protocol":"vless","subscriptionId":"fixture-sub","missing":false,"favorite":false}
            ],"subscriptions":[{"id":"fixture-sub","name":"Fixture subscription"}]
        }),
        Read::Traffic => {
            json!({"schemaVersion":1,"scope":"controller_attributed_tun_counters","availability":"observed",
            "sample":{"identity":"a".repeat(64),"rxBytes":4096,"txBytes":8192,"sampledAtMs":1000}})
        }
        Read::Diagnostics => json!({"version":1,"rules":{"total":42},"providers":{"total":2}}),
        Read::Rules => json!({"version":1,"rules":{"total":3,"shown":3,"truncated":false,"items":[
            {"type":"DOMAIN","payload":"example.invalid","target":"VPN"},
            {"type":"IP-CIDR","payload":"192.0.2.0/24","target":"DIRECT"},
            {"type":"MATCH","payload":"","target":"REJECT"}
        ]}}),
        Read::Providers => {
            json!({"version":1,"providers":{"total":2,"shown":2,"truncated":false,"items":[
                {"name":"fixture-domains","behavior":"domain","updatedAt":"2026-09-28","ruleCount":12,"status":"loaded","refreshable":true},
                {"name":"fixture-empty","behavior":"ipcidr","updatedAt":"","ruleCount":0,"status":"empty","refreshable":false}
            ]}})
        }
        Read::CustomRules => json!({"version":1,"rules":[
            {"id":"fixture-rule-1","kind":"domain","value":"fixture.invalid","action":"proxy"},
            {"id":"fixture-rule-2","kind":"ipcidr","value":"192.0.2.0/24","action":"direct"}
        ]}),
        Read::HostSupport => json!({
            "schemaVersion":3,"scope":"native_support",
            "coverage":{"coreSetupVerified":true,"serviceEnablementVerified":true,
                "fileReadiness":true,"loadedPolicyCounts":false},
            "host":{"core":{"installed":true,"fileNetworkCapabilities":true,"tunDevicePresent":true},
                "runtimeService":{"loaded":true,"active":true,"enabled":false,"ownsCurrentProcess":true},
                "loginService":{"loaded":true,"active":false,"enabled":false,"ownsCurrentProcess":null},
                "files":{"store":true,"template":true,"generatedConfig":true,"runtimeUnit":true,"loginUnit":true},
                "configuredPolicy":null}
        }),
        Read::ProfileDetails(_) => {
            json!({"version":1,"protocol":"vless","transport":"xhttp","security":"reality","server":"fixture.invalid:443","sni":"fixture.invalid"})
        }
        Read::SubscriptionUsage(_) => {
            json!({"schemaVersion":1,"scope":"private_provider_reported_usage",
            "instanceId":"fixture-runtime","availability":"reported","usage":{
            "uploadBytes":"1073741824","downloadBytes":"2147483648","totalBytes":"10737418240","expiryUnixSeconds":"1893456000"}})
        }
        Read::Connections => {
            json!({"schemaVersion":1,"scope":"owned_core_active_connection_count","availability":"observed","count":3,"instanceId":"fixture-runtime"})
        }
        Read::ConnectionOverview => json!({
            "schemaVersion":1,"scope":"owned_core_connection_categories","availability":"observed","total":3,
            "network":{"tcp":2,"udp":1,"other":0},
            "outcome":{"direct":1,"blocked":0,"vpn":2,"unclassified":0},
            "instanceId":"fixture-runtime"
        }),
        Read::ConnectionRows => json!({
            "schemaVersion":1,"scope":"owned_core_private_connection_rows","availability":"observed",
            "total":2,"shown":2,"truncated":false,
            "rows":[
                {"host":"example.invalid","ip":"203.0.113.8","port":443,"network":"tcp","route":"vpn"},
                {"host":null,"ip":"192.0.2.2","port":53,"network":"udp","route":"direct"}
            ],"instanceId":"fixture-runtime"
        }),
        Read::RouteCheck(target) => json!({
            "version":1,"query":target.as_str(),"source":"custom","outcome":"vpn",
            "ruleType":"DOMAIN","rulePayload":"example.invalid","target":"PROXY"
        }),
        Read::Observation => json!({
            "schemaVersion":1,"scope":"local_runtime_observation","instanceId":"fixture-runtime",
            "transition":null,"availability":"observed",
            "desired":{"connected":true,"mode":"rule","generation":5},"lastKnownActual":"connected",
            "manualRecoveryRequired":false,
            "facts":{"ownedCoreRunning":true,"visibleMihomoCount":1,"ownedAuxiliaryMihomoCount":0,
                "visibleTunCount":1,"managedTunCount":1,"ownedControllerConfigVerified":true,"desiredProfileMatchesOwned":true},
            "verification":{"serviceOwnership":false,"tunOwnership":false,"routes":false,"dns":false,"internet":false}
        }),
    };
    json!({"api":"omavless.control","version":1,"id":"fixture-request","ok":true,"revision":7,"result":result})
}
