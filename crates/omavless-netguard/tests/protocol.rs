use omavless_netguard::protocol::*;

#[test]
fn request_roundtrips_cover_only_fixed_operations() {
    for request in [
        Request::Status {},
        Request::Arm {
            generation: 0,
            mode: Mode::Full,
        },
        Request::Arm {
            generation: u64::MAX,
            mode: Mode::Full,
        },
        Request::Disarm { generation: 42 },
    ] {
        assert_eq!(
            decode_request(&encode_request(request).unwrap()),
            Ok(request)
        );
    }
    assert_eq!(
        encode_request(Request::Status {}).unwrap(),
        br#"{"version":1,"payload":{"operation":"status"}}"#
    );
}

#[test]
fn malformed_duplicate_unknown_and_privileged_inputs_fail_closed() {
    for payload in [
        r#"{"operation":"status","operation":"status"}"#,
        r#"{"operation":"status","\u006fperation":"status"}"#,
        r#"{"operation":"status","generation":1}"#,
        r#"{"operation":"arm","generation":1,"generation":1,"mode":"full"}"#,
        r#"{"operation":"arm","generation":1,"mode":"full","mode":"full"}"#,
        r#"{"operation":"arm","generation":1,"mode":"routing"}"#,
        r#"{"operation":"arm","generation":1,"mode":null}"#,
        r#"{"operation":"arm","generation":1.0,"mode":"full"}"#,
        r#"{"operation":"arm","generation":1e0,"mode":"full"}"#,
        r#"{"operation":"arm","generation":true,"mode":"full"}"#,
        r#"{"operation":"arm","generation":-1,"mode":"full"}"#,
        r#"{"operation":"arm","generation":18446744073709551616,"mode":"full"}"#,
        r#"{"operation":"arm","generation":1}"#,
        r#"{"operation":"disarm","generation":1,"mode":"full"}"#,
        r#"{"operation":"reconcile"}"#,
        r#"{"operation":"recover"}"#,
        r#"["status"]"#,
        r#"null"#,
    ] {
        assert_eq!(
            decode_request(format!(r#"{{"version":1,"payload":{payload}}}"#).as_bytes()),
            Err(WireError::Invalid)
        );
    }
    for field in [
        "uid",
        "command",
        "path",
        "nft",
        "interface",
        "mark",
        "port",
        "address",
        "dns",
        "unit",
        "environment",
    ] {
        let input = format!(r#"{{"version":1,"payload":{{"operation":"status","{field}":null}}}}"#);
        assert_eq!(decode_request(input.as_bytes()), Err(WireError::Invalid));
    }
    for input in [
        r#"{"version":1,"version":1,"payload":{"operation":"status"}}"#,
        r#"{"version":1,"payload":{"operation":"status"},"payload":{"operation":"status"}}"#,
        r#"{"version":1,"payload":{"operation":"status"},"extra":{}}"#,
        r#"{"version":1,"payload":{"operation":"status"}} {}"#,
        r#"[1,{"operation":"status"}]"#,
        r#"{"version":1.0,"payload":{"operation":"status"}}"#,
        r#"{"version":1,"payload":{"operation":"status"},"\u0076ersion":1}"#,
    ] {
        assert_eq!(decode_request(input.as_bytes()), Err(WireError::Invalid));
    }
    assert_eq!(decode_request(b"\xff"), Err(WireError::Invalid));
    assert_eq!(
        decode_request(br#"{"version":2,"payload":{"operation":"status"}}"#),
        Err(WireError::UnsupportedVersion)
    );
}

#[test]
fn frame_bound_includes_whitespace_and_depth_is_bounded() {
    let mut request = encode_request(Request::Status {}).unwrap();
    request.resize(MAX_FRAME_BYTES, b' ');
    assert!(decode_request(&request).is_ok());
    request.push(b' ');
    assert_eq!(decode_request(&request), Err(WireError::TooLarge));
    let deep = format!(
        r#"{{"version":1,"payload":{{"operation":"status","x":{}{}}}}}"#,
        "[".repeat(1000),
        "]".repeat(1000)
    );
    assert!(decode_request(deep.as_bytes()).is_err());
    let objects = format!("{}0{}", "{\"x\":".repeat(1000), "}".repeat(1000));
    assert!(decode_request(objects.as_bytes()).is_err());
}

#[test]
fn all_one_byte_inputs_and_every_truncated_valid_frame_fail_safely() {
    for byte in 0..=255 {
        assert!(decode_request(&[byte]).is_err());
        assert!(decode_response(&[byte]).is_err());
    }
    let request = encode_request(Request::Arm {
        generation: u64::MAX,
        mode: Mode::Full,
    })
    .unwrap();
    for end in 0..request.len() {
        assert!(decode_request(&request[..end]).is_err());
    }
    // Safe fixed errors cannot echo any invalid input, even arbitrary private text.
    let error = decode_request(b"private malformed content").unwrap_err();
    assert_eq!(format!("{error:?}"), "Invalid");
}

#[test]
fn response_validation_rejects_forged_health_and_nested_duplicates() {
    for response in [
        Response::Status {
            policy_version: POLICY_VERSION,
            protection: Protection::Armed {
                generation: u64::MAX,
            },
            health: Health::Verified,
        },
        Response::Status {
            policy_version: POLICY_VERSION,
            protection: Protection::Disarmed {},
            health: Health::Verified,
        },
        Response::Status {
            policy_version: POLICY_VERSION,
            protection: Protection::Emergency {},
            health: Health::ManualRecoveryRequired,
        },
        Response::Error {
            code: ErrorCode::Unauthorized,
        },
    ] {
        assert_eq!(
            decode_response(&encode_response(response).unwrap()),
            Ok(response)
        );
    }
    for protection in [
        r#"{"state":"armed","generation":1,"generation":1}"#,
        r#"{"state":"disarmed","generation":1}"#,
        r#"{"state":"emergency"}"#,
        r#"{"state":"disarmed","endpoint":"private"}"#,
    ] {
        let input = format!(
            r#"{{"version":1,"payload":{{"result":"status","policy_version":1,"protection":{protection},"health":"verified"}}}}"#
        );
        assert_eq!(decode_response(input.as_bytes()), Err(WireError::Invalid));
    }
    assert!(
        encode_response(Response::Status {
            policy_version: POLICY_VERSION,
            protection: Protection::Emergency {},
            health: Health::Verified
        })
        .is_err()
    );
    assert!(decode_response(&vec![b' '; MAX_FRAME_BYTES + 1]).is_err());
}
