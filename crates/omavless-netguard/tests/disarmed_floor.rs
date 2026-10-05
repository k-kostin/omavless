use omavless_netguard::{policy::Policy, protocol::*, transaction::*};

fn disarmed(floor: Option<u64>) -> Response {
    Response::Status {
        policy_version: POLICY_VERSION,
        protection: Protection::Disarmed {
            closed_generation: floor,
        },
        health: Health::Verified,
    }
}

#[test]
fn exact_same_producer_preserves_missing_closed_and_completed_retry() {
    assert_eq!(
        observe(Observation {
            marker: Marker::Missing,
            table: Table::Absent
        }),
        disarmed(None)
    );
    for n in [0, 7, u64::MAX] {
        let state = Observation {
            marker: Marker::Closed(n),
            table: Table::Absent,
        };
        assert_eq!(observe(state), disarmed(Some(n)));
        assert_eq!(
            plan(Request::Disarm { generation: n }, state)
                .unwrap()
                .response()
                .unwrap(),
            disarmed(Some(n))
        );
        let mut recovery = reconcile(state).unwrap();
        while let Some(effect) = recovery.next_effect() {
            recovery.acknowledge(effect, true).unwrap();
        }
        assert_eq!(recovery.response().unwrap(), disarmed(Some(n)));
        let mut stop = plan(
            Request::Disarm { generation: n },
            Observation {
                marker: Marker::Armed(n),
                table: Table::OwnedVerified(Policy::FullVpn),
            },
        )
        .unwrap();
        while let Some(effect) = stop.next_effect() {
            stop.acknowledge(effect, true).unwrap();
        }
        assert_eq!(stop.response().unwrap(), disarmed(Some(n)));
    }
}

#[test]
fn v2_requires_explicit_null_or_exact_unsigned_floor_without_fallback() {
    assert_eq!(VERSION, 2);
    assert_eq!(
        decode_request(br#"{"version":1,"payload":{"operation":"status"}}"#),
        Err(WireError::UnsupportedVersion)
    );
    for floor in [None, Some(0), Some(7), Some(u64::MAX)] {
        let wire = encode_response(disarmed(floor)).unwrap();
        assert_eq!(decode_response(&wire), Ok(disarmed(floor)));
        let text = String::from_utf8(wire).unwrap();
        assert!(text.contains("\"closed_generation\":"));
        assert_eq!(
            decode_response(text.replace("\"version\":2", "\"version\":1").as_bytes()),
            Err(WireError::UnsupportedVersion)
        );
    }
    for protection in [
        r#"{"state":"disarmed"}"#,
        r#"{"state":"disarmed","closed_generation":true}"#,
        r#"{"state":"disarmed","closed_generation":-1}"#,
        r#"{"state":"disarmed","closed_generation":1.0}"#,
        r#"{"state":"disarmed","closed_generation":"7"}"#,
        r#"{"state":"disarmed","closed_generation":18446744073709551616}"#,
        r#"{"state":"disarmed","closed_generation":7,"closed_generation":7}"#,
        r#"{"state":"disarmed","closed_generation":7,"\u0063losed_generation":7}"#,
        r#"{"state":"armed","generation":7,"closed_generation":7}"#,
        r#"{"state":"emergency","closed_generation":null}"#,
    ] {
        let wire = format!(
            r#"{{"version":2,"payload":{{"result":"status","policy_version":1,"protection":{protection},"health":"verified"}}}}"#
        );
        assert_eq!(decode_response(wire.as_bytes()), Err(WireError::Invalid));
    }
}
