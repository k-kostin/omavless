// SPDX-License-Identifier: MIT

//! Confirmed new-profile import; private payload cannot be formatted.

use crate::import_read_protocol::MAX_IMPORT_STDIN_BYTES;
use crate::mutation::MutationDigest;
use crate::mutation_protocol::{MutationProtocolError, append_field, exact_fields, metadata};
use crate::profile_mutation_protocol::MAX_PROFILE_NAME_INPUT_BYTES;
use omavless_control_protocol::validate_request;
use serde_json::Value;

pub struct ProfileImportRequest {
    pub(crate) name: String,
    pub(crate) input: String,
    pub(crate) operation_id: Option<String>,
    pub(crate) expected_revision: Option<u64>,
    pub(crate) digest: MutationDigest,
}

/// Data admission is separate from connection permission. Native credentials
/// must fit the existing editor/IPC and survive canonical private reimport.
pub(crate) fn admitted_profile_input(input: &str) -> bool {
    use omavless_domain::private_store::PrivateProfileCredential;
    if input.len() > MAX_IMPORT_STDIN_BYTES {
        return false;
    }
    let Ok(profile) = PrivateProfileCredential::parse_input(input) else {
        return false;
    };
    if let PrivateProfileCredential::WireGuard(original) = &profile {
        let Ok(export) = profile.private_export() else {
            return false;
        };
        let Ok(text) = std::str::from_utf8(export.expose_private_bytes()) else {
            return false;
        };
        if text.len() > MAX_IMPORT_STDIN_BYTES {
            return false;
        }
        let Ok(PrivateProfileCredential::WireGuard(roundtrip)) =
            PrivateProfileCredential::parse_input(text)
        else {
            return false;
        };
        let (Ok(a), Ok(b)) = (original.private_record(), roundtrip.private_record()) else {
            return false;
        };
        if a.expose_private_bytes() != b.expose_private_bytes() {
            return false;
        }
        // Reserve the real maximum metadata/name envelope, not just string size.
        let request = serde_json::json!({"api":"omavless.control","version":1,
            "id":"\\".repeat(64),"method":"plugin.action","params":{
            "instanceId":"\\".repeat(128),"action":"profile-replace",
            "profileId":"00000000-0000-4000-8000-000000000001",
            "name":"\u{0001}".repeat(MAX_PROFILE_NAME_INPUT_BYTES),"input":text,
            "operationId":"\\".repeat(64),"expectedRevision":omavless_control_protocol::MAX_REVISION}});
        if omavless_control_protocol::encode_request(&request).is_err() {
            return false;
        }
    }
    true
}

