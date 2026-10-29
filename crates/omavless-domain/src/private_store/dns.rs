// SPDX-License-Identifier: MIT
//! A closed user preference; resolver addresses are never caller input.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DnsPreset {
    #[default]
    PresetDefault,
    Google,
    Cloudflare,
}

impl DnsPreset {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "preset-default" => Some(Self::PresetDefault),
            "google" => Some(Self::Google),
            "cloudflare" => Some(Self::Cloudflare),
            _ => None,
        }
    }

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::PresetDefault => "preset-default",
            Self::Google => "google",
            Self::Cloudflare => "cloudflare",
        }
    }
}

pub(super) fn validate(value: Option<&Value>) -> Result<DnsPreset, PrivateStoreError> {
    match value {
        None => Ok(DnsPreset::PresetDefault),
        Some(value) => value
            .as_str()
            .and_then(DnsPreset::parse)
            .ok_or(PrivateStoreError::InvalidShape),
    }
}

impl PrivateStore {
    /// Saved intent only. Custom templates retain their own DNS policy.
    #[must_use]
    pub fn dns_preset(&self) -> DnsPreset {
        // The complete document was validated at construction.
        validate(self.document.get("dnsPreset")).expect("validated DNS preference")
    }
}

pub fn apply_dns_preset(
    input: &str,
    preset: DnsPreset,
) -> Result<(Vec<u8>, bool), PrivateStoreError> {
    let mut store = parse_private_store(input)?;
    if store.dns_preset() == preset {
        // In particular, a legacy absent default stays byte-for-byte absent.
        return Ok((input.as_bytes().to_vec(), false));
    }
    store.document["dnsPreset"] = preset.id().into();
    let validated = parse_private_store(&store.document.to_string())?;
    let mut bytes =
        serde_json::to_vec(&validated.document).map_err(|_| PrivateStoreError::InvalidJson)?;
    bytes.push(b'\n');
    Ok((bytes, true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn legacy_absence_and_default_noop_preserve_bytes() {
        let input = "{\"version\":3, \"profiles\":[],\"subscriptions\":[]}\n";
        let store = parse_private_store(input).unwrap();
        assert_eq!(store.dns_preset(), DnsPreset::PresetDefault);
        assert!(
            !store
                .document
                .as_object()
                .unwrap()
                .contains_key("dnsPreset")
        );
        assert_eq!(
            apply_dns_preset(input, DnsPreset::PresetDefault).unwrap(),
            (input.as_bytes().to_vec(), false)
        );
    }

    #[test]
    fn only_known_string_ids_are_accepted_without_echo() {
        for value in [
            Value::Null,
            json!(true),
            json!([]),
            json!("Google"),
            json!("https://private-input.invalid/token"),
        ] {
            let source = json!({"version":3,"profiles":[],"subscriptions":[],"dnsPreset":value});
            let error = parse_private_store(&source.to_string()).err().unwrap();
            assert!(!format!("{error:?} {error}").contains("private-input"));
        }
    }

    #[test]
    fn explicit_preference_preserves_all_other_normalized_fields() {
        let source = json!({"version":3,"profiles":[],"subscriptions":[],
            "futureExtension":{"nested":[1,true,"synthetic"]}});
        let original = parse_private_store(&source.to_string()).unwrap().document;
        let mut input = source.to_string();
        for preset in [
            DnsPreset::Google,
            DnsPreset::Cloudflare,
            DnsPreset::PresetDefault,
        ] {
            let (bytes, changed) = apply_dns_preset(&input, preset).unwrap();
            assert!(changed);
            input = String::from_utf8(bytes).unwrap();
            let store = parse_private_store(&input).unwrap();
            assert_eq!(store.dns_preset(), preset);
            let mut rest = store.document;
            rest.as_object_mut().unwrap().remove("dnsPreset");
            assert_eq!(rest, original);
        }
    }
}
