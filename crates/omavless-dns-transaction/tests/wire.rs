// SPDX-License-Identifier: MIT
use omavless_dns_transaction::wire::*;

const EPOCH: &str = "11111111111111111111111111111111";
const LEASE: &str = "22222222222222222222222222222222";

fn frame(method: &str, params: &str) -> Vec<u8> {
    format!("{{\"api\":\"omavless.dns\",\"version\":1,\"id\":\"r1\",\"method\":\"{method}\",\"params\":{params}}}\n").into_bytes()
}

fn params() -> String {
    format!("{{\"epoch\":\"{EPOCH}\",\"leaseId\":\"{LEASE}\",\"operationId\":\"op-1\"}}")
}

#[test]
fn all_methods_have_one_exact_shape() {
    for (name, method, value) in [
        ("hello", Method::Hello, "{}".to_owned()),
        ("status", Method::Status, "{}".to_owned()),
        (
            "acquire",
            Method::Acquire,
            format!("{{\"epoch\":\"{EPOCH}\",\"operationId\":\"op-1\"}}"),
        ),
        ("apply", Method::Apply, params()),
        ("release", Method::Release, params()),
        (
            "verify",
            Method::Verify,
            format!("{{\"epoch\":\"{EPOCH}\",\"leaseId\":\"{LEASE}\"}}"),
        ),
    ] {
        let request = decode(&frame(name, &value)).unwrap();
        assert_eq!(request.method(), method);
        assert_eq!(request.id(), "r1");
    }
}

#[test]
fn max_legal_request_and_operation_ids() {
    let mut value = String::from_utf8(frame("apply", &params())).unwrap();
    value = value
        .replace("r1", &"r".repeat(64))
        .replace("op-1", &"o".repeat(64));
    let request = decode(value.as_bytes()).unwrap();
    assert_eq!(request.id().len(), 64);
    assert_eq!(request.operation().unwrap().len(), 64);
    assert_eq!(request.epoch(), Some(EPOCH));
    assert_eq!(request.lease(), Some(LEASE));
}

#[test]
fn exact_frame_bound_includes_newline() {
    let base = frame("hello", "{}");
    let mut padded = vec![b' '; MAX_FRAME - base.len()];
    padded.extend_from_slice(&base);
    assert!(decode(&padded).is_ok());
    padded.insert(0, b' ');
    assert_eq!(decode(&padded).unwrap_err(), WireError::InvalidFrame);
}

#[test]
fn missing_newline_extra_frame_and_crlf_refuse() {
    let base = frame("hello", "{}");
    for bytes in [
        vec![],
        vec![b'\n'],
        base[..base.len() - 1].to_vec(),
        [base.clone(), base.clone()].concat(),
        [base.clone(), vec![b' ']].concat(),
        [base[..base.len() - 1].to_vec(), b"\r\n".to_vec()].concat(),
    ] {
        assert!(decode(&bytes).is_err());
    }
}

#[test]
fn invalid_utf8_and_escaped_surrogate_refuse() {
    assert_eq!(decode(b"{\xff}\n").unwrap_err(), WireError::InvalidFrame);
    let value = String::from_utf8(frame("hello", "{}"))
        .unwrap()
        .replace("r1", "\\ud800");
    assert!(decode(value.as_bytes()).is_err());
}

#[test]
fn nested_duplicate_keys_and_escape_aliases_refuse() {
    for params in [
        format!("{{\"epoch\":\"{EPOCH}\",\"epoch\":\"{EPOCH}\"}}"),
        format!("{{\"operationId\":\"one\",\"operation\\u0049d\":\"two\",\"epoch\":\"{EPOCH}\"}}"),
        format!("{{\"leaseId\":\"{LEASE}\",\"leaseId\":\"{LEASE}\"}}"),
    ] {
        assert!(decode(&frame("acquire", &params)).is_err());
    }
}

#[test]
fn duplicate_top_level_keys_refuse() {
    let valid = String::from_utf8(frame("hello", "{}")).unwrap();
    for duplicate in [
        "\"id\":\"other\",",
        "\"method\":\"status\",",
        "\"version\":1,",
        "\"params\":{},",
    ] {
        let value = valid.replacen('{', &format!("{{{duplicate}"), 1);
        assert!(decode(value.as_bytes()).is_err());
    }
}

#[test]
fn scalar_array_trailing_json_and_deep_containers_refuse() {
    for input in ["null\n", "[]\n", "true\n", "1\n", "{} {}\n"] {
        assert!(decode(input.as_bytes()).is_err());
    }
    assert!(
        decode(&frame(
            "hello",
            &format!("{}{}", "[".repeat(200), "]".repeat(200))
        ))
        .is_err()
    );
    assert!(decode(&frame("apply", "{\"epoch\":{\"nested\":{}}}")).is_err());
    assert!(decode(b"[\"omavless.dns\",1,\"r1\",\"hello\",{}]\n").is_err());
}

