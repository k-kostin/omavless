// SPDX-License-Identifier: MIT

use omavless_mihomo::{ErrorKind, parse_controller_response};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Corpus {
    version: u64,
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Case {
    id: String,
    status_line: String,
    headers: Vec<String>,
    body: String,
    outcome: String,
    status: Option<u16>,
}

#[test]
fn credential_free_controller_response_corpus() {
    let corpus: Corpus = serde_json::from_str(include_str!(
        "../../../tests/parity_cases/mihomo-controller-response-v1.json"
    ))
    .expect("valid corpus");
    assert_eq!(corpus.version, 1);
    assert!(corpus.cases.len() >= 12);
    for case in corpus.cases {
        let mut encoded = case.status_line.into_bytes();
        encoded.extend_from_slice(b"\r\n");
        for header in case.headers {
            encoded.extend_from_slice(header.as_bytes());
            encoded.extend_from_slice(b"\r\n");
        }
        encoded.extend_from_slice(b"\r\n");
        encoded.extend_from_slice(case.body.as_bytes());
        match case.outcome.as_str() {
            "ok" => {
                let response = parse_controller_response(&encoded)
                    .unwrap_or_else(|error| panic!("{} should pass: {error}", case.id));
                assert_eq!(
                    response.status,
                    case.status.expect("expected status"),
                    "{}",
                    case.id
                );
            }
            "invalid_response" => assert_eq!(
                parse_controller_response(&encoded)
                    .unwrap_err_or_else(|| panic!("{} should fail", case.id))
                    .kind(),
                ErrorKind::InvalidResponse,
                "{}",
                case.id
            ),
            other => panic!("unknown corpus outcome {other}"),
        }
    }
}

trait UnwrapErrOrElse<T, E> {
    fn unwrap_err_or_else(self, failed: impl FnOnce() -> E) -> E;
}

impl<T, E> UnwrapErrOrElse<T, E> for Result<T, E> {
    fn unwrap_err_or_else(self, failed: impl FnOnce() -> E) -> E {
        match self {
            Ok(_) => failed(),
            Err(error) => error,
        }
    }
}