pub fn parse_profile_import_request(
    request: &Value,
) -> Result<ProfileImportRequest, MutationProtocolError> {
    validate_request(request).map_err(|_| MutationProtocolError::InvalidRequest)?;
    omavless_control_protocol::encode_request(request)
        .map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "profiles.import" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    let params = request["params"]
        .as_object()
        .ok_or(MutationProtocolError::InvalidArgument)?;
    if !exact_fields(
        params,
        &["name", "input", "operationId", "expectedRevision"],
        &["name", "input"],
    ) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    let name = params["name"]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= MAX_PROFILE_NAME_INPUT_BYTES)
        .ok_or(MutationProtocolError::InvalidArgument)?;
    let input = params["input"]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= MAX_IMPORT_STDIN_BYTES)
        .ok_or(MutationProtocolError::InvalidArgument)?;
    let metadata = metadata(params)?;
    if !admitted_profile_input(input) {
        if let Some(summary) = crate::import_read_protocol::rejected_import(input) {
            return Err(MutationProtocolError::ImportRejected(summary));
        }
        return Err(MutationProtocolError::InvalidArgument);
    }
    let mut bytes = b"omavless.control/profile-import/v1\0".to_vec();
    append_field(&mut bytes, name);
    append_field(&mut bytes, input);
    match metadata.expected_revision {
        Some(revision) => {
            bytes.push(1);
            bytes.extend_from_slice(&revision.to_be_bytes());
        }
        None => bytes.push(0),
    }
    Ok(ProfileImportRequest {
        name: name.to_owned(),
        input: input.to_owned(),
        operation_id: metadata.operation_id.map(str::to_owned),
        expected_revision: metadata.expected_revision,
        digest: MutationDigest::from_semantic_bytes(&bytes),
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;
    fn request(params: Value) -> Value {
        json!({"api": "omavless.control", "version": 1, "id": "import",
               "method": "profiles.import", "params": params})
    }
    #[test]
    fn native_data_admission_roundtrips_wg_awg_crlf_and_compressed_input() {
        let native = "[Interface]\nPrivateKey = BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=\nAddress = 10.8.0.2/32\n[Peer]\nPublicKey = CQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQk=\nAllowedIPs = 0.0.0.0/0\nEndpoint = 192.0.2.2:51820\n";
        let awg = native.replace("[Peer]", "Jc = 4\nJmin = 10\nJmax = 30\nS1 = 20\nS2 = 25\nH1 = 100\nH2 = 200\nH3 = 300\nH4 = 400\n[Peer]");
        // Public synthetic qCompress fixture, not a provider credential.
        let compressed = "vpn://AAAAznici_bMK0ktSktMTo3lCijKLEssSfVOrVSwVXAqT_YgEttyOaakFKUWFyvYKhga6FnoGegZ6RsbcUUHpKYWxXIFlCblZCZDTHUOzPYiEttyOebk5JenpngGgAw20ANDfQMu17yUgvzMvBKQbZZGIMv0jKxMDS2MDLgAAnc-uQ";
        for text in [
            native.to_owned(),
            native.replace('\n', "\r\n"),
            awg,
            compressed.to_owned(),
        ] {
            assert!(admitted_profile_input(&text));
            assert!(
                parse_profile_import_request(&request(json!({"name":"Synthetic", "input":text})))
                    .is_ok()
            );
        }
        assert!(!admitted_profile_input(
            &native.replace("[Peer]", "PostUp = execute\n[Peer]")
        ));
        assert!(!admitted_profile_input(
            &"x".repeat(MAX_IMPORT_STDIN_BYTES + 1)
        ));
        let escaped =
            request(json!({"name":"Name", "input":"\u{0001}".repeat(MAX_IMPORT_STDIN_BYTES)}));
        assert!(parse_profile_import_request(&escaped).is_err());
    }

    pub(crate) fn oversized_native_fixture() -> String {
        let ips = (0..180)
            .map(|i| format!("2001:db8:{i:x}:1234:1234:1234:1234:1234/128"))
            .collect::<Vec<_>>()
            .join(", ");
        let dns = (0..180)
            .map(|i| format!("2001:db8:{i:x}:1234:1234:1234:1234:1234"))
            .collect::<Vec<_>>()
            .join(", ");
        let junk = (1..=5)
            .map(|i| format!("I{i} = {}\n", "<r 1>".repeat(810)))
            .collect::<String>();
        format!(
            "[Interface]\nPrivateKey = BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=\nAddress = 10.8.0.2/32\nDNS = {dns}\nJc = 4\nJmin = 10\nJmax = 30\nS1 = 20\nS2 = 25\nS3 = 30\nS4 = 35\nH1 = 100\nH2 = 200\nH3 = 300\nH4 = 400\n{junk}[Peer]\nPublicKey = CQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQk=\nAllowedIPs = {ips}\nEndpoint = 192.0.2.2:51820\n"
        )
    }

    #[test]
    fn oversized_valid_native_store_material_is_not_silently_editable_or_exportable() {
        use omavless_domain::private_store::PrivateProfileCredential;
        let input = oversized_native_fixture();
        let profile = PrivateProfileCredential::parse_input(&input).unwrap();
        assert!(
            profile
                .private_export()
                .unwrap()
                .expose_private_bytes()
                .len()
                > MAX_IMPORT_STDIN_BYTES
        );
        assert!(!admitted_profile_input(&input));
        // A compact source under 32KiB can expand past the editor bound when
        // canonical separators are restored. Admission must check both forms.
        let compact = input.replace(", ", ",").replace(" = ", "=");
        let compact = (0..200)
            .find_map(|removed| {
                let candidate =
                    compact.replace(&"<r 1>".repeat(810), &"<r 1>".repeat(810 - removed));
                if candidate.len() > MAX_IMPORT_STDIN_BYTES {
                    return None;
                }
                let p = PrivateProfileCredential::parse_input(&candidate).ok()?;
                (p.private_export().ok()?.expose_private_bytes().len() > MAX_IMPORT_STDIN_BYTES)
                    .then_some(candidate)
            })
            .expect("bounded synthetic canonical growth witness");
        assert!(!admitted_profile_input(&compact));
    }
    #[test]
    fn import_rejects_client_ids_paths_and_unknown_input() {
        for params in [
            json!({}),
            json!({"name": "Name", "input": null}),
            json!({"name": " ", "input": "private-token"}),
            json!({"name": "Name", "input": "private-token", "profileId": "generated"}),
            json!({"name": "Name", "input": "private-token", "oldId": "replace"}),
            json!({"name": "Name", "input": "private-token", "path": "/tmp/input"}),
            json!({"name": "Name", "input": "x".repeat(MAX_IMPORT_STDIN_BYTES+1)}),
            json!({"name": "x".repeat(MAX_PROFILE_NAME_INPUT_BYTES+1), "input": "private-token"}),
        ] {
            let error = parse_profile_import_request(&request(params))
                .err()
                .unwrap();
            assert!(!format!("{error:?} {error}").contains("private-token"));
        }
    }
    #[test]
    fn import_digest_binds_private_intent_and_revision_but_not_request_id() {
        let original = request(
            json!({"name": "Name", "input": "trojan://synthetic-password@203.0.113.1:443", "operationId": "op"}),
        );
        let parsed = parse_profile_import_request(&original).ok().unwrap();
        for (field, value) in [
            ("name", json!("Other")),
            ("input", json!("trojan://other-password@203.0.113.1:443")),
            ("expectedRevision", json!(0)),
        ] {
            let mut changed = original.clone();
            changed["params"][field] = value;
            assert!(parse_profile_import_request(&changed).ok().unwrap().digest != parsed.digest);
        }
        let mut retry = original.clone();
        retry["id"] = json!("retry");
        assert!(parse_profile_import_request(&retry).ok().unwrap().digest == parsed.digest);
    }
}