#[test]
fn no_missing_fields_or_wrong_types() {
    for key in ["api", "version", "id", "method", "params"] {
        let mut value: serde_json::Value = serde_json::from_slice(&frame("hello", "{}")).unwrap();
        value.as_object_mut().unwrap().remove(key);
        let bytes = format!("{value}\n");
        assert!(decode(bytes.as_bytes()).is_err());
    }
    for params in [
        "null",
        "[]",
        "true",
        "\"text\"",
        "{\"epoch\":null}",
        "{\"operationId\":null}",
        "{\"leaseId\":null}",
    ] {
        assert!(decode(&frame("hello", params)).is_err());
    }
}

#[test]
fn version_mismatch_does_not_fall_back() {
    let valid = String::from_utf8(frame("hello", "{}")).unwrap();
    for version in ["0", "2", "18446744073709551615"] {
        let value = valid.replace("\"version\":1", &format!("\"version\":{version}"));
        assert_eq!(
            decode(value.as_bytes()).unwrap_err(),
            WireError::UnsupportedVersion
        );
    }
    for version in ["true", "-1", "1.0", "\"1\"", "null"] {
        let value = valid.replace("\"version\":1", &format!("\"version\":{version}"));
        assert!(decode(value.as_bytes()).is_err());
    }
}

#[test]
fn runtime_api_is_not_broker_api() {
    let value = String::from_utf8(frame("hello", "{}"))
        .unwrap()
        .replace("omavless.dns", "omavless.control");
    assert_eq!(
        decode(value.as_bytes()).unwrap_err(),
        WireError::InvalidRequest
    );
}

#[test]
fn no_command_path_interface_uid_dns_or_policy_injection() {
    for key in [
        "interface",
        "ifindex",
        "uid",
        "pid",
        "unit",
        "command",
        "path",
        "dns",
        "domains",
        "policy",
        "timeout",
    ] {
        let value = format!("{{\"{key}\":\"private-sentinel\"}}");
        assert!(decode(&frame("acquire", &value)).is_err());
        let bytes = String::from_utf8(frame("hello", "{}")).unwrap().replacen(
            '{',
            &format!("{{\"{key}\":true,"),
            1,
        );
        assert!(decode(bytes.as_bytes()).is_err());
    }
    for method in [
        "exec",
        "sudo",
        "reset-all",
        "revert",
        "enroll",
        "recover",
        "set-dns",
        "connect",
    ] {
        assert_eq!(
            decode(&frame(method, "{}")).unwrap_err(),
            WireError::UnknownMethod
        );
    }
}

#[test]
fn methods_do_not_accept_other_methods_parameters() {
    for method in ["hello", "status", "acquire", "verify"] {
        assert_eq!(
            decode(&frame(method, &params())).unwrap_err(),
            WireError::InvalidParams
        );
    }
    for method in ["apply", "release", "verify", "acquire"] {
        assert_eq!(
            decode(&frame(method, "{}")).unwrap_err(),
            WireError::InvalidParams
        );
    }
}

#[test]
fn lease_and_epoch_are_bounded_exact_hex_not_paths() {
    for bad in [
        "",
        "a",
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "../../private",
        "111111111111111111111111111111111",
    ] {
        for reference in [EPOCH, LEASE] {
            assert!(decode(&frame("apply", &params().replace(reference, bad))).is_err());
        }
    }
}

#[test]
fn id_and_operation_charset_length_limits() {
    for bad in [
        "".to_owned(),
        "a".repeat(65),
        "тест".to_owned(),
        "uri://private".to_owned(),
        "contains space".to_owned(),
    ] {
        let value = String::from_utf8(frame("apply", &params())).unwrap();
        for target in ["r1", "op-1"] {
            assert!(decode(value.replace(target, &bad).as_bytes()).is_err());
        }
    }
}

#[test]
fn errors_never_echo_input_or_json_parser_fragments() {
    for input in [
        b"{password-private-sentinel}\n".as_slice(),
        b"{\"key\":\"private-sentinel\"}\n",
        b"{\"uri\":\"vless://private-sentinel\"}\n",
    ] {
        let error = decode(input).unwrap_err();
        for text in [
            error.to_string(),
            format!("{error:?}"),
            String::from_utf8(error.encode()).unwrap(),
        ] {
            assert!(!text.contains("sentinel") && !text.contains("vless://") && text.len() < 160);
        }
    }
}

#[test]
fn accepted_request_debug_does_not_publish_references() {
    let request = decode(&frame("apply", &params())).unwrap();
    let text = format!("{request:?}");
    for forbidden in [EPOCH, LEASE, "op-1", "r1"] {
        assert!(!text.contains(forbidden));
    }
}

#[test]
fn error_envelopes_are_fixed_bounded_ndjson() {
    for error in [
        WireError::InvalidFrame,
        WireError::InvalidRequest,
        WireError::UnsupportedVersion,
        WireError::UnknownMethod,
        WireError::InvalidParams,
    ] {
        let bytes = error.encode();
        assert!(bytes.ends_with(b"\n") && bytes.len() < 160);
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["id"], serde_json::Value::Null);
        assert_eq!(value["code"], error.code());
        assert_eq!(value["ok"], false);
    }
}
