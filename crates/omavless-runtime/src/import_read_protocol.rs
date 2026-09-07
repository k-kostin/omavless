// SPDX-License-Identifier: MIT

//! Exact, private v1 import classification input. No caller-provided store,
//! duplicate flag, path, network instruction or mutation metadata is accepted.

use crate::mutation_protocol::{MutationProtocolError, exact_fields};
use omavless_control_protocol::{MAX_STRING_BYTES, validate_request};
use serde_json::Value;

pub const MAX_IMPORT_STDIN_BYTES: usize = MAX_STRING_BYTES;

pub struct ImportPreviewRequest<'a> {
    input: &'a str,
}

impl ImportPreviewRequest<'_> {
    pub(crate) fn private_input(&self) -> &str {
        self.input
    }
}

pub fn parse_import_preview_request(
    request: &Value,
) -> Result<ImportPreviewRequest<'_>, MutationProtocolError> {
    validate_request(request).map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "imports.classify" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    let params = request["params"]
        .as_object()
        .ok_or(MutationProtocolError::InvalidArgument)?;
    if !exact_fields(params, &["input"], &["input"]) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    let input = params["input"]
        .as_str()
        .filter(|value| !value.trim().is_empty() && value.len() <= MAX_IMPORT_STDIN_BYTES)
        .ok_or(MutationProtocolError::InvalidArgument)?;
    Ok(ImportPreviewRequest { input })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(params: Value) -> Value {
        json!({"api": "omavless.control", "version": 1,
               "id": "preview", "method": "imports.classify", "params": params})
    }

    #[test]
    fn exact_private_input_and_v1_string_bound() {
        let value = request(json!({"input": "x".repeat(MAX_IMPORT_STDIN_BYTES)}));
        assert_eq!(
            parse_import_preview_request(&value)
                .unwrap()
                .private_input()
                .len(),
            MAX_IMPORT_STDIN_BYTES
        );
        let value = request(json!({"input": "x".repeat(MAX_IMPORT_STDIN_BYTES + 1)}));
        assert!(parse_import_preview_request(&value).is_err());
        // JSON escaping can exceed the frame bound even within the string cap.
        let value = request(
            json!({"input": format!("x{}", "\u{0001}".repeat(MAX_IMPORT_STDIN_BYTES - 1))}),
        );
        assert!(parse_import_preview_request(&value).is_ok());
        assert!(omavless_control_protocol::encode_request(&value).is_err());
    }

    #[test]
    fn malformed_params_never_echo_input() {
        for params in [
            json!({}),
            json!({"input": null}),
            json!({"input": 1}),
            json!({"input": ""}),
            json!({"input": "  \n"}),
            json!({"input": "private-token", "duplicate": false}),
            json!({"input": "private-token", "operationId": "op"}),
            json!({"input": "private-token", "expectedRevision": 0}),
            json!({"input": "private-token", "path": "/tmp/input"}),
            json!({"input": "private-token", "urls": []}),
        ] {
            let error = parse_import_preview_request(&request(params))
                .err()
                .unwrap();
            assert!(!format!("{error:?} {error}").contains("private-token"));
        }
        let mut value = request(json!({"input": "private-token"}));
        value["method"] = json!("profiles.import");
        assert!(parse_import_preview_request(&value).is_err());
    }
}
