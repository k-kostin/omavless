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
            json!({"runtimeOwnership":true,"methods":["ui.snapshot","runtime.observation"]})
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
